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
use crate::registry::{PendingApproval, Registry, Snapshot};
use crate::store::{DayReport, Store};
use anyhow::{Context, Result};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use sidepaw_core::{Event, Host, Kind, Level};
use std::collections::HashMap;
use std::future::IntoFuture;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::oneshot;

pub struct App {
    pub store: Store,
    pub registry: Registry,
    pub config: Config,
    /// Permission prompts held open for the user to answer (level 4).
    pending: HashMap<u64, Pending>,
    next_pending: u64,
}

impl App {
    pub fn new(store: Store, registry: Registry, config: Config) -> Self {
        App {
            store,
            registry,
            config,
            pending: HashMap::new(),
            next_pending: 1,
        }
    }
}

struct Pending {
    key: String,
    tool: Option<String>,
    summary: Option<String>,
    deadline_ms: u64,
    answer: oneshot::Sender<bool>,
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
        let now = now_ms();
        let app = self.app.lock().unwrap();
        let mut snap = app.registry.snapshot(now);
        snap.pending = app
            .pending
            .iter()
            .map(|(id, p)| PendingApproval {
                id: *id,
                key: p.key.clone(),
                tool: p.tool.clone(),
                summary: p.summary.clone(),
                expires_in_ms: p.deadline_ms.saturating_sub(now),
            })
            .collect();
        snap.pending.sort_by_key(|p| p.id);
        snap
    }

    /// Answer a held permission prompt. The hook handler that holds it
    /// records the decision and replies to the host.
    pub fn decide(&self, id: u64, allow: bool) -> Result<()> {
        let pending = self
            .app
            .lock()
            .unwrap()
            .pending
            .remove(&id)
            .context("that prompt is no longer waiting")?;
        pending
            .answer
            .send(allow)
            .map_err(|_| anyhow::anyhow!("the host stopped waiting"))
    }

    fn register_pending(
        &self,
        key: String,
        tool: Option<String>,
        summary: Option<String>,
        window_ms: u64,
    ) -> (u64, oneshot::Receiver<bool>) {
        let (tx, rx) = oneshot::channel();
        let mut app = self.app.lock().unwrap();
        let id = app.next_pending;
        app.next_pending += 1;
        app.pending.insert(
            id,
            Pending {
                key,
                tool,
                summary,
                deadline_ms: now_ms() + window_ms,
                answer: tx,
            },
        );
        (id, rx)
    }

    pub fn set_host_act(&self, host: Host, act: bool) -> Result<()> {
        Config::write_host_act(&home::dir()?, host, act)?;
        let mut app = self.app.lock().unwrap();
        app.config
            .hosts
            .entry(host.as_str().to_owned())
            .or_default()
            .act = act;
        Ok(())
    }

    pub fn host_act(&self, host: Host) -> bool {
        self.app.lock().unwrap().config.host(host).act
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn level(&self) -> Level {
        self.app.lock().unwrap().config.pet.level
    }

    /// Persist and apply a new global level. Per-host and per-project
    /// overrides in the file stay as they are.
    pub fn set_level(&self, level: Level) -> Result<()> {
        Config::write_level(&home::dir()?, level)?;
        self.app.lock().unwrap().config.pet.level = level;
        Ok(())
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
    let shared = Shared::new(App::new(store, Registry::default(), config), token.clone());
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
) -> Response {
    let Ok(host) = host.parse::<Host>() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(event) = adapters::from_native(host, &payload) else {
        return StatusCode::NO_CONTENT.into_response();
    };

    // Level 4 with `act` on: hold the host's permission prompt open so
    // the user can answer from the pet. Claude Code shows its own
    // prompt only after this request returns, so the window is short
    // and an unanswered prompt falls through to the terminal unchanged.
    let hold = event.kind == Kind::ApprovalWait && {
        let app = shared.app.lock().unwrap();
        let cfg = app.config.host(host);
        cfg.act
            && app
                .config
                .effective_level(host, event.cwd.as_deref())
                .may_act()
    };
    let key = event.session_key();
    let tool = event.tool.as_ref().map(|t| t.name.clone());
    let summary = event.text.as_deref().map(redact::text);
    let window_ms = shared.app.lock().unwrap().config.host(host).act_window_s * 1000;

    let status = ingest(&shared, event);
    if !hold || status != StatusCode::ACCEPTED {
        return status.into_response();
    }

    let (id, rx) = shared.register_pending(key.clone(), tool.clone(), summary.clone(), window_ms);
    let decision = tokio::time::timeout(Duration::from_millis(window_ms), rx).await;
    let Ok(Ok(allow)) = decision else {
        shared.app.lock().unwrap().pending.remove(&id);
        return StatusCode::NO_CONTENT.into_response();
    };

    {
        let mut app = shared.app.lock().unwrap();
        if let Err(e) = app.store.insert_approval(
            &key,
            &now_rfc3339(),
            tool.as_deref(),
            summary.as_deref(),
            allow,
        ) {
            tracing::error!("store: {e}");
        }
    }
    ingest(&shared, answered(host, &key));
    Json(decision_body(allow)).into_response()
}

/// The event the state machine needs once the user has answered; the
/// host sends nothing on its own for the approve path.
fn answered(host: Host, key: &str) -> Event {
    Event {
        v: 1,
        host,
        session: key
            .split_once(':')
            .map(|(_, s)| s)
            .unwrap_or(key)
            .to_owned(),
        cwd: None,
        ts: now_rfc3339(),
        kind: Kind::ApprovalDone,
        tool: None,
        text: None,
        error: None,
        tokens: None,
        raw: None,
    }
}

/// Claude Code's PermissionRequest decision shape.
fn decision_body(allow: bool) -> serde_json::Value {
    let decision = if allow {
        serde_json::json!({ "behavior": "allow" })
    } else {
        serde_json::json!({ "behavior": "deny", "message": "Denied from sidepaw." })
    };
    serde_json::json!({
        "hookSpecificOutput": { "hookEventName": "PermissionRequest", "decision": decision }
    })
}

async fn get_state(State(shared): State<Shared>) -> Json<Snapshot> {
    Json(shared.snapshot())
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
    use sidepaw_core::State;
    use tower::ServiceExt;

    fn test_shared() -> Shared {
        shared_with(Config::default())
    }

    fn shared_with(config: Config) -> Shared {
        Shared::new(
            App::new(Store::in_memory().unwrap(), Registry::default(), config),
            "t0k3n".into(),
        )
    }

    const PERMISSION: &str = r#"{"hook_event_name":"PermissionRequest","session_id":"s1","cwd":"/p",
                                 "tool_name":"Bash","tool_input":{"command":"rm -rf build"}}"#;

    fn act_config(window_s: u64) -> Config {
        Config::parse(&format!(
            "[pet]\nlevel = 4\n[hosts.claude-code]\nact = true\nact_window_s = {window_s}\n"
        ))
        .unwrap()
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
    async fn held_permission_prompt_is_answered_from_the_pet() {
        let shared = shared_with(act_config(5));
        let r = router(shared.clone());
        let request = tokio::spawn(async move {
            r.oneshot(req(
                "POST",
                "/v1/native/claude-code",
                Some("t0k3n"),
                PERMISSION,
            ))
            .await
            .unwrap()
        });

        let mut snap = shared.snapshot();
        for _ in 0..50 {
            if !snap.pending.is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
            snap = shared.snapshot();
        }
        assert_eq!(snap.overall, State::Waiting);
        let p = &snap.pending[0];
        assert_eq!(p.tool.as_deref(), Some("Bash"));
        assert_eq!(p.summary.as_deref(), Some("rm -rf build"));
        assert!(p.expires_in_ms <= 5_000);

        shared.decide(p.id, false).unwrap();
        let res = request.await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["hookSpecificOutput"]["decision"]["behavior"], "deny");

        let snap = shared.snapshot();
        assert!(snap.pending.is_empty());
        assert_ne!(
            snap.overall,
            State::Waiting,
            "answered prompts stop waiting"
        );
        let n: i64 = {
            // Scoped: `decide` below takes the same lock.
            let app = shared.app.lock().unwrap();
            app.store
                .conn_for_tests()
                .query_row(
                    "SELECT COUNT(*) FROM approvals WHERE decision = 'deny'",
                    [],
                    |r| r.get(0),
                )
                .unwrap()
        };
        assert_eq!(n, 1);
        assert!(shared.decide(p.id, true).is_err(), "answered once only");
    }

    #[tokio::test]
    async fn unanswered_prompt_falls_through_to_the_host() {
        let shared = shared_with(act_config(0));
        let r = router(shared.clone());
        let res = r
            .oneshot(req(
                "POST",
                "/v1/native/claude-code",
                Some("t0k3n"),
                PERMISSION,
            ))
            .await
            .unwrap();
        assert_eq!(
            res.status(),
            StatusCode::NO_CONTENT,
            "empty 2xx: the terminal prompt shows"
        );
        let snap = shared.snapshot();
        assert!(snap.pending.is_empty());
        assert_eq!(snap.overall, State::Waiting, "the pet still shows waiting");
    }

    #[tokio::test]
    async fn act_needs_level_four_and_the_host_switch() {
        for cfg in [
            "[pet]\nlevel = 4\n",
            "[pet]\nlevel = 3\n[hosts.claude-code]\nact = true\n",
        ] {
            let shared = shared_with(Config::parse(cfg).unwrap());
            let r = router(shared.clone());
            let res = r
                .oneshot(req(
                    "POST",
                    "/v1/native/claude-code",
                    Some("t0k3n"),
                    PERMISSION,
                ))
                .await
                .unwrap();
            assert_eq!(res.status(), StatusCode::ACCEPTED, "{cfg}");
            assert!(shared.snapshot().pending.is_empty(), "{cfg}");
        }
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
