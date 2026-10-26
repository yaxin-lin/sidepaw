//! HTTP ingest and state endpoints, bound to 127.0.0.1 only.
//!
//! Routes:
//!   GET  /healthz                 liveness, no auth
//!   POST /v1/events               a unified Event (adapters that run elsewhere)
//!   POST /v1/native/{host}        a host's native payload (hooks, the shim)
//!   GET  /v1/state                snapshot of every session's state
//!   GET  /v1/report/today         counters since local midnight
//!
//! All `/v1` routes require `Authorization: Bearer <token>`.

use crate::adapters;
use crate::clock::{now_ms, now_rfc3339, start_of_today_rfc3339};
use crate::config::Config;
use crate::home::{self, DaemonState};
use crate::redact;
use crate::registry::{Registry, Snapshot};
use crate::store::{DayReport, Store};
use anyhow::{Context, Result};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use sidepaw_core::{Event, Host};
use std::future::IntoFuture;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct App {
    pub store: Store,
    pub registry: Registry,
    pub config: Config,
}

/// `raw` carries whole file contents for edit tools; above this the
/// payload is dropped rather than stored. Everything else in the event
/// survives.
pub const RAW_MAX_BYTES: usize = 64 * 1024;

#[derive(Clone)]
pub struct Shared {
    app: Arc<Mutex<App>>,
    token: Arc<String>,
}

impl Shared {
    pub fn new(app: App, token: String) -> Self {
        Shared {
            app: Arc::new(Mutex::new(app)),
            token: Arc::new(token),
        }
    }

