use crate::config::{self, Result};
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
    #[cfg(test)]
    pub upstream_override: Option<String>,
    pub dir: PathBuf,
    pub port: u16,
    pub permits: Arc<tokio::sync::Semaphore>,
}
fn err(native: bool, status: u16, code: &str) -> Response {
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
            json!({"type":"error","error":{"type":kind,"message":code}})
        } else {
            json!({"error":{"type":"local_hub_error","message":code,"code":code}})
        }),
    )
        .into_response()
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
    let key = match tokio::fs::read_to_string(app.dir.join("hub.key")).await {
        Ok(v) => v,
        Err(_) => return err(native, 503, "LOCAL_KEY_UNAVAILABLE"),
    };
    if !auth(&h, key.trim(), native) {
        return err(native, 401, "LOCAL_KEY_REQUIRED");
    }
    let registry = match config::read(&app.dir.join("providers.json")) {
        Ok(v) => v,
        Err(_) => return err(native, 503, "REGISTRY_INVALID"),
    };
    let models = match config::catalog(&registry) {
        Ok(v) => v,
        Err(_) => return err(native, 503, "REGISTRY_INVALID"),
    };
    if path == "/v1/models" && req.method() == "GET" {
        let data:Vec<_>=models.iter().filter(|m|!native||config::anthropic(&m.p).ok().flatten().is_some()).map(|m|if native{json!({"id":m.id,"type":"model","display_name":m.m["name"],"created_at":"1970-01-01T00:00:00Z"})}else{json!({"id":m.id,"object":"model","owned_by":m.p["name"],"name":m.m["name"],"billing":m.p["billing"],"context_window":m.m["contextWindow"],"max_output_tokens":m.m["maxTokens"]})}).collect();
        return Json(if native{json!({"data":data,"has_more":false,"first_id":data.first().map(|v|&v["id"]),"last_id":data.last().map(|v|&v["id"])})}else{json!({"object":"list","data":data})}).into_response();
    }
    let messages = path == "/v1/messages";
    if !["/v1/messages", "/v1/chat/completions"].contains(&path.as_str()) || req.method() != "POST"
    {
        return err(native, 404, "ENDPOINT_NOT_SUPPORTED");
    }
    if !header(&h, "content-type").is_some_and(|s| s.starts_with("application/json")) {
        return err(native, 415, "JSON_REQUIRED");
    }
    let permit = match app.permits.clone().try_acquire_owned() {
        Ok(v) => v,
        Err(_) => return err(native, 429, "LOCAL_CONCURRENCY_LIMIT"),
    };
    let raw = match tokio::time::timeout(
        Duration::from_secs(30),
        to_bytes(req.into_body(), 20 * 1024 * 1024),
    )
    .await
    {
        Ok(Ok(v)) => v,
        _ => return err(native, 413, "REQUEST_TOO_LARGE_OR_TIMEOUT"),
    };
    let mut body: Value = match serde_json::from_slice(&raw) {
        Ok(v) => v,
        Err(_) => return err(native, 400, "INVALID_JSON"),
    };
    let route = match models.iter().find(|m| body["model"] == m.id) {
        Some(v) => v,
        None => return err(native, 400, "UNKNOWN_MODEL"),
    };
    if !body["messages"].as_array().is_some_and(|a| !a.is_empty())
        || body.get("stream").is_some_and(|v| !v.is_boolean())
    {
        return err(native, 400, "INVALID_CHAT_REQUEST");
    }
    if messages && normalize(&mut body).is_err() {
        return err(native, 400, "INVALID_MESSAGES_REQUEST");
    }
    let url = if messages {
        match config::anthropic(&route.p) {
            Ok(Some(v)) => v,
            _ => return err(native, 400, "ANTHROPIC_NOT_SUPPORTED_BY_PROVIDER"),
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
            _ => return err(native, 400, "INVALID_MAX_TOKENS"),
        },
        None => max,
    };
    body["max_tokens"] = json!(n.min(max));
    body["model"] = route.m["id"].clone();
    body.as_object_mut().unwrap().remove("store");
    body.as_object_mut()
        .unwrap()
        .remove("max_completion_tokens");
    let upstream_key = match tokio::fs::read_to_string(route.p["keyFile"].as_str().unwrap()).await {
        Ok(v) => v,
        Err(_) => return err(native, 503, "PROVIDER_KEY_UNAVAILABLE"),
    };
    let mut builder = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(600));
    if let Some(proxy) = registry["proxy"].as_str().filter(|v| !v.is_empty()) {
        builder = match reqwest::Proxy::all(proxy) {
            Ok(p) => builder.proxy(p),
            Err(_) => return err(native, 503, "PROXY_INVALID"),
        };
    }
    let client = match builder.build() {
        Ok(v) => v,
        Err(_) => return err(native, 503, "UPSTREAM_UNAVAILABLE"),
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
            return err(native, 400, "INVALID_ANTHROPIC_HEADERS");
        }
        request = request.header("anthropic-version", version);
        if let Some(beta) = header(&h, "anthropic-beta") {
            if beta.len() > 1024
                || beta.is_empty()
                || !beta
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_,.-".contains(&b))
            {
                return err(native, 400, "INVALID_ANTHROPIC_HEADERS");
            }
            request = request.header("anthropic-beta", beta);
        }
    }
    let response = match request.send().await {
        Ok(v) => v,
        Err(_) => return err(native, 502, "UPSTREAM_UNAVAILABLE"),
    };
    if !response.status().is_success() {
        let s = response.status().as_u16();
        return err(native, s, &format!("UPSTREAM_{s}"));
    }
    let mut bytes = response.bytes_stream();
    let stream = async_stream::stream! {let _permit=permit;let mut total=0usize;while let Some(chunk)=bytes.next().await{match chunk{Ok(chunk)=>{total+=chunk.len();if total>64*1024*1024{yield Err(std::io::Error::other("response too large"));break;}yield Ok(chunk);},Err(_)=>{yield Err(std::io::Error::other("upstream stream failed"));break;}}}};
    Response::builder()
        .status(200)
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
}
