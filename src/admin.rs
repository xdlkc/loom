//! Loopback-only console, separate from inference. Browser credentials are short lived.
use crate::{
    config::{self, Result},
    harness, manage, network,
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
    pub home: PathBuf,
    pub store: Store,
    pub port: u16,
    pub gateway_port: u16,
    auth: Arc<Mutex<Auth>>,
}
impl App {
    pub fn new(data: PathBuf, store: Store, port: u16, gateway_port: u16) -> Self {
        Self::new_with_home(data, config::home(), store, port, gateway_port)
    }
    pub fn new_with_home(
        data: PathBuf,
        home: PathBuf,
        store: Store,
        port: u16,
        gateway_port: u16,
    ) -> Self {
        Self {
            data,
            home,
            store,
            port,
            gateway_port,
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
        Json(json!({"error": code})),
    )
        .into_response()
}
fn error_msg(status: u16, code: &str, message: &str) -> Response {
    (
        StatusCode::from_u16(status).unwrap(),
        Json(json!({"error": code, "message": message})),
    )
        .into_response()
}
fn safe_message(e: &(dyn std::error::Error + Send + Sync)) -> String {
    let s = e.to_string();
    if s.contains("opencode.jsonc contains comments") {
        return "OpenCode 配置文件 opencode.jsonc 含有注释，Loom 已保护性跳过覆盖；请先移除注释或改用 opencode.json。".into();
    }
    if s.contains("both exist") {
        return "检测到 opencode.json 与 opencode.jsonc 同时存在，Loom 已拒绝覆盖；请保留其中一份后重试。".into();
    }
    if s.contains("add a provider first") {
        return "请先添加并启用至少一个模型厂商，再同步客户端。".into();
    }
    // Only surface authored non-ASCII validation guidance; hide raw OS/filesystem errors.
    if !s.is_ascii() && !s.contains('/') && !s.contains('\\') {
        return s;
    }
    "操作失败，请检查配置参数或本机文件权限。".into()
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
/// `Uri::path()` arrives as sent: browsers encode the `:` in `harness:native` session ids.
fn decode_path(raw: &str) -> String {
    fn hex(c: u8) -> Option<u8> {
        (c as char).to_digit(16).map(|d| d as u8)
    }
    let b = raw.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let (Some(high), Some(low)) = (hex(b[i + 1]), hex(b[i + 2]))
        {
            out.push(high * 16 + low);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
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
        format!("loom.localhost:{}", app.port),
    ];
    let public_navigation = method == "GET"
        && req.uri() == "/"
        && text(&h, "sec-fetch-mode").as_deref() == Some("navigate")
        && text(&h, "sec-fetch-dest").as_deref() == Some("document");
    if !host.as_ref().is_some_and(|h| hosts.contains(h))
        || origin
            .as_ref()
            .is_some_and(|s| host.as_ref().is_none_or(|h| s != &format!("http://{h}")))
        || (text(&h, "sec-fetch-site").as_deref() == Some("cross-site") && !public_navigation)
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
        let home = app.home.clone();
        let gateway_port = app.gateway_port;
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
        } else if path == "/api/setup" && method == "GET" {
            match manage::setup_snapshot(&data, &home, gateway_port, None).await {
                Ok(v) => Json(v).into_response(),
                Err(e) => error_msg(500, "SETUP_READ_FAILED", &safe_message(&*e)),
            }
        } else if path == "/api/providers/test" && method == "POST" {
            let Ok(bytes) = to_bytes(req.into_body(), 65536).await else {
                return error_msg(400, "INVALID_REQUEST", "请求体格式无效或过大。");
            };
            let Ok(v) = serde_json::from_slice::<Value>(&bytes) else {
                return error_msg(400, "INVALID_JSON", "请提交有效的 JSON 参数。");
            };
            let Some(pid) = v["provider_id"]
                .as_str()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            else {
                return error_msg(400, "PROVIDER_REQUIRED", "请指定要检测的供应商 ID。");
            };
            let model = v["model_id"].as_str().map(String::from);
            match manage::verify_provider(
                &data,
                pid,
                model.as_deref(),
                #[cfg(test)]
                None,
            )
            .await
            {
                Ok(res) => Json(res).into_response(),
                Err(e) => error_msg(400, "VERIFY_FAILED", &safe_message(&*e)),
            }
        } else if method == "POST"
            && [
                "/api/providers/preset",
                "/api/providers/custom",
                "/api/providers/update",
                "/api/providers/delete",
                "/api/proxy",
                "/api/sync",
            ]
            .contains(&path.as_str())
        {
            let Ok(bytes) = to_bytes(req.into_body(), 65536).await else {
                return error_msg(400, "INVALID_REQUEST", "请求体格式无效或过大。");
            };
            let v: Value = if bytes.is_empty() {
                json!({})
            } else if let Ok(parsed) = serde_json::from_slice(&bytes) {
                parsed
            } else {
                return error_msg(400, "INVALID_JSON", "请提交有效的 JSON 参数。");
            };
            let route = path.clone();
            let op = tokio::task::spawn_blocking(move || -> Result<Value> {
                match route.as_str() {
                    "/api/providers/preset" => {
                        let pid = v["preset_id"].as_str().unwrap_or("");
                        let key = v["api_key"].as_str();
                        let enabled = v.get("enabled").and_then(Value::as_bool);
                        manage::upsert_preset(&data, &home, gateway_port, pid, key, enabled)
                    }
                    "/api/providers/custom" => {
                        manage::upsert_custom(&data, &home, gateway_port, &v)
                    }
                    "/api/providers/update" => {
                        manage::update_provider(&data, &home, gateway_port, &v)
                    }
                    "/api/providers/delete" => {
                        let pid = v["provider_id"].as_str().unwrap_or("");
                        manage::delete_provider(&data, &home, gateway_port, pid)
                    }
                    "/api/proxy" => {
                        let url = v["url"].as_str().unwrap_or("");
                        let configured = network::set_proxy(&data, url).map_err(|_| {
                            "代理地址须为 HTTP/HTTPS URL，不含用户名、密码、查询参数或自定义路径。"
                        })?;
                        Ok(json!({"ok": true, "configured": configured}))
                    }
                    "/api/sync" => {
                        let reg = config::read(&data.join("providers.json"))?;
                        if config::catalog(&reg)?.is_empty() {
                            return Err("请先添加并启用至少一个模型厂商，再同步客户端。".into());
                        }
                        config::sync_for_port(&data, &home, gateway_port)?;
                        Ok(json!({
                            "ok": true,
                            "clients": manage::inspect_clients(&home, gateway_port)
                        }))
                    }
                    _ => Err("NOT_FOUND".into()),
                }
            })
            .await;
            match op {
                Ok(Ok(res)) => Json(res).into_response(),
                Ok(Err(e)) => error_msg(400, "ACTION_FAILED", &safe_message(&*e)),
                Err(_) => error_msg(500, "TASK_FAILED", "后台处理异常，请重试。"),
            }
        } else {
            let route = path.clone();
            let operation = method.clone();
            let result=tokio::task::spawn_blocking(move ||->Result<Value>{
                let get=|k:&str|q.get(k).map(String::as_str).unwrap_or("");
                let f=Filter{from:get("from").parse().unwrap_or(0),to:get("to").parse().unwrap_or(i64::MAX),provider:get("provider").into(),model:get("model").into(),harness:get("harness").into(),status:get("status").into(),limit:get("limit").parse().unwrap_or(50),offset:get("offset").parse().unwrap_or(0)};
                match (operation.as_str(),route.as_str()) {
                    ("GET","/api/overview")=>store.stats(&f),
                    ("GET","/api/requests")=>store.requests(&f),
                    ("GET","/api/sessions")=>store.sessions(get("harness"),get("q"),f.offset),
                    ("GET","/api/settings")=>Ok(json!({"capture_bodies":store.setting("capture_bodies",json!(true)),"retention_days":store.setting("retention_days",json!(30)),"import_sessions":store.setting("import_sessions",json!(true)),"last_import":store.setting("last_import",Value::Null),"version":env!("CARGO_PKG_VERSION"),"database_bytes":std::fs::metadata(store.dir.join("traces.sqlite3")).map(|m|m.len()).unwrap_or(0)})),
                    ("POST","/api/import")=>harness::import(&store,&home,&data),
                    ("POST","/api/prune")=>Ok(json!({"removed_requests":store.retention()?})),
                    _ if operation=="GET"&&route.starts_with("/api/requests/")=>store.request(&decode_path(route.trim_start_matches("/api/requests/"))),
                    _ if operation=="GET"&&route.starts_with("/api/sessions/")=>store.session(&decode_path(route.trim_start_matches("/api/sessions/")),f.offset),
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
pub async fn open_console(
    data: &std::path::Path,
    port: u16,
    host: &str,
    page: Option<&str>,
) -> Result<()> {
    if !["loom.localhost", "localhost", "127.0.0.1"].contains(&host) {
        return Err("管控台仅支持本机域名".into());
    }
    let valid_page = page.filter(|p| {
        [
            "onboarding",
            "providers",
            "integrations",
            "overview",
            "requests",
            "sessions",
            "settings",
        ]
        .contains(p)
    });
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
    let url = match valid_page {
        Some(p) => format!("http://{host}:{port}/#ticket={ticket}&page={p}"),
        None => format!("http://{host}:{port}/#ticket={ticket}"),
    };
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
    println!("已打开 Loom 本机管控台：http://{host}:{port}/；登录票据不输出。");
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
    #[test]
    fn decode_path_decodes_escapes_and_leaves_malformed_ones_alone() {
        for (raw, decoded) in [
            ("claude-code%3Aabc", "claude-code:abc"),
            ("a%2Fb%20c%2Bd", "a/b c+d"),
            ("%E4%B8%AD", "中"),
            ("plain-id_1.2", "plain-id_1.2"),
            ("%ZZ", "%ZZ"),
            ("%2", "%2"),
            ("trailing%", "trailing%"),
            ("100%", "100%"),
            // A truncated UTF-8 sequence cannot be restored; it becomes one replacement character.
            ("%C3", "\u{FFFD}"),
        ] {
            assert_eq!(decode_path(raw), decoded, "{raw}");
        }
    }
    #[tokio::test]
    async fn console_requires_one_time_login_same_origin_and_never_exposes_hub_key() {
        let t = tempfile::tempdir().unwrap();
        let d = t.path().join("data");
        let h = t.path().join("home");
        config::init(&d).unwrap();
        let key = std::fs::read_to_string(d.join("hub.key")).unwrap();
        let store = Store::open(&d).unwrap();
        let app = router(App::new_with_home(d, h, store, 4320, 4319));
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

    #[tokio::test]
    async fn local_domain_login_works_and_rejects_other_local_origins() {
        let t = tempfile::tempdir().unwrap();
        let d = t.path().join("data");
        let h = t.path().join("home");
        config::init(&d).unwrap();
        let key = std::fs::read_to_string(d.join("hub.key")).unwrap();
        let store = Store::open(&d).unwrap();
        let app = router(App::new_with_home(d, h, store, 4320, 4319));
        let authorization = format!("Bearer {}", key.trim());
        let response = app
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
        let v: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap();
        let mut login = request(
            "POST",
            "/api/login",
            Some("http://loom.localhost:4320"),
            None,
            json!({"ticket":v["ticket"]}),
        );
        login
            .headers_mut()
            .insert("host", "loom.localhost:4320".parse().unwrap());
        let response = app.clone().oneshot(login).await.unwrap();
        assert_eq!(response.status(), 200);
        let cookie = response.headers()["set-cookie"]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string();
        for (host, origin, expected) in [
            ("loom.localhost:4320", "http://loom.localhost:4320", 200),
            ("loom.localhost:4320", "http://localhost:4320", 403),
            ("loom.localhost:4320", "http://127.0.0.1:4320", 403),
            ("loom.localhost:4320", "http://loom.localhost:4321", 403),
            ("other.localhost:4320", "http://other.localhost:4320", 403),
        ] {
            let mut req = request("GET", "/api/overview", Some(origin), None, Value::Null);
            req.headers_mut().insert("host", host.parse().unwrap());
            req.headers_mut().insert("cookie", cookie.parse().unwrap());
            assert_eq!(
                app.clone().oneshot(req).await.unwrap().status().as_u16(),
                expected
            );
        }
        for (path, expected) in [("/", 200), ("/api/overview", 403)] {
            let mut req = request("GET", path, None, None, Value::Null);
            for (key, value) in [
                ("host", "loom.localhost:4320"),
                ("sec-fetch-site", "cross-site"),
                ("sec-fetch-mode", "navigate"),
                ("sec-fetch-dest", "document"),
            ] {
                req.headers_mut().insert(
                    axum::http::HeaderName::from_static(key),
                    value.parse().unwrap(),
                );
            }
            assert_eq!(
                app.clone().oneshot(req).await.unwrap().status().as_u16(),
                expected
            );
        }
    }

    #[tokio::test]
    async fn console_management_apis_validate_auth_and_preserve_credentials() {
        let t = tempfile::tempdir().unwrap();
        let d = t.path().join("data");
        let h = t.path().join("home");
        config::init(&d).unwrap();
        let key = std::fs::read_to_string(d.join("hub.key")).unwrap();
        let store = Store::open(&d).unwrap();
        let app = router(App::new_with_home(d.clone(), h.clone(), store, 4320, 14319));

        // Unauthenticated setup read & mutations must fail with 401.
        assert_eq!(
            app.clone()
                .oneshot(request("GET", "/api/setup", None, None, Value::Null))
                .await
                .unwrap()
                .status(),
            401
        );
        assert_eq!(
            app.clone()
                .oneshot(request(
                    "POST",
                    "/api/providers/preset",
                    Some("http://127.0.0.1:4320"),
                    None,
                    json!({"preset_id": "deepseek", "api_key": "SECRET"})
                ))
                .await
                .unwrap()
                .status(),
            401
        );

        let authorization = format!("Bearer {}", key.trim());
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
        let v: Value =
            serde_json::from_slice(&to_bytes(launch.into_body(), 65536).await.unwrap()).unwrap();
        let login = app
            .clone()
            .oneshot(request(
                "POST",
                "/api/login",
                Some("http://127.0.0.1:4320"),
                None,
                json!({"ticket": v["ticket"]}),
            ))
            .await
            .unwrap();
        let cookie = login.headers()["set-cookie"]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string();

        let authed = |method: &str, path: &str, origin: Option<&str>, body: Value| {
            let mut r = request(method, path, origin, None, body);
            r.headers_mut().insert("cookie", cookie.parse().unwrap());
            r
        };

        // Cross-origin or missing Origin on POST must be rejected (403).
        assert_eq!(
            app.clone()
                .oneshot(authed(
                    "POST",
                    "/api/providers/preset",
                    None,
                    json!({"preset_id": "deepseek", "api_key": "SECRET"})
                ))
                .await
                .unwrap()
                .status(),
            403
        );
        assert_eq!(
            app.clone()
                .oneshot(authed(
                    "POST",
                    "/api/providers/preset",
                    Some("http://evil.invalid"),
                    json!({"preset_id": "deepseek", "api_key": "SECRET"})
                ))
                .await
                .unwrap()
                .status(),
            403
        );

        // Save preset, update proxy, verify setup never leaks secrets.
        let save_res = app
            .clone()
            .oneshot(authed(
                "POST",
                "/api/providers/preset",
                Some("http://127.0.0.1:4320"),
                json!({"preset_id": "deepseek", "api_key": "UPSTREAM_ADMIN_FIXTURE"}),
            ))
            .await
            .unwrap();
        assert_eq!(save_res.status(), 200);
        assert_eq!(
            std::fs::read_to_string(d.join("keys/local-deepseek.key")).unwrap(),
            "UPSTREAM_ADMIN_FIXTURE"
        );
        assert!(h.join(".pi/agent/models.json").is_file());

        let proxy_res = app
            .clone()
            .oneshot(authed(
                "POST",
                "/api/proxy",
                Some("http://127.0.0.1:4320"),
                json!({"url": "http://127.0.0.1:17890"}),
            ))
            .await
            .unwrap();
        assert_eq!(proxy_res.status(), 200);

        let setup_res = app
            .clone()
            .oneshot(authed("GET", "/api/setup", None, Value::Null))
            .await
            .unwrap();
        assert_eq!(setup_res.status(), 200);
        let setup_bytes = to_bytes(setup_res.into_body(), 256 * 1024).await.unwrap();
        let setup_text = String::from_utf8_lossy(&setup_bytes);
        assert!(!setup_text.contains("UPSTREAM_ADMIN_FIXTURE"));
        assert!(!setup_text.contains(key.trim()));
        assert!(!setup_text.contains("17890"));
        let setup_json: Value = serde_json::from_slice(&setup_bytes).unwrap();
        assert_eq!(setup_json["needs_setup"], false);
        assert_eq!(setup_json["proxy"]["configured"], true);

        // Sync, add custom provider, delete custom provider.
        let custom_res = app
            .clone()
            .oneshot(authed(
                "POST",
                "/api/providers/custom",
                Some("http://127.0.0.1:4320"),
                json!({
                    "prefix": "custom-test",
                    "name": "Custom Test",
                    "base_url": "https://custom.invalid/v1",
                    "models_text": "test-m1",
                    "api_key": "CUSTOM_KEY_FIXTURE"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(custom_res.status(), 200);

        let del_res = app
            .clone()
            .oneshot(authed(
                "POST",
                "/api/providers/delete",
                Some("http://127.0.0.1:4320"),
                json!({"provider_id": "local-custom-test"}),
            ))
            .await
            .unwrap();
        assert_eq!(del_res.status(), 200);
        assert!(!d.join("keys/local-custom-test.key").exists());
    }

    /// Exchanges hub.key for a console cookie the way `loom console` does.
    async fn console_cookie(app: axum::Router, key: &str) -> String {
        let launch = app
            .clone()
            .oneshot(request(
                "POST",
                "/api/launch",
                None,
                Some(&format!("Bearer {}", key.trim())),
                Value::Null,
            ))
            .await
            .unwrap();
        let v: Value =
            serde_json::from_slice(&to_bytes(launch.into_body(), 65536).await.unwrap()).unwrap();
        let login = app
            .oneshot(request(
                "POST",
                "/api/login",
                Some("http://127.0.0.1:4320"),
                None,
                json!({"ticket": v["ticket"]}),
            ))
            .await
            .unwrap();
        login.headers()["set-cookie"]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string()
    }
    fn get(path: &str, cookie: &str) -> Request {
        let mut r = request("GET", path, None, None, Value::Null);
        r.headers_mut().insert("cookie", cookie.parse().unwrap());
        r
    }

    #[tokio::test]
    async fn console_decodes_percent_encoded_request_and_session_ids() {
        let t = tempfile::tempdir().unwrap();
        let d = t.path().join("data");
        let h = t.path().join("home");
        config::init(&d).unwrap();
        let key = std::fs::read_to_string(d.join("hub.key")).unwrap();
        let store = Store::open(&d).unwrap();
        let state = App::new_with_home(d, h, store, 4320, 4319);
        // Native imports use `harness:native` ids, and browsers encode `:` as %3A in a path.
        {
            let c = state.store.conn.lock().unwrap();
            c.execute(
                "INSERT INTO sessions(id,harness,native_id,started,updated) VALUES('claude-code:fixture','claude-code','fixture',1,1)",
                [],
            )
            .unwrap();
            c.execute(
                "INSERT INTO requests(id,started,status,harness,http,detail) VALUES('req:fixture',1,'success','pi',200,'{}')",
                [],
            )
            .unwrap();
        }
        let app = router(state);
        let cookie = console_cookie(app.clone(), &key).await;

        let response = app
            .clone()
            .oneshot(get("/api/sessions/claude-code%3Afixture?offset=0", &cookie))
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let v: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap();
        assert_eq!(v["session"]["id"], "claude-code:fixture");

        let response = app
            .oneshot(get("/api/requests/req%3Afixture", &cookie))
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
    }

    #[tokio::test]
    async fn console_requests_route_honours_the_status_filter() {
        let t = tempfile::tempdir().unwrap();
        let d = t.path().join("data");
        let h = t.path().join("home");
        config::init(&d).unwrap();
        let key = std::fs::read_to_string(d.join("hub.key")).unwrap();
        let store = Store::open(&d).unwrap();
        let mut f = crate::telemetry::Flight::start(store.clone(), &d, &HeaderMap::new(), "openai");
        f.headers(401);
        f.finish(Some("UPSTREAM_401"));
        let app = router(App::new_with_home(d, h, store, 4320, 4319));
        let cookie = console_cookie(app.clone(), &key).await;
        for (status, total) in [("failed", 1), ("success", 0), ("error", 1), ("", 1)] {
            let response = app
                .clone()
                .oneshot(get(&format!("/api/requests?status={status}"), &cookie))
                .await
                .unwrap();
            assert_eq!(response.status(), 200);
            let v: Value =
                serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap())
                    .unwrap();
            assert_eq!(v["total"], total, "status={status:?}");
        }
    }
}
