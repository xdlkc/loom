use crate::config::{self, Result};
use crate::telemetry::{Flight, Store};
use axum::{
    Json, Router,
    body::{Body, to_bytes},
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use futures_util::StreamExt;
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Arc, time::Duration};
use subtle::ConstantTimeEq;
#[derive(Clone)]
pub struct App {
    pub telemetry: Option<Store>,
    #[cfg(test)]
    pub upstream_override: Option<String>,
    pub dir: PathBuf,
    pub port: u16,
    pub permits: Arc<tokio::sync::Semaphore>,
}
fn err(native: bool, status: u16, code: &str) -> Response {
    err_message(native, status, code, code)
}
fn err_message(native: bool, status: u16, code: &str, message: &str) -> Response {
    let kind = match status {
        401 => "authentication_error",
        403 => "permission_error",
        429 => "rate_limit_error",
        500.. => "api_error",
        _ => "invalid_request_error",
    };
    (
        StatusCode::from_u16(status).unwrap(),
        Json(if native {
            json!({"type":"error","error":{"type":kind,"message":message}})
        } else {
            json!({"error":{"type":"local_hub_error","message":message,"code":code}})
        }),
    )
        .into_response()
}
// Never reflect upstream prose, arbitrary error types, or credential-bearing URLs.
// Only known machine-readable billing categories select a fixed explanation.
fn upstream_error(native: bool, status: u16, body: &Value) -> Response {
    let code = format!("UPSTREAM_{status}");
    if status != 402 {
        return err(native, status, &code);
    }
    let categories: Vec<&str> = ["/error/type", "/error/code", "/type", "/code"]
        .iter()
        .filter_map(|p| body.pointer(p).and_then(Value::as_str))
        .collect();
    let quota = ["quote_exceeded", "quota_exceeded", "usage_limit_reached"];
    let balance = [
        "insufficient_credit",
        "reject_no_credit",
        "insufficient_balance",
        "insufficient_balance_error",
    ];
    let message = if categories.iter().any(|v| quota.contains(v)) {
        "UPSTREAM_402 [subscription_quota_exhausted]: 上游套餐额度已用尽，请在对应账号查看窗口重置时间。钱包有余额不代表套餐仍有额度。"
    } else if categories.iter().any(|v| balance.contains(v)) {
        "UPSTREAM_402 [insufficient_credit]: 当前 Key 对应计费账户余额不足或欠费。请核对 Key 的计费范围，钱包余额与订阅额度分别计算。"
    } else {
        "UPSTREAM_402 [payment_required]: 上游返回计费或额度错误，具体原因尚未确认。请在 loom console 的请求详情核对上游错误、账号和 Key 所属通道。"
    };
    err_message(native, status, &code, message)
}
fn recorded_error(trace: &mut Option<Flight>, native: bool, status: u16, code: &str) -> Response {
    if let Some(t) = trace {
        t.fail(status, code);
    }
    err(native, status, code)
}
fn transport_error(
    trace: &mut Option<Flight>,
    native: bool,
    status: u16,
    e: &reqwest::Error,
    proxy_configured: bool,
) -> Response {
    let (category, hint) = crate::network::failure(e);
    if let Some(t) = trace {
        t.error_response(
            json!({"transport_error":{"category":category,"proxy_configured":proxy_configured}}),
        );
        t.fail(status, "UPSTREAM_UNAVAILABLE");
    }
    err_message(
        native,
        status,
        "UPSTREAM_UNAVAILABLE",
        &format!("UPSTREAM_UNAVAILABLE [{category}]: {hint}"),
    )
}
fn header<'a>(h: &'a HeaderMap, k: &str) -> Option<&'a str> {
    h.get(k).and_then(|v| v.to_str().ok())
}
fn equal(a: &str, b: &str) -> bool {
    a.as_bytes().ct_eq(b.as_bytes()).into()
}
fn auth(h: &HeaderMap, key: &str, native: bool) -> bool {
    let raw = header(h, "authorization");
    let bearer = raw.and_then(|s| s.strip_prefix("Bearer "));
    let api = if native { header(h, "x-api-key") } else { None };
    if raw.is_some() && bearer.is_none() {
        return false;
    }
    if bearer.is_some_and(|v| !equal(v, key)) || api.is_some_and(|v| !equal(v, key)) {
        return false;
    }
    bearer.is_some_and(|v| equal(v, key)) || api.is_some_and(|v| equal(v, key))
}
pub fn normalize(v: &mut Value) -> Result<()> {
    if !v["max_tokens"].as_u64().is_some_and(|n| n > 0) {
        return Err("INVALID_MESSAGES_REQUEST".into());
    }
    let msgs = v["messages"].as_array().ok_or("INVALID_MESSAGES_REQUEST")?;
    let blocks = |v: &Value| -> Result<Vec<Value>> {
        if let Some(s) = v.as_str() {
            Ok(vec![json!({"type":"text","text":s})])
        } else {
            v.as_array()
                .cloned()
                .ok_or("INVALID_MESSAGES_REQUEST".into())
        }
    };
    let mut extra = vec![];
    let mut messages = vec![];
    for m in msgs {
        if m["role"] == "system" {
            extra.extend(blocks(&m["content"])?);
        } else {
            if m["role"] != "user" && m["role"] != "assistant" {
                return Err("INVALID_MESSAGES_REQUEST".into());
            }
            if !m["content"].is_string() && !m["content"].as_array().is_some_and(|a| !a.is_empty())
            {
                return Err("INVALID_MESSAGES_REQUEST".into());
            }
            messages.push(m.clone());
        }
    }
    if messages.is_empty() {
        return Err("INVALID_MESSAGES_REQUEST".into());
    }
    if v.get("system").is_some() && !v["system"].is_string() && !v["system"].is_array() {
        return Err("INVALID_MESSAGES_REQUEST".into());
    }
    if !extra.is_empty() {
        let mut system = if v.get("system").is_some() {
            blocks(&v["system"])?
        } else {
            vec![]
        };
        system.extend(extra);
        v["system"] = json!(system);
    }
    v["messages"] = json!(messages);
    Ok(())
}
pub fn router(app: App) -> Router {
    Router::new()
        .route("/health", get(handle))
        .route("/v1/models", get(handle))
        .route("/v1/chat/completions", post(handle))
        .route("/v1/messages", post(handle))
        .fallback(handle)
        .with_state(app)
}
async fn handle(State(app): State<App>, req: Request) -> Response {
    let path = req.uri().path().to_string();
    let h = req.headers().clone();
    let native = path == "/v1/messages"
        || h.contains_key("anthropic-version")
        || h.contains_key("x-api-key");
    if ![
        format!("127.0.0.1:{}", app.port),
        format!("localhost:{}", app.port),
    ]
    .iter()
    .any(|v| Some(v.as_str()) == header(&h, "host"))
        || h.contains_key("origin")
        || header(&h, "sec-fetch-site") == Some("cross-site")
    {
        return err(native, 403, "LOCAL_CLIENTS_ONLY");
    }
    if path == "/health" && req.method() == "GET" {
        return Json(json!({"ok":true,"service":"local-model-hub","runtime":"rust"}))
            .into_response();
    }
    let mut trace = if req.method() == "POST" {
        app.telemetry.clone().map(|store| {
            Flight::start(
                store,
                &app.dir,
                &h,
                if native { "anthropic" } else { "openai" },
            )
        })
    } else {
        None
    };
    let key = match tokio::fs::read_to_string(app.dir.join("hub.key")).await {
        Ok(v) => v,
        Err(_) => return recorded_error(&mut trace, native, 503, "LOCAL_KEY_UNAVAILABLE"),
    };
    if !auth(&h, key.trim(), native) {
        return recorded_error(&mut trace, native, 401, "LOCAL_KEY_REQUIRED");
    }
    if let Some(t) = &mut trace {
        t.stage("authenticated");
    }
    let registry = match config::read(&app.dir.join("providers.json")) {
        Ok(v) => v,
        Err(_) => return recorded_error(&mut trace, native, 503, "REGISTRY_INVALID"),
    };
    let models = match config::catalog(&registry) {
        Ok(v) => v,
        Err(_) => return recorded_error(&mut trace, native, 503, "REGISTRY_INVALID"),
    };
    if path == "/v1/models" && req.method() == "GET" {
        let data:Vec<_>=models.iter().filter(|m|!native||config::anthropic(&m.p).ok().flatten().is_some()).map(|m|if native{json!({"id":m.id,"type":"model","display_name":m.m["name"],"created_at":"1970-01-01T00:00:00Z"})}else{json!({"id":m.id,"object":"model","owned_by":m.p["name"],"name":m.m["name"],"billing":m.p["billing"],"context_window":m.m["contextWindow"],"max_output_tokens":m.m["maxTokens"]})}).collect();
        return Json(if native{json!({"data":data,"has_more":false,"first_id":data.first().map(|v|&v["id"]),"last_id":data.last().map(|v|&v["id"])})}else{json!({"object":"list","data":data})}).into_response();
    }
    let messages = path == "/v1/messages";
    if !["/v1/messages", "/v1/chat/completions"].contains(&path.as_str()) || req.method() != "POST"
    {
        return recorded_error(&mut trace, native, 404, "ENDPOINT_NOT_SUPPORTED");
    }
    if !header(&h, "content-type").is_some_and(|s| s.starts_with("application/json")) {
        return recorded_error(&mut trace, native, 415, "JSON_REQUIRED");
    }
    let permit = match app.permits.clone().try_acquire_owned() {
        Ok(v) => v,
        Err(_) => return recorded_error(&mut trace, native, 429, "LOCAL_CONCURRENCY_LIMIT"),
    };
    let raw = match tokio::time::timeout(
        Duration::from_secs(30),
        to_bytes(req.into_body(), 20 * 1024 * 1024),
    )
    .await
    {
        Ok(Ok(v)) => v,
        _ => return recorded_error(&mut trace, native, 413, "REQUEST_TOO_LARGE_OR_TIMEOUT"),
    };
    let mut body: Value = match serde_json::from_slice(&raw) {
        Ok(v) => v,
        Err(_) => return recorded_error(&mut trace, native, 400, "INVALID_JSON"),
    };
    if let Some(t) = &mut trace {
        t.request_body(&body);
    }
    let route = match models.iter().find(|m| body["model"] == m.id) {
        Some(v) => v,
        None => return recorded_error(&mut trace, native, 400, "UNKNOWN_MODEL"),
    };
    if !body["messages"].as_array().is_some_and(|a| !a.is_empty())
        || body.get("stream").is_some_and(|v| !v.is_boolean())
    {
        return recorded_error(&mut trace, native, 400, "INVALID_CHAT_REQUEST");
    }
    if messages && normalize(&mut body).is_err() {
        return recorded_error(&mut trace, native, 400, "INVALID_MESSAGES_REQUEST");
    }
    if let Some(t) = &mut trace {
        t.stage("routed");
    }
    let url = if messages {
        match config::anthropic(&route.p) {
            Ok(Some(v)) => v,
            _ => {
                return recorded_error(
                    &mut trace,
                    native,
                    400,
                    "ANTHROPIC_NOT_SUPPORTED_BY_PROVIDER",
                );
            }
        }
    } else {
        format!(
            "{}/chat/completions",
            route.p["baseURL"].as_str().unwrap().trim_end_matches('/')
        )
    };
    #[cfg(test)]
    let url = app.upstream_override.clone().unwrap_or(url);
    let max = route.m["maxTokens"].as_u64().unwrap_or(8192);
    let requested = if messages {
        body.get("max_tokens")
    } else {
        body.get("max_completion_tokens").or(body.get("max_tokens"))
    };
    let n = match requested {
        Some(v) => match v.as_u64() {
            Some(n) if n > 0 => n,
            _ => return recorded_error(&mut trace, native, 400, "INVALID_MAX_TOKENS"),
        },
        None => max,
    };
    body["max_tokens"] = json!(n.min(max));
    body["model"] = route.m["id"].clone();
    body.as_object_mut().unwrap().remove("store");
    body.as_object_mut()
        .unwrap()
        .remove("max_completion_tokens");
    if let Some(t) = &mut trace {
        let provider = registry["providers"]
            .as_object()
            .and_then(|p| p.iter().find(|(_, p)| **p == route.p))
            .map(|(id, _)| id.as_str())
            .unwrap_or("unknown");
        t.forwarded(provider, &body);
    }
    let upstream_key = match tokio::fs::read_to_string(route.p["keyFile"].as_str().unwrap()).await {
        Ok(v) => v,
        Err(_) => return recorded_error(&mut trace, native, 503, "PROVIDER_KEY_UNAVAILABLE"),
    };
    let mut builder = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(600));
    let proxy_configured = registry["proxy"].as_str().is_some_and(|v| !v.is_empty());
    if let Some(proxy) = registry["proxy"].as_str().filter(|v| !v.is_empty()) {
        builder = match reqwest::Proxy::all(proxy) {
            Ok(p) => builder.proxy(p),
            Err(_) => return recorded_error(&mut trace, native, 503, "PROXY_INVALID"),
        };
    }
    let client = match builder.build() {
        Ok(v) => v,
        Err(e) => return transport_error(&mut trace, native, 503, &e, proxy_configured),
    };
    let streaming = body["stream"] == true;
    let mut request = client.post(url).json(&body).header(
        if messages {
            "x-api-key"
        } else {
            "authorization"
        },
        if messages {
            upstream_key.trim().to_string()
        } else {
            format!("Bearer {}", upstream_key.trim())
        },
    );
    if messages {
        let version = header(&h, "anthropic-version").unwrap_or("2023-06-01");
        if version != "2023-06-01" {
            return recorded_error(&mut trace, native, 400, "INVALID_ANTHROPIC_HEADERS");
        }
        request = request.header("anthropic-version", version);
        if let Some(beta) = header(&h, "anthropic-beta") {
            if beta.len() > 1024
                || beta.is_empty()
                || !beta
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_,.-".contains(&b))
            {
                return recorded_error(&mut trace, native, 400, "INVALID_ANTHROPIC_HEADERS");
            }
            request = request.header("anthropic-beta", beta);
        }
    }
    if let Some(t) = &mut trace {
        t.stage("dispatch");
    }
    let mut response = match request.send().await {
        Ok(v) => v,
        Err(e) => return transport_error(&mut trace, native, 502, &e, proxy_configured),
    };
    if let Some(t) = &mut trace {
        t.headers(response.status().as_u16());
    }
    if !response.status().is_success() {
        let s = response.status().as_u16();
        let mut bytes = Vec::new();
        let read = tokio::time::timeout(Duration::from_secs(2), async {
            while let Ok(Some(chunk)) = response.chunk().await {
                let count = (16384 - bytes.len()).min(chunk.len());
                bytes.extend_from_slice(&chunk[..count]);
                if bytes.len() >= 16384 {
                    break;
                }
            }
        })
        .await;
        let upstream = serde_json::from_slice::<Value>(&bytes)
            .unwrap_or_else(|_| json!({"non_json":true,"captured_bytes":bytes.len()}));
        if let Some(t) = &mut trace {
            t.error_response(json!({"upstream_error":upstream,"client_error":format!("UPSTREAM_{s}"),"error_body_incomplete":read.is_err()||bytes.len()>=16384}));
            t.fail(s, &format!("UPSTREAM_{s}"));
        }
        return upstream_error(native, s, &upstream);
    }
    let request_id = trace
        .as_ref()
        .map(|t| t.record.id.clone())
        .unwrap_or_default();
    let mut bytes = response.bytes_stream();
    let stream = async_stream::stream! {
        let _permit=permit;
        let mut trace=trace;
        let mut total=0usize;
        while let Some(chunk)=bytes.next().await {
            match chunk {
                Ok(chunk)=> {
                    total+=chunk.len();
                    if total>64*1024*1024 {
                        if let Some(t)=&mut trace {t.finish(Some("RESPONSE_SIZE_LIMIT"));}
                        yield Err(std::io::Error::other("response too large"));break;
                    }
                    if let Some(t)=&mut trace {t.chunk(&chunk);}
                    yield Ok(chunk);
                },
                Err(_)=> {
                    if let Some(t)=&mut trace {t.finish(Some("UPSTREAM_STREAM_ERROR"));}
                    yield Err(std::io::Error::other("upstream stream failed"));break;
                }
            }
        }
        if let Some(t)=&mut trace {t.finish(None);}
    };
    Response::builder()
        .status(200)
        .header("x-loom-request-id", request_id)
        .header(
            "content-type",
            if streaming {
                "text/event-stream; charset=utf-8"
            } else {
                "application/json; charset=utf-8"
            },
        )
        .header("cache-control", "no-store")
        .header("x-accel-buffering", "no")
        .body(Body::from_stream(stream))
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower::ServiceExt;
    #[tokio::test]
    async fn transport_failure_keeps_502_protocol_and_only_records_safe_categories() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let e = reqwest::Client::builder()
            .no_proxy()
            .build()
            .unwrap()
            .get(format!("http://{address}/?key=PRIVATE_TRANSPORT_FIXTURE"))
            .send()
            .await
            .unwrap_err();
        let d = tempfile::tempdir().unwrap();
        config::init(d.path()).unwrap();
        let store = Store::open(d.path()).unwrap();
        for native in [false, true] {
            let mut trace = Some(Flight::start(
                store.clone(),
                d.path(),
                &HeaderMap::new(),
                if native { "anthropic" } else { "openai" },
            ));
            let id = trace.as_ref().unwrap().record.id.clone();
            let response = transport_error(&mut trace, native, 502, &e, true);
            assert_eq!(response.status(), 502);
            let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
            let text = String::from_utf8_lossy(&bytes);
            assert!(text.contains("connect_failed") && text.contains("UPSTREAM_UNAVAILABLE"));
            assert!(
                !text.contains("PRIVATE_TRANSPORT_FIXTURE") && !text.contains(&address.to_string())
            );
            let detail = store.request(&id).unwrap();
            assert_eq!(
                detail["response"]["transport_error"]["category"],
                "connect_failed"
            );
            assert_eq!(
                detail["response"]["transport_error"]["proxy_configured"],
                true
            );
            assert!(!detail.to_string().contains("PRIVATE_TRANSPORT_FIXTURE"));
        }
    }
    #[tokio::test]
    async fn billing_errors_keep_protocol_status_and_never_echo_upstream_secrets() {
        for native in [false, true] {
            for (kind, expected) in [
                ("quote_exceeded", "subscription_quota_exhausted"),
                ("reject_no_credit", "insufficient_credit"),
                ("fixture-private-type", "payment_required"),
            ] {
                let response = upstream_error(
                    native,
                    402,
                    &json!({"error":{"type":kind,"message":"Bearer sk-fixture-private-secret https://private.example/?api_key=fixture-private-secret"}}),
                );
                assert_eq!(response.status(), 402);
                let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
                let text = String::from_utf8(bytes.to_vec()).unwrap();
                assert!(!text.contains("fixture-private") && !text.contains("private.example"));
                let v: Value = serde_json::from_slice(&bytes).unwrap();
                assert!(v["error"]["message"].as_str().unwrap().contains(expected));
                assert!(
                    v["error"]["message"]
                        .as_str()
                        .unwrap()
                        .contains("UPSTREAM_402")
                );
                if native {
                    assert_eq!(v["type"], "error");
                } else {
                    assert_eq!(v["error"]["code"], "UPSTREAM_402");
                }
            }
        }
    }
    #[tokio::test]
    async fn billing_cause_is_available_without_body_capture_or_telemetry() {
        let d = tempfile::tempdir().unwrap();
        config::init(d.path()).unwrap();
        let keyfile = d.path().join("upstream.key");
        config::write(&keyfile, b"UPSTREAM_FIXTURE").unwrap();
        config::write(&d.path().join("providers.json"), &serde_json::to_vec(&json!({"proxy":"","providers":{"test":{"enabled":true,"prefix":"test","name":"Test","baseURL":"https://example.invalid/v1","anthropicBaseURL":"https://example.invalid","keyFile":keyfile,"models":[{"id":"coder","maxTokens":64}]}}})).unwrap()).unwrap();
        let upstream = Router::new().fallback(post(|| async {
            (
                StatusCode::PAYMENT_REQUIRED,
                Json(json!({"error":{"type":"quote_exceeded","message":"UPSTREAM_FIXTURE"}})),
            )
        }));
        let bind = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = bind.local_addr().unwrap();
        let task = tokio::spawn(async move { axum::serve(bind, upstream).await.unwrap() });
        let app = router(App {
            telemetry: None,
            upstream_override: Some(format!("http://{addr}/")),
            dir: d.path().into(),
            port: 4319,
            permits: Arc::new(tokio::sync::Semaphore::new(8)),
        });
        let token = std::fs::read_to_string(d.path().join("hub.key")).unwrap();
        for uri in ["/v1/chat/completions", "/v1/messages"] {
            let req = Request::builder().method("POST").uri(uri).header("host","localhost:4319").header("content-type","application/json").header("authorization",format!("Bearer {}",token.trim())).body(Body::from(json!({"model":"test/coder","max_tokens":32,"messages":[{"role":"user","content":"fixture"}]}).to_string())).unwrap();
            let response = app.clone().oneshot(req).await.unwrap();
            assert_eq!(response.status(), 402);
            let text = String::from_utf8(
                to_bytes(response.into_body(), 65536)
                    .await
                    .unwrap()
                    .to_vec(),
            )
            .unwrap();
            assert!(text.contains("subscription_quota_exhausted"));
            assert!(!text.contains("UPSTREAM_FIXTURE") && !text.contains(token.trim()));
        }
        task.abort();
    }
    #[test]
    fn conflicting_keys_fail() {
        let mut h = HeaderMap::new();
        h.insert("x-api-key", "fixture".parse().unwrap());
        assert!(auth(&h, "fixture", true));
        h.insert("authorization", "Bearer other".parse().unwrap());
        assert!(!auth(&h, "fixture", true));
    }
    #[test]
    fn native_blocks_preserved() {
        let mut b = json!({"max_tokens":12,"messages":[{"role":"user","content":[{"type":"tool_result","tool_use_id":"test","content":"ok"}]},{"role":"system","content":"policy"}],"thinking":{"type":"adaptive"}});
        normalize(&mut b).unwrap();
        assert_eq!(b["system"][0]["text"], "policy");
        assert_eq!(b["thinking"]["type"], "adaptive");
        assert_eq!(b["messages"].as_array().unwrap().len(), 1);
    }
    #[tokio::test]
    async fn loopback_auth_catalog_reload() {
        let d = tempfile::tempdir().unwrap();
        config::init(d.path()).unwrap();
        let app = router(App {
            telemetry: None,
            upstream_override: None,
            dir: d.path().into(),
            port: 4319,
            permits: Arc::new(tokio::sync::Semaphore::new(8)),
        });
        let req = |uri, host| {
            axum::http::Request::builder()
                .uri(uri)
                .header("host", host)
                .body(Body::empty())
                .unwrap()
        };
        assert_eq!(
            app.clone()
                .oneshot(req("/health", "evil:4319"))
                .await
                .unwrap()
                .status(),
            403
        );
        assert_eq!(
            app.clone()
                .oneshot(req("/health", "localhost:4319"))
                .await
                .unwrap()
                .status(),
            200
        );
        assert_eq!(
            app.oneshot(req("/v1/models", "localhost:4319"))
                .await
                .unwrap()
                .status(),
            401
        );
    }
    #[tokio::test]
    async fn forwards_native_sse_and_thinking_with_local_auth_isolation() {
        let d = tempfile::tempdir().unwrap();
        config::init(d.path()).unwrap();
        let keyfile = d.path().join("upstream.key");
        config::write(&keyfile, b"UPSTREAM_FIXTURE").unwrap();
        config::save(&d.path().join("providers.json"),&json!({"proxy":"","providers":{"local-test":{"prefix":"test","name":"Test","enabled":true,"baseURL":"https://provider.invalid/v1","anthropicBaseURL":"https://provider.invalid/anthropic","keyFile":keyfile,"models":[{"id":"coder","name":"Coder","maxTokens":64}]}}})).unwrap();
        let gate = Arc::new(tokio::sync::Notify::new());
        let g = gate.clone();
        let upstream=Router::new().route("/",post(move |h:HeaderMap,Json(v):Json<Value>|{let g=g.clone();async move{
            assert_eq!(header(&h,"x-api-key"),Some("UPSTREAM_FIXTURE"));assert!(!h.contains_key("authorization"));
            assert_eq!(v["model"],"coder");assert_eq!(v["max_tokens"],64);assert_eq!(v["output_config"]["effort"],"high");assert_eq!(v["thinking"]["type"],"adaptive");
            let stream=async_stream::stream!{yield Ok::<_,std::io::Error>("event: message_start\ndata: {}\n\n");g.notified().await;yield Ok("event: message_stop\ndata: {}\n\n");};
            Response::builder().header("content-type","text/event-stream").body(Body::from_stream(stream)).unwrap()
        }}));
        let bind = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = bind.local_addr().unwrap();
        let task = tokio::spawn(async move { axum::serve(bind, upstream).await.unwrap() });
        let app = router(App {
            telemetry: None,
            upstream_override: Some(format!("http://{addr}/")),
            dir: d.path().into(),
            port: 4319,
            permits: Arc::new(tokio::sync::Semaphore::new(8)),
        });
        let token = std::fs::read_to_string(d.path().join("hub.key")).unwrap();
        let req=axum::http::Request::builder().method("POST").uri("/v1/messages").header("host","localhost:4319").header("content-type","application/json").header("x-api-key",token.trim()).body(Body::from(json!({"model":"test/coder","max_tokens":128,"stream":true,"messages":[{"role":"user","content":"fixture"}],"thinking":{"type":"adaptive"},"output_config":{"effort":"high"}}).to_string())).unwrap();
        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), 200);
        let mut stream = response.into_body().into_data_stream();
        let first = tokio::time::timeout(Duration::from_secs(2), stream.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(String::from_utf8_lossy(&first).contains("message_start"));
        gate.notify_one();
        let tail = stream.next().await.unwrap().unwrap();
        assert!(String::from_utf8_lossy(&tail).contains("message_stop"));
        task.abort();
    }
    #[tokio::test]
    async fn observes_real_stream_without_altering_bytes_and_keeps_usage_tail() {
        let d = tempfile::tempdir().unwrap();
        config::init(d.path()).unwrap();
        let keyfile = d.path().join("upstream.key");
        config::write(&keyfile, b"UPSTREAM_FIXTURE").unwrap();
        config::save(&d.path().join("providers.json"),&json!({"providers":{"local-test":{"prefix":"test","name":"Test","enabled":true,"baseURL":"https://provider.invalid/v1","keyFile":keyfile,"models":[{"id":"coder","maxTokens":128}]}}})).unwrap();
        let wire = "data: {\"id\":\"response_fixture\",\"choices\":[{\"delta\":{\"content\":\"你好\"}}]}\n\ndata: {\"choices\":[],\"usage\":{\"prompt_tokens\":100,\"completion_tokens\":7,\"prompt_tokens_details\":{\"cached_tokens\":80}}}\n\ndata: [DONE]\n\n";
        let upstream = Router::new().route(
            "/",
            post(move |h: HeaderMap| async move {
                assert_eq!(header(&h, "authorization"), Some("Bearer UPSTREAM_FIXTURE"));
                assert!(!h.contains_key("x-loom-session-id"));
                let parts = wire
                    .as_bytes()
                    .chunks(3)
                    .map(|v| Ok::<_, std::io::Error>(axum::body::Bytes::copy_from_slice(v)))
                    .collect::<Vec<_>>();
                Response::builder()
                    .header("content-type", "text/event-stream")
                    .body(Body::from_stream(futures_util::stream::iter(parts)))
                    .unwrap()
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, upstream).await.unwrap() });
        let store = Store::open(d.path()).unwrap();
        let app = router(App {
            telemetry: Some(store.clone()),
            upstream_override: Some(format!("http://{addr}/")),
            dir: d.path().into(),
            port: 4319,
            permits: Arc::new(tokio::sync::Semaphore::new(8)),
        });
        let key = std::fs::read_to_string(d.path().join("hub.key")).unwrap();
        let req=Request::builder().method("POST").uri("/v1/chat/completions").header("host","127.0.0.1:4319").header("content-type","application/json").header("authorization",format!("Bearer {}",key.trim())).header("x-loom-harness","pi").header("x-loom-session-id","pi_fixture").body(Body::from(json!({"model":"test/coder","messages":[{"role":"user","content":"hello"}],"stream":true}).to_string())).unwrap();
        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), 200);
        let id = response.headers()["x-loom-request-id"]
            .to_str()
            .unwrap()
            .to_owned();
        let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        assert_eq!(&bytes[..], wire.as_bytes());
        let saved = store.request(&id).unwrap();
        assert_eq!(saved["status"], "success");
        assert_eq!(saved["session"], "pi:pi_fixture");
        assert_eq!(saved["provider"], "local-test");
        assert_eq!(saved["usage"]["input"], 100);
        assert_eq!(saved["usage"]["cache_read"], 80);
        assert!(saved["ttft_ms"].is_number());
        assert!(
            !serde_json::to_string(&saved)
                .unwrap()
                .contains("UPSTREAM_FIXTURE")
        );
        server.abort();
    }
}
