//! HTTP ingest and state endpoints, bound to 127.0.0.1 only.
//!
//! Routes:
//!   GET  /healthz                 liveness, no auth
//!   POST /v1/events               a unified Event (adapters that run elsewhere)
//!   POST /v1/native/{host}        a host's native payload (hooks, the shim)
//!   GET  /v1/state                snapshot of every session's state
//!
//! All `/v1` routes require `Authorization: Bearer <token>`.

use crate::adapters::{self, now_ms};
use crate::home::{self, DaemonState};
use crate::redact;
use crate::registry::{Registry, Snapshot};
use crate::store::Store;
use anyhow::{Context, Result};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use sidepaw_core::{Event, Host, Level};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct App {
    pub store: Store,
    pub registry: Registry,
    pub level: Level,
}

#[derive(Clone)]
pub struct Shared {
    app: Arc<Mutex<App>>,
    token: Arc<String>,
}

pub async fn run(port: u16) -> Result<()> {
    let dir = home::dir()?;
    let store = Store::open(&dir.join("sidepaw.sqlite"))?;
    let token = DaemonState::token_or_new();
    let shared = Shared {
        app: Arc::new(Mutex::new(App {
            store,
            registry: Registry::default(),
            level: Level::DEFAULT,
        })),
        token: Arc::new(token.clone()),
    };

    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .context("bind")?;
    let port = listener.local_addr()?.port();
    DaemonState { port, token }.save()?;
    tracing::info!("listening on 127.0.0.1:{port}, data in {}", dir.display());

    tokio::spawn(tick_loop(shared.app.clone()));

    axum::serve(listener, router(shared))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}

pub fn router(shared: Shared) -> Router {
    let protected = Router::new()
        .route("/v1/events", post(post_event))
        .route("/v1/native/{host}", post(post_native))
        .route("/v1/state", get(get_state))
        .route_layer(middleware::from_fn_with_state(shared.clone(), auth));
    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .merge(protected)
        .with_state(shared)
}

async fn auth(
    State(shared): State<Shared>,
    headers: HeaderMap,
    req: axum::extract::Request,
    next: Next,
) -> Response {
    let ok = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|t| t == shared.token.as_str())
        .unwrap_or(false);
    if ok {
        next.run(req).await
    } else {
        StatusCode::UNAUTHORIZED.into_response()
    }
}

async fn post_event(State(shared): State<Shared>, Json(event): Json<Event>) -> StatusCode {
    ingest(&shared, event)
}

async fn post_native(
    State(shared): State<Shared>,
    Path(host): Path<String>,
    Json(payload): Json<serde_json::Value>,
) -> StatusCode {
    let host: Host = match serde_json::from_value(serde_json::Value::String(host)) {
        Ok(h) => h,
        Err(_) => return StatusCode::NOT_FOUND,
    };
    match adapters::from_native(host, &payload) {
        Some(event) => ingest(&shared, event),
        None => StatusCode::NO_CONTENT,
    }
}

async fn get_state(State(shared): State<Shared>) -> Json<Snapshot> {
    let app = shared.app.lock().unwrap();
    Json(app.registry.snapshot(now_ms()))
}

/// Redact, filter, store, advance the state machine. The order matters:
/// nothing reaches the store before redaction and the level filter.
fn ingest(shared: &Shared, mut event: Event) -> StatusCode {
    let mut app = shared.app.lock().unwrap();
    redact::event(&mut event);
    app.level.filter(&mut event);
    if let Err(e) = app.store.insert_event(&event) {
        tracing::error!("store: {e}");
        return StatusCode::INTERNAL_SERVER_ERROR;
    }
    if let Some((key, state)) = app.registry.apply(&event, now_ms()) {
        tracing::debug!("{key} -> {state:?}");
        if let Err(e) = app.store.insert_state_change(&key, &event.ts, state) {
            tracing::error!("store: {e}");
        }
    }
    StatusCode::ACCEPTED
}

async fn tick_loop(app: Arc<Mutex<App>>) {
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    loop {
        interval.tick().await;
        let now = now_ms();
        let mut app = app.lock().unwrap();
        let changes = app.registry.tick(now);
        for (key, state) in changes {
            let ts = adapters::now_rfc3339();
            if let Err(e) = app.store.insert_state_change(&key, &ts, state) {
                tracing::error!("store: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    fn test_shared() -> Shared {
        Shared {
            app: Arc::new(Mutex::new(App {
                store: Store::in_memory().unwrap(),
                registry: Registry::default(),
                level: Level::DEFAULT,
            })),
            token: Arc::new("t0k3n".into()),
        }
    }

    fn req(method: &str, path: &str, token: Option<&str>, body: &str) -> Request<Body> {
        let mut b = Request::builder()
            .method(method)
            .uri(path)
            .header("content-type", "application/json");
        if let Some(t) = token {
            b = b.header("authorization", format!("Bearer {t}"));
        }
        b.body(Body::from(body.to_owned())).unwrap()
    }

    #[tokio::test]
    async fn rejects_missing_token() {
        let r = router(test_shared());
        let res = r.oneshot(req("GET", "/v1/state", None, "")).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn native_claude_code_payload_moves_state() {
        let shared = test_shared();
        let r = router(shared.clone());
        let payload = r#"{"hook_event_name":"PreToolUse","session_id":"s1","cwd":"/p",
                          "tool_name":"Bash","tool_input":{"command":"ls"}}"#;
        let res = r
            .clone()
            .oneshot(req(
                "POST",
                "/v1/native/claude-code",
                Some("t0k3n"),
                payload,
            ))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::ACCEPTED);

        let res = r
            .oneshot(req("GET", "/v1/state", Some("t0k3n"), ""))
            .await
            .unwrap();
        let body = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["overall"]["state"], "tool");
        assert_eq!(v["sessions"][0]["key"], "claude-code:s1");
    }

    /// Regression: approving a permission prompt used to leave the pet in
    /// Waiting until the turn ended, because Claude Code sends no event on
    /// approve.
    #[tokio::test]
    async fn approved_permission_returns_to_thinking() {
        let shared = test_shared();
        let r = router(shared.clone());
        let common =
            r#""session_id":"s1","cwd":"/p","tool_name":"Bash","tool_input":{"command":"ls"}"#;
        for hook in ["PreToolUse", "PermissionRequest", "PostToolUse"] {
            let payload = format!(r#"{{"hook_event_name":"{hook}",{common}}}"#);
            let res = r
                .clone()
                .oneshot(req(
                    "POST",
                    "/v1/native/claude-code",
                    Some("t0k3n"),
                    &payload,
                ))
                .await
                .unwrap();
            assert_eq!(res.status(), StatusCode::ACCEPTED, "{hook}");
        }
        let res = r
            .oneshot(req("GET", "/v1/state", Some("t0k3n"), ""))
            .await
            .unwrap();
        let body = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["overall"]["state"], "thinking");
    }

    #[tokio::test]
    async fn unknown_host_is_404_and_ignored_payload_is_204() {
        let r = router(test_shared());
        let res = r
            .clone()
            .oneshot(req("POST", "/v1/native/nope", Some("t0k3n"), "{}"))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
        let res = r
            .oneshot(req(
                "POST",
                "/v1/native/claude-code",
                Some("t0k3n"),
                r#"{"hook_event_name":"PreCompact","session_id":"s"}"#,
            ))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NO_CONTENT);
    }
}