    /// The same view `GET /v1/state` returns, for callers in the same
    /// process (the desktop app) that do not need to go through HTTP.
    pub fn snapshot(&self) -> Snapshot {
        self.app.lock().unwrap().registry.snapshot(now_ms())
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn report_today(&self) -> Result<DayReport> {
        self.app
            .lock()
            .unwrap()
            .store
            .report_since(&start_of_today_rfc3339())
    }
}

/// A server that has bound its socket and is accepting in the
/// background on the current tokio runtime.
pub struct Running {
    pub port: u16,
    pub shared: Shared,
}

/// Open the store, bind `127.0.0.1:port`, record port and token in
/// `state.json`, and spawn the accept loop and the decay ticker. Returns
/// as soon as the socket is bound; the server lives as long as the
/// runtime does.
pub async fn start(port: u16) -> Result<Running> {
    let dir = home::dir()?;
    let config = Config::load(&dir)?;
    let store = Store::open(&dir.join("sidepaw.sqlite"))?;
    let previous = DaemonState::load().ok();
    let token = previous
        .as_ref()
        .map(|s| s.token.clone())
        .unwrap_or_else(DaemonState::new_token);
    // Precedence for the port: the caller, then config.toml, then the
    // port from last run (hook configs embed it), then any free one.
    let port = if port == 0 { config.daemon.port } else { port };
    let shared = Shared::new(
        App {
            store,
            registry: Registry::default(),
            config,
        },
        token.clone(),
    );
    let wanted = match (port, previous.as_ref()) {
        (0, Some(s)) if s.port != 0 => s.port,
        _ => port,
    };
    let listener = match tokio::net::TcpListener::bind(("127.0.0.1", wanted)).await {
        Ok(l) => l,
        Err(_) if wanted != port => tokio::net::TcpListener::bind(("127.0.0.1", port))
            .await
            .context("bind")?,
        Err(e) => return Err(e).context("bind"),
    };
    let port = listener.local_addr()?.port();
    DaemonState { port, token }.save()?;
    tracing::info!("listening on 127.0.0.1:{port}, data in {}", dir.display());

    tokio::spawn(tick_loop(shared.app.clone()));
    tokio::spawn(axum::serve(listener, router(shared.clone())).into_future());
    Ok(Running { port, shared })
}

/// Headless mode: `start`, then block until Ctrl-C.
pub async fn run(port: u16) -> Result<()> {
    let _running = start(port).await?;
    tokio::signal::ctrl_c().await?;
    Ok(())
}

pub fn router(shared: Shared) -> Router {
    let protected = Router::new()
        .route("/v1/events", post(post_event))
        .route("/v1/native/{host}", post(post_native))
        .route("/v1/state", get(get_state))
        .route("/v1/report/today", get(get_report_today))
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
    let Ok(host) = host.parse::<Host>() else {
        return StatusCode::NOT_FOUND;
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

async fn get_report_today(State(shared): State<Shared>) -> Response {
    match shared.report_today() {
        Ok(r) => Json(r).into_response(),
        Err(e) => {
            tracing::error!("report: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Redact, filter, store, advance the state machine. The order matters:
/// the level is resolved from the full event (it needs `cwd`), and
/// nothing reaches the store before redaction and the level filter.
fn ingest(shared: &Shared, mut event: Event) -> StatusCode {
    let mut app = shared.app.lock().unwrap();
    if !app.config.host_enabled(event.host) {
        return StatusCode::NO_CONTENT;
    }
    let level = app.config.effective_level(event.host, event.cwd.as_deref());
    redact::event(&mut event);
    level.filter(&mut event);
    if event
        .raw
        .as_ref()
        .is_some_and(|r| r.to_string().len() > RAW_MAX_BYTES)
    {
        event.raw = None;
    }
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
            let ts = now_rfc3339();
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
        shared_with(Config::default())
    }

    fn shared_with(config: Config) -> Shared {
        Shared::new(
            App {
                store: Store::in_memory().unwrap(),
                registry: Registry::default(),
                config,
            },
            "t0k3n".into(),
        )
    }

    fn stored_text(shared: &Shared) -> Option<String> {
        shared.app.lock().unwrap().store.last_event_text().unwrap()
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
    async fn level_is_resolved_from_host_and_project_before_filtering() {
        let config = Config::parse(
            r#"
[pet]
level = 2
[projects."/work/secret"]
level = 0
"#,
        )
        .unwrap();
        let shared = shared_with(config);
        let r = router(shared.clone());
        let payload = |cwd: &str| {
            format!(
                r#"{{"hook_event_name":"PreToolUse","session_id":"s","cwd":"{cwd}",
                     "tool_name":"Bash","tool_input":{{"command":"make"}}}}"#
            )
        };
        r.clone()
            .oneshot(req(
                "POST",
                "/v1/native/claude-code",
                Some("t0k3n"),
                &payload("/home/me/toy"),
            ))
            .await
            .unwrap();
        assert_eq!(
            stored_text(&shared).as_deref(),
            Some("make"),
            "level 2 keeps text"
        );

        r.oneshot(req(
            "POST",
            "/v1/native/claude-code",
            Some("t0k3n"),
            &payload("/work/secret/src"),
        ))
        .await
        .unwrap();
        assert_eq!(stored_text(&shared), None, "project at level 0 drops text");
    }

    #[tokio::test]
    async fn disabled_host_is_ignored() {
        let shared = shared_with(
            Config::parse(
                "[hosts.claude-code]
enabled = false
",
            )
            .unwrap(),
        );
        let r = router(shared.clone());
        let res = r
            .oneshot(req(
                "POST",
                "/v1/native/claude-code",
                Some("t0k3n"),
                r#"{"hook_event_name":"Stop","session_id":"s","cwd":"/"}"#,
            ))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NO_CONTENT);
        assert_eq!(shared.app.lock().unwrap().store.session_count().unwrap(), 0);
    }

    #[tokio::test]
    async fn oversized_raw_is_dropped_not_the_event() {
        let shared = shared_with(
            Config::parse(
                "[pet]
level = 2
",
            )
            .unwrap(),
        );
        let r = router(shared.clone());
        let big = "x".repeat(RAW_MAX_BYTES + 1);
        let payload = format!(
            r#"{{"hook_event_name":"PreToolUse","session_id":"s","cwd":"/p",
                 "tool_name":"Write","tool_input":{{"file_path":"/p/a","content":"{big}"}}}}"#
        );
        let res = r
            .oneshot(req(
                "POST",
                "/v1/native/claude-code",
                Some("t0k3n"),
                &payload,
            ))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::ACCEPTED);
        let app = shared.app.lock().unwrap();
        assert_eq!(app.store.session_count().unwrap(), 1);
        assert!(app.store.last_event_raw().unwrap().is_none());
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
