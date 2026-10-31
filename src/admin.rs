//! Loopback-only console, separate from inference. Browser credentials are short lived.
use crate::{
    config::{self, Result},
    harness,
    telemetry::{Filter, Store, hash},
};
use axum::{
    Json, Router,
    body::to_bytes,
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::any,
};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use subtle::ConstantTimeEq;
#[derive(Default)]
struct Auth {
    tickets: HashMap<String, Instant>,
    sessions: HashMap<String, Instant>,
}
#[derive(Clone)]
pub struct App {
    pub data: PathBuf,
    pub store: Store,
    pub port: u16,
    auth: Arc<Mutex<Auth>>,
}
impl App {
    pub fn new(data: PathBuf, store: Store, port: u16) -> Self {
        Self {
            data,
            store,
            port,
            auth: Arc::new(Mutex::new(Auth::default())),
        }
    }
}
pub fn router(app: App) -> Router {
    Router::new().fallback(any(handle)).with_state(app)
}
fn text(h: &HeaderMap, name: &str) -> Option<String> {
    h.get(name).and_then(|v| v.to_str().ok()).map(String::from)
}
fn error(status: u16, code: &str) -> Response {
    (
        StatusCode::from_u16(status).unwrap(),
        Json(json!({"error":code})),
    )
        .into_response()
}
fn token() -> String {
    format!(
        "{:032x}{:032x}",
        rand::random::<u128>(),
        rand::random::<u128>()
    )
}
fn query(uri: &axum::http::Uri) -> HashMap<String, String> {
    reqwest::Url::parse(&format!("http://localhost{uri}"))
        .map(|u| u.query_pairs().into_owned().collect())
        .unwrap_or_default()
}
async fn handle(State(app): State<App>, req: Request) -> Response {
    let h = req.headers().clone();
    let path = req.uri().path().to_string();
    let method = req.method().to_string();
    let q = query(req.uri());
    let host = text(&h, "host");
    let origin = text(&h, "origin");
    let hosts = [
        format!("127.0.0.1:{}", app.port),
        format!("localhost:{}", app.port),
    ];
    if !host.as_ref().is_some_and(|h| hosts.contains(h))
        || origin
            .as_ref()
            .is_some_and(|s| !hosts.iter().any(|h| s == &format!("http://{h}")))
        || text(&h, "sec-fetch-site").as_deref() == Some("cross-site")
    {
        return error(403, "LOCAL_SAME_ORIGIN_REQUIRED");
    }
    let mut result = if method == "GET" && path == "/" {
        (
            [("content-type", "text/html; charset=utf-8")],
            include_str!("../web/index.html"),
        )
            .into_response()
    } else if method == "GET" && path == "/app.js" {
        (
            [("content-type", "application/javascript; charset=utf-8")],
            include_str!("../web/app.js"),
        )
            .into_response()
    } else if method == "GET" && path == "/style.css" {
        (
            [("content-type", "text/css; charset=utf-8")],
            include_str!("../web/style.css"),
        )
            .into_response()
    } else if method == "POST" && path == "/api/launch" {
        if origin.is_some() {
            return error(403, "CLI_ONLY");
        }
        let key = std::fs::read_to_string(app.data.join("hub.key")).unwrap_or_default();
        let provided = text(&h, "authorization").unwrap_or_default();
        if key.trim().is_empty()
            || !bool::from(
                provided
                    .as_bytes()
                    .ct_eq(format!("Bearer {}", key.trim()).as_bytes()),
            )
        {
            return error(401, "LOCAL_KEY_REQUIRED");
        }
        let t = token();
        let mut auth = app.auth.lock().unwrap();
        auth.tickets
            .retain(|_, t| t.elapsed() < Duration::from_secs(60));
        auth.tickets.insert(hash(&t), Instant::now());
        Json(json!({"ticket":t,"expires_in":60})).into_response()
    } else if method == "POST" && path == "/api/login" {
        if origin.is_none() {
            return error(403, "ORIGIN_REQUIRED");
        }
        let bytes = match to_bytes(req.into_body(), 2048).await {
            Ok(v) => v,
            Err(_) => return error(400, "INVALID_LOGIN"),
        };
        let v: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        let mut auth = app.auth.lock().unwrap();
        let valid = auth
            .tickets
            .remove(&hash(v["ticket"].as_str().unwrap_or("")))
            .is_some_and(|t| t.elapsed() < Duration::from_secs(60));
        if !valid {
            return error(401, "TICKET_EXPIRED");
        }
        auth.sessions
            .retain(|_, t| t.elapsed() < Duration::from_secs(8 * 3600));
        let session = token();
        auth.sessions.insert(hash(&session), Instant::now());
        let cookie =
            format!("loom_console={session}; HttpOnly; SameSite=Strict; Path=/; Max-Age=28800");
        ([("set-cookie", cookie)], Json(json!({"ok":true}))).into_response()
    } else {
        let cookie = text(&h, "cookie").unwrap_or_default();
        let cookie = cookie
            .split(';')
            .filter_map(|s| s.trim().split_once('='))
            .find(|(k, _)| *k == "loom_console")
            .map(|(_, v)| v)
            .unwrap_or("");
        if !app
            .auth
            .lock()
            .unwrap()
            .sessions
            .get(&hash(cookie))
            .is_some_and(|t| t.elapsed() < Duration::from_secs(8 * 3600))
        {
            return error(401, "OPEN_WITH_LOOM_CONSOLE");
        }
        if method != "GET" && origin.is_none() {
            return error(403, "ORIGIN_REQUIRED");
        }
        let store = app.store.clone();
        let data = app.data.clone();
        if path == "/api/settings" && method == "POST" {
            let bytes = match to_bytes(req.into_body(), 2048).await {
                Ok(v) => v,
                Err(_) => return error(400, "INVALID_SETTINGS"),
            };
            let v: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
            if !v.is_object()
                || v.as_object().unwrap().keys().any(|k| {
                    !["capture_bodies", "retention_days", "import_sessions"].contains(&k.as_str())
                })
                || v.get("retention_days")
                    .is_some_and(|v| !v.as_u64().is_some_and(|n| (1..=3650).contains(&n)))
                || ["capture_bodies", "import_sessions"]
                    .iter()
                    .any(|k| v.get(*k).is_some_and(|v| !v.is_boolean()))
            {
                return error(400, "INVALID_SETTINGS");
            }
            for (k, v) in v.as_object().unwrap() {
                if store.set_setting(k, v).is_err() {
                    return error(500, "STORE_ERROR");
                }
            }
            Json(json!({"ok":true})).into_response()
        } else {
            let route = path.clone();
            let operation = method.clone();
            let result=tokio::task::spawn_blocking(move ||->Result<Value>{
                let get=|k:&str|q.get(k).map(String::as_str).unwrap_or("");
                let f=Filter{from:get("from").parse().unwrap_or(0),to:get("to").parse().unwrap_or(i64::MAX),provider:get("provider").into(),model:get("model").into(),harness:get("harness").into(),limit:get("limit").parse().unwrap_or(50),offset:get("offset").parse().unwrap_or(0)};
                match (operation.as_str(),route.as_str()) {
                    ("GET","/api/overview")=>store.stats(&f),
                    ("GET","/api/requests")=>store.requests(&f),
                    ("GET","/api/sessions")=>store.sessions(get("harness"),get("q"),f.offset),
                    ("GET","/api/settings")=>Ok(json!({"capture_bodies":store.setting("capture_bodies",json!(true)),"retention_days":store.setting("retention_days",json!(30)),"import_sessions":store.setting("import_sessions",json!(true)),"last_import":store.setting("last_import",Value::Null),"version":env!("CARGO_PKG_VERSION"),"database_bytes":std::fs::metadata(store.dir.join("traces.sqlite3")).map(|m|m.len()).unwrap_or(0)})),
                    ("POST","/api/import")=>harness::import(&store,&config::home(),&data),
                    ("POST","/api/prune")=>Ok(json!({"removed_requests":store.retention()?})),
                    _ if operation=="GET"&&route.starts_with("/api/requests/")=>store.request(route.trim_start_matches("/api/requests/")),
                    _ if operation=="GET"&&route.starts_with("/api/sessions/")=>store.session(route.trim_start_matches("/api/sessions/"),f.offset),
                    _=>Err("NOT_FOUND".into())
                }
            }).await;
            match result {
                Ok(Ok(v)) => Json(v).into_response(),
                _ => error(404, "NOT_FOUND_OR_UNAVAILABLE"),
            }
        }
    };
    let headers = result.headers_mut();
    for (k, v) in [
        ("cache-control", "no-store"),
        ("x-content-type-options", "nosniff"),
        ("x-frame-options", "DENY"),
        ("cross-origin-resource-policy", "same-origin"),
        ("referrer-policy", "no-referrer"),
        (
            "content-security-policy",
            "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'",
        ),
    ] {
        headers.insert(
            axum::http::HeaderName::from_static(k),
            axum::http::HeaderValue::from_static(v),
        );
    }
    result
}
pub async fn open_console(data: &std::path::Path, port: u16) -> Result<()> {
    let key = std::fs::read_to_string(data.join("hub.key"))?;
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let v: Value = client
        .post(format!("http://127.0.0.1:{port}/api/launch"))
        .bearer_auth(key.trim())
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let ticket = v["ticket"].as_str().ok_or("ticket missing")?;
    let url = format!("http://127.0.0.1:{port}/#ticket={ticket}");
    let command = if cfg!(target_os = "macos") {
        "/usr/bin/open"
    } else {
        "xdg-open"
    };
    if !std::process::Command::new(command)
        .arg(url)
        .status()?
        .success()
    {
        return Err("无法打开本机浏览器".into());
    }
    println!("已打开 Loom 本机管控台：http://127.0.0.1:{port}/；登录票据不输出。");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use tower::ServiceExt;
    fn request(
        method: &str,
        path: &str,
        origin: Option<&str>,
        auth: Option<&str>,
        body: Value,
    ) -> Request {
        let mut r = Request::builder()
            .method(method)
            .uri(path)
            .header("host", "127.0.0.1:4320")
            .header("content-type", "application/json");
        if let Some(o) = origin {
            r = r.header("origin", o);
        }
        if let Some(a) = auth {
            r = r.header("authorization", a);
        }
        r.body(axum::body::Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap()
    }
    #[tokio::test]
    async fn console_requires_one_time_login_same_origin_and_never_exposes_hub_key() {
        let d = tempfile::tempdir().unwrap();
        config::init(d.path()).unwrap();
        let key = std::fs::read_to_string(d.path().join("hub.key")).unwrap();
        let store = Store::open(d.path()).unwrap();
        let app = router(App::new(d.path().into(), store, 4320));
        assert_eq!(
            app.clone()
                .oneshot(request("GET", "/api/overview", None, None, Value::Null))
                .await
                .unwrap()
                .status(),
            401
        );
        let authorization = format!("Bearer {}", key.trim());
        assert_eq!(
            app.clone()
                .oneshot(request(
                    "POST",
                    "/api/launch",
                    Some("http://evil.invalid"),
                    Some(&authorization),
                    Value::Null
                ))
                .await
                .unwrap()
                .status(),
            403
        );
        let launch = app
            .clone()
            .oneshot(request(
                "POST",
                "/api/launch",
                None,
                Some(&authorization),
                Value::Null,
            ))
            .await
            .unwrap();
        assert_eq!(launch.status(), 200);
        let v: Value =
            serde_json::from_slice(&to_bytes(launch.into_body(), 65536).await.unwrap()).unwrap();
        let login = json!({"ticket":v["ticket"]});
        let response = app
            .clone()
            .oneshot(request(
                "POST",
                "/api/login",
                Some("http://127.0.0.1:4320"),
                None,
                login.clone(),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let cookie = response.headers()["set-cookie"]
            .to_str()
            .unwrap()
            .to_string();
        assert!(cookie.contains("HttpOnly") && cookie.contains("SameSite=Strict"));
        assert!(!cookie.contains(key.trim()));
        assert_eq!(
            app.clone()
                .oneshot(request(
                    "POST",
                    "/api/login",
                    Some("http://127.0.0.1:4320"),
                    None,
                    login
                ))
                .await
                .unwrap()
                .status(),
            401
        );
        let mut stats = request("GET", "/api/overview", None, None, Value::Null);
        stats
            .headers_mut()
            .insert("cookie", cookie.split(';').next().unwrap().parse().unwrap());
        let response = app.clone().oneshot(stats).await.unwrap();
        assert_eq!(response.status(), 200);
        let data = to_bytes(response.into_body(), 65536).await.unwrap();
        assert!(!String::from_utf8_lossy(&data).contains(key.trim()));
        let mut bad = request("GET", "/api/overview", None, None, Value::Null);
        bad.headers_mut()
            .insert("host", "attacker.invalid:4320".parse().unwrap());
        assert_eq!(app.oneshot(bad).await.unwrap().status(), 403);
    }
}
