pub mod usage;
use crate::config::{self, Result};
use regex::Regex;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Instant,
};
use usage::{Collector, Usage};

pub fn now() -> i64 {
    chrono::Utc::now().timestamp_millis()
}
pub fn hash(s: &str) -> String {
    Sha256::digest(s.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
#[derive(Clone)]
pub struct Redactor {
    secrets: Vec<String>,
}
impl Redactor {
    pub fn new(d: &Path) -> Self {
        let mut paths = vec![d.join("hub.key")];
        if let Ok(r) = config::read(&d.join("providers.json"))
            && let Some(p) = r["providers"].as_object()
        {
            paths.extend(
                p.values()
                    .filter_map(|v| v["keyFile"].as_str().map(PathBuf::from)),
            );
        }
        Self {
            secrets: paths
                .iter()
                .filter_map(|p| fs::read_to_string(p).ok())
                .map(|s| s.trim().to_string())
                .filter(|s| s.len() >= 6)
                .collect(),
        }
    }
    pub fn text(&self, s: &str) -> String {
        static TOKENS: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
            Regex::new(r"(?i)(?:bearer\s+[a-z0-9._~+/=-]+|(?:sk-|hub-|gh[pousr]_)[a-z0-9_-]{12,}|eyJ[a-z0-9_-]+\.[a-z0-9_-]+\.[a-z0-9_-]+)").unwrap()
        });
        let mut out = s.to_string();
        for k in &self.secrets {
            out = out.replace(k, "[REDACTED]");
        }
        TOKENS.replace_all(&out, "[REDACTED]").into_owned()
    }
    pub fn events(&self, events: &[Value]) -> Value {
        fn text_values(v: &mut Value, f: &mut dyn FnMut(&mut String)) {
            match v {
                Value::Array(a) => {
                    for v in a {
                        text_values(v, f)
                    }
                }
                Value::Object(m) => {
                    for (k, v) in m {
                        if [
                            "content",
                            "text",
                            "thinking",
                            "reasoning_content",
                            "partial_json",
                            "arguments",
                        ]
                        .contains(&k.as_str())
                        {
                            if let Value::String(s) = v {
                                f(s);
                            } else {
                                text_values(v, f);
                            }
                        } else {
                            text_values(v, f);
                        }
                    }
                }
                _ => {}
            }
        }
        let mut v = json!(events);
        let mut ranges = vec![];
        let mut combined = String::new();
        text_values(&mut v, &mut |s| {
            let start = combined.len();
            combined.push_str(s);
            ranges.push(start..combined.len());
        });
        let mut blocked = vec![false; ranges.len()];
        for secret in &self.secrets {
            for (start, _) in combined.match_indices(secret) {
                let end = start + secret.len();
                for (i, r) in ranges.iter().enumerate() {
                    if r.start < end && r.end > start {
                        blocked[i] = true;
                    }
                }
            }
        }
        let mut i = 0;
        text_values(&mut v, &mut |s| {
            if blocked[i] {
                *s = "[REDACTED]".into();
            }
            i += 1;
        });
        self.value(&v)
    }
    pub fn value(&self, v: &Value) -> Value {
        match v {
            Value::String(s) => json!(self.text(s)),
            Value::Array(a) => Value::Array(a.iter().map(|v| self.value(v)).collect()),
            Value::Object(m) => Value::Object(
                m.iter()
                    .map(|(k, v)| {
                        let sensitive = [
                            "authorization",
                            "x-api-key",
                            "cookie",
                            "set-cookie",
                            "api_key",
                            "apikey",
                            "apiKey",
                            "access_token",
                            "refresh_token",
                            "password",
                            "secret",
                            "signature",
                            "user_id",
                            "token",
                            "client_secret",
                            "credential",
                        ]
                        .iter()
                        .any(|n| k.eq_ignore_ascii_case(n));
                        (
                            k.clone(),
                            if sensitive {
                                json!("[REDACTED]")
                            } else {
                                self.value(v)
                            },
                        )
                    })
                    .collect(),
            ),
            _ => v.clone(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Record {
    pub id: String,
    pub started: i64,
    pub ended: Option<i64>,
    pub status: String,
    pub http: u16,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub harness: String,
    pub session: Option<String>,
    pub protocol: String,
    pub streaming: bool,
    pub first_byte_ms: Option<u64>,
    pub ttft_ms: Option<u64>,
    pub duration_ms: Option<u64>,
    pub usage: Usage,
    pub response_id: Option<String>,
    pub finish_reason: Option<String>,
    pub error: Option<String>,
    pub spans: Vec<Value>,
    pub request: Value,
    pub forwarded: Value,
    pub response: Value,
    pub truncated: bool,
    pub parse_errors: usize,
}
#[derive(Clone)]
pub struct Store {
    pub(crate) conn: Arc<Mutex<Connection>>,
    pub dir: PathBuf,
}
impl Store {
    pub fn open(d: &Path) -> Result<Self> {
        let dir = d.join("observability");
        fs::create_dir_all(&dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
        }
        let path = dir.join("traces.sqlite3");
        if !path.exists() {
            config::write(&path, &[])?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
        }
        let c = Connection::open(&path)?;
        c.busy_timeout(std::time::Duration::from_secs(3))?;
        // DELETE journaling avoids permissive WAL sidecars on imported/legacy directories.
        c.execute_batch("PRAGMA journal_mode=DELETE; PRAGMA secure_delete=ON;
CREATE TABLE IF NOT EXISTS requests(id TEXT PRIMARY KEY, started INTEGER NOT NULL, ended INTEGER, provider TEXT, model TEXT, harness TEXT NOT NULL, session TEXT, status TEXT NOT NULL, http INTEGER, duration REAL, ttft REAL, input INTEGER, output INTEGER, cache_read INTEGER, cache_write INTEGER, reasoning INTEGER, response_id TEXT, detail TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS req_time ON requests(started); CREATE INDEX IF NOT EXISTS req_dimension ON requests(provider,model,harness);CREATE INDEX IF NOT EXISTS req_session ON requests(session);CREATE INDEX IF NOT EXISTS req_response ON requests(response_id);
CREATE TABLE IF NOT EXISTS sessions(id TEXT PRIMARY KEY,harness TEXT NOT NULL,native_id TEXT NOT NULL,title TEXT,cwd TEXT,parent TEXT,started INTEGER,updated INTEGER,source TEXT,coverage TEXT NOT NULL DEFAULT 'native', UNIQUE(harness,native_id));
CREATE TABLE IF NOT EXISTS events(id TEXT PRIMARY KEY,session TEXT NOT NULL,parent TEXT,ts INTEGER NOT NULL,kind TEXT NOT NULL,payload TEXT NOT NULL,response_id TEXT,ordinal INTEGER NOT NULL DEFAULT 0);CREATE INDEX IF NOT EXISTS evt_session ON events(session,ts);
CREATE TABLE IF NOT EXISTS imports(path TEXT PRIMARY KEY,modified INTEGER,size INTEGER,offset INTEGER,session TEXT);
CREATE TABLE IF NOT EXISTS settings(key TEXT PRIMARY KEY,value TEXT NOT NULL);")?;
        // Entries left in flight by a crash are explicitly incomplete, never successful.
        c.execute(
            "UPDATE requests SET status='interrupted',ended=?1 WHERE status='inflight'",
            params![now()],
        )?;
        Ok(Self {
            conn: Arc::new(Mutex::new(c)),
            dir,
        })
    }
    pub fn save(&self, r: &Record) -> Result<()> {
        let c = self.conn.lock().map_err(|_| "trace lock poisoned")?;
        let mut r = r.clone();
        if r.session.is_none()
            && let Some(response) = &r.response_id
        {
            let linked=c.query_row("SELECT e.session,s.harness FROM events e JOIN sessions s ON s.id=e.session WHERE e.response_id=?1 LIMIT 1",[response],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?))).optional()?;
            if let Some((session, harness)) = linked {
                r.session = Some(session);
                r.harness = harness;
            }
        }
        if let Some(sid) = &r.session {
            let native = sid.split_once(':').map(|(_, s)| s).unwrap_or(sid);
            c.execute("INSERT INTO sessions(id,harness,native_id,title,started,updated,coverage) VALUES(?1,?2,?3,'',?4,?5,'gateway_only') ON CONFLICT(id) DO UPDATE SET updated=MAX(sessions.updated,excluded.updated)",params![sid,r.harness,native,r.started,r.ended.unwrap_or(r.started)])?;
        }
        c.execute("INSERT INTO requests(id,started,ended,provider,model,harness,session,status,http,duration,ttft,input,output,cache_read,cache_write,reasoning,response_id,detail) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18) ON CONFLICT(id) DO UPDATE SET started=excluded.started,ended=excluded.ended,provider=excluded.provider,model=excluded.model,harness=excluded.harness,session=excluded.session,status=excluded.status,http=excluded.http,duration=excluded.duration,ttft=excluded.ttft,input=excluded.input,output=excluded.output,cache_read=excluded.cache_read,cache_write=excluded.cache_write,reasoning=excluded.reasoning,response_id=excluded.response_id,detail=excluded.detail",params![r.id,r.started,r.ended,r.provider,r.model,r.harness,r.session,r.status,r.http,sql_int(r.duration_ms),sql_int(r.ttft_ms),sql_int(r.usage.input),sql_int(r.usage.output),sql_int(r.usage.cache_read),sql_int(r.usage.cache_write),sql_int(r.usage.reasoning),r.response_id,serde_json::to_string(&r)?])?;
        Ok(())
    }
    pub fn setting(&self, key: &str, default: Value) -> Value {
        self.conn
            .lock()
            .ok()
            .and_then(|c| {
                c.query_row("SELECT value FROM settings WHERE key=?1", [key], |r| {
                    r.get::<_, String>(0)
                })
                .ok()
            })
            .and_then(|v| serde_json::from_str(&v).ok())
            .unwrap_or(default)
    }
    pub fn set_setting(&self, key: &str, v: &Value) -> Result<()> {
        self.conn.lock().map_err(|_|"trace lock poisoned")?.execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![key,serde_json::to_string(v)?])?;
        Ok(())
    }
    pub fn request(&self, id: &str) -> Result<Value> {
        let c = self.conn.lock().map_err(|_| "trace lock poisoned")?;
        let (text, status, session, harness, ended): (
            String,
            String,
            Option<String>,
            String,
            Option<i64>,
        ) = c.query_row(
            "SELECT detail,status,session,harness,ended FROM requests WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )?;
        let mut v: Value = serde_json::from_str(&text)?;
        v["status"] = json!(status);
        v["session"] = json!(session);
        v["harness"] = json!(harness);
        v["ended"] = json!(ended);
        Ok(v)
    }
    pub fn requests(&self, f: &Filter) -> Result<Value> {
        let c = self.conn.lock().map_err(|_| "trace lock poisoned")?;
        let args = f.args();
        let count: i64 = c.query_row(
            &format!("SELECT COUNT(*) FROM requests WHERE {}", Filter::SQL),
            rusqlite::params_from_iter(args.iter().map(sql_value)),
            |r| r.get(0),
        )?;
        let mut args = args;
        args.push(json!(f.limit.clamp(1, 200)));
        args.push(json!(f.offset.min(1000000)));
        let mut q=c.prepare(&format!("SELECT id,started,provider,model,harness,session,status,http,duration,ttft,input,output,cache_read,cache_write FROM requests WHERE {} ORDER BY started DESC,id LIMIT ?6 OFFSET ?7",Filter::SQL))?;
        let rows=q.query_map(rusqlite::params_from_iter(args.iter().map(sql_value)),|r|Ok(json!({"id":r.get::<_,String>(0)?,"started":r.get::<_,i64>(1)?,"provider":r.get::<_,Option<String>>(2)?,"model":r.get::<_,Option<String>>(3)?,"harness":r.get::<_,String>(4)?,"session":r.get::<_,Option<String>>(5)?,"status":r.get::<_,String>(6)?,"http":r.get::<_,u16>(7)?,"duration_ms":r.get::<_,Option<f64>>(8)?,"ttft_ms":r.get::<_,Option<f64>>(9)?,"input":r.get::<_,Option<i64>>(10)?,"output":r.get::<_,Option<i64>>(11)?,"cache_read":r.get::<_,Option<i64>>(12)?,"cache_write":r.get::<_,Option<i64>>(13)?})))?;
        Ok(json!({"total":count,"items":rows.collect::<std::result::Result<Vec<_>,_>>()?}))
    }
}
fn sql_int(n: Option<u64>) -> Option<i64> {
    n.and_then(|n| i64::try_from(n).ok())
}
fn sql_value(v: &Value) -> rusqlite::types::Value {
    match v {
        Value::Number(n) => rusqlite::types::Value::Integer(n.as_i64().unwrap_or(0)),
        Value::String(s) => s.clone().into(),
        _ => rusqlite::types::Value::Null,
    }
}
#[derive(Clone, Deserialize)]
pub struct Filter {
    #[serde(default)]
    pub from: i64,
    #[serde(default = "max_time")]
    pub to: i64,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub harness: String,
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
}
fn max_time() -> i64 {
    i64::MAX
}
fn default_limit() -> u32 {
    50
}
impl Default for Filter {
    fn default() -> Self {
        Self {
            from: 0,
            to: max_time(),
            provider: String::new(),
            model: String::new(),
            harness: String::new(),
            limit: 50,
            offset: 0,
        }
    }
}
impl Filter {
    const SQL: &'static str = "started>=?1 AND started<?2 AND (?3='' OR provider=?3) AND (?4='' OR model=?4) AND (?5='' OR harness=?5)";
    fn args(&self) -> Vec<Value> {
        vec![
            json!(self.from),
            json!(self.to),
            json!(self.provider),
            json!(self.model),
            json!(self.harness),
        ]
    }
}

pub struct Flight {
    pub record: Record,
    pub collector: Collector,
    store: Store,
    redactor: Redactor,
    clock: Instant,
    finished: bool,
    capture: bool,
}
impl Flight {
    pub fn start(store: Store, d: &Path, h: &axum::http::HeaderMap, protocol: &str) -> Self {
        let harness = h
            .get("x-loom-harness")
            .and_then(|v| v.to_str().ok())
            .filter(|s| ["pi", "opencode2", "claude-code"].contains(s))
            .unwrap_or("unknown")
            .to_string();
        let session = h
            .get("x-loom-session-id")
            .and_then(|v| v.to_str().ok())
            .filter(|s| {
                !s.is_empty()
                    && s.len() <= 200
                    && s.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"_-.".contains(&b))
            })
            .map(|s| format!("{harness}:{s}"));
        let r = Record {
            id: format!("req_{:032x}", rand::random::<u128>()),
            started: now(),
            ended: None,
            status: "inflight".into(),
            http: 0,
            provider: None,
            model: None,
            harness,
            session,
            protocol: protocol.into(),
            streaming: false,
            first_byte_ms: None,
            ttft_ms: None,
            duration_ms: None,
            usage: Usage::default(),
            response_id: None,
            finish_reason: None,
            error: None,
            spans: vec![json!({"stage":"accepted","ms":0})],
            request: Value::Null,
            forwarded: Value::Null,
            response: Value::Null,
            truncated: false,
            parse_errors: 0,
        };
        let capture = store.setting("capture_bodies", json!(true)) == true;
        let redactor = Redactor::new(d);
        let f = Self {
            record: r,
            collector: Collector::default(),
            store,
            redactor,
            clock: Instant::now(),
            finished: false,
            capture,
        };
        if f.store.save(&f.record).is_err() {
            eprintln!("Loom: 无法写入本机观测数据；推理继续。");
        }
        f
    }
    pub fn stage(&mut self, s: &str) {
        self.record
            .spans
            .push(json!({"stage":s,"ms":self.elapsed()}));
    }
    fn elapsed(&self) -> u64 {
        self.clock.elapsed().as_millis() as u64
    }
    pub fn request_body(&mut self, v: &Value) {
        if self.record.session.is_none()
            && let Some(raw) = v.pointer("/metadata/user_id").and_then(Value::as_str)
        {
            let id = serde_json::from_str::<Value>(raw)
                .ok()
                .and_then(|v| v["session_id"].as_str().map(String::from))
                .or_else(|| raw.rsplit_once("_session_").map(|(_, s)| s.to_string()));
            if let Some(id) = id.filter(|s| {
                s.len() <= 200
                    && s.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"_-.".contains(&b))
            }) {
                self.record.harness = "claude-code".into();
                self.record.session = Some(format!("claude-code:{id}"));
            }
        }
        self.record.model = v["model"].as_str().map(|s| self.redactor.text(s));
        if self.capture {
            self.record.request = self.redactor.value(v);
        }
        self.record.streaming = v["stream"] == true;
        self.collector = Collector::new(self.record.protocol == "anthropic", self.record.streaming);
    }
    pub fn forwarded(&mut self, provider: &str, v: &Value) {
        self.record.provider = Some(provider.into());
        if self.capture {
            self.record.forwarded = self.redactor.value(v);
        }
        self.stage("request_prepared");
    }
    pub fn headers(&mut self, http: u16) {
        self.record.http = http;
        self.stage("upstream_headers");
    }
    pub fn chunk(&mut self, b: &[u8]) {
        if self.record.first_byte_ms.is_none() {
            self.record.first_byte_ms = Some(self.elapsed());
            self.stage("first_byte");
        }
        self.collector.feed(b);
        if self.record.ttft_ms.is_none() && self.collector.first_token {
            self.record.ttft_ms = Some(self.elapsed());
            self.stage("first_token");
        }
    }
    pub fn finish(&mut self, error: Option<&str>) {
        if self.finished {
            return;
        }
        self.finished = true;
        self.collector.finish();
        self.record.ended = Some(now());
        self.record.duration_ms = Some(self.elapsed());
        self.record.usage = self.collector.usage.clone();
        self.record.response_id = self.collector.response_id.clone();
        self.record.finish_reason = self.collector.finish_reason.clone();
        self.record.truncated = self.collector.truncated;
        self.record.parse_errors = self.collector.parse_errors;
        let missing_end = self.record.streaming && !self.collector.ended;
        let error = error.or(if self.collector.failed {
            Some("UPSTREAM_EVENT_ERROR")
        } else {
            None
        });
        self.record.status = if error.is_some() {
            "failed"
        } else if missing_end || self.collector.truncated || self.collector.parse_errors > 0 {
            "partial"
        } else {
            "success"
        }
        .into();
        self.record.error = error.map(String::from);
        if self.capture && self.record.response.is_null() {
            self.record.response = self.redactor.events(&self.collector.events);
        }
        self.stage("complete");
        if self.store.save(&self.record).is_err() {
            eprintln!("Loom: 本机观测写入失败；未输出请求内容。");
        }
    }
    pub fn error_response(&mut self, v: Value) {
        if self.capture {
            self.record.response = self.redactor.value(&v);
        }
    }
    pub fn fail(&mut self, http: u16, code: &str) {
        self.record.http = http;
        if self.record.response.is_null() {
            self.error_response(json!({"client_error":code,"http":http}));
        }
        self.finish(Some(code));
    }
}
impl Drop for Flight {
    fn drop(&mut self) {
        if !self.finished {
            self.finish(Some("CLIENT_DISCONNECTED"));
            self.record.status = "cancelled".into();
            let _ = self.store.save(&self.record);
        }
    }
}

impl Store {
    pub fn stats(&self, f: &Filter) -> Result<Value> {
        let c = self.conn.lock().map_err(|_| "trace lock poisoned")?;
        let args = f.args();
        let aggregate = "COUNT(*),SUM(status='success'),SUM(status IN ('failed','partial','cancelled','interrupted')),SUM(input),SUM(output),SUM(cache_read),SUM(cache_write),SUM(reasoning),SUM(input IS NOT NULL AND output IS NOT NULL),SUM(cache_read IS NOT NULL),SUM(CASE WHEN cache_read IS NOT NULL THEN input END),AVG(duration),AVG(ttft)";
        fn metrics(r: &rusqlite::Row<'_>, base: usize) -> rusqlite::Result<Value> {
            let n = |i| r.get::<_, Option<i64>>(base + i);
            let requests = n(0)?.unwrap_or(0);
            let cached = n(5)?;
            let denominator = n(10)?;
            Ok(
                json!({"requests":requests,"success":n(1)?.unwrap_or(0),"errors":n(2)?.unwrap_or(0),"input_tokens":n(3)?,"output_tokens":n(4)?,"cache_read_tokens":cached,"cache_write_tokens":n(6)?,"reasoning_tokens":n(7)?,"usage_observed":n(8)?.unwrap_or(0),"cache_observed":n(9)?.unwrap_or(0),"cache_hit_rate":cached.zip(denominator).filter(|(_,d)|*d>0).map(|(n,d)|n as f64/d as f64),"avg_duration_ms":r.get::<_,Option<f64>>(base+11)?,"avg_ttft_ms":r.get::<_,Option<f64>>(base+12)?}),
            )
        }
        let summary = c.query_row(
            &format!("SELECT {aggregate} FROM requests WHERE {}", Filter::SQL),
            rusqlite::params_from_iter(args.iter().map(sql_value)),
            |r| metrics(r, 0),
        )?;
        let mut q=c.prepare(&format!("SELECT provider,model,harness,{aggregate} FROM requests WHERE {} GROUP BY provider,model,harness ORDER BY SUM(COALESCE(input,0)+COALESCE(output,0)) DESC",Filter::SQL))?;
        let groups = q
            .query_map(
                rusqlite::params_from_iter(args.iter().map(sql_value)),
                |r| {
                    let mut v = metrics(r, 3)?;
                    v["provider"] = json!(r.get::<_, Option<String>>(0)?);
                    v["model"] = json!(r.get::<_, Option<String>>(1)?);
                    v["harness"] = json!(r.get::<_, String>(2)?);
                    Ok(v)
                },
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let bucket = if f.to.saturating_sub(f.from) > 3 * 86400000 {
            86400000
        } else {
            3600000
        };
        let mut q=c.prepare(&format!("SELECT (started/{bucket})*{bucket},{aggregate} FROM requests WHERE {} GROUP BY 1 ORDER BY 1",Filter::SQL))?;
        let series = q
            .query_map(
                rusqlite::params_from_iter(args.iter().map(sql_value)),
                |r| {
                    let mut v = metrics(r, 1)?;
                    v["timestamp"] = json!(r.get::<_, i64>(0)?);
                    Ok(v)
                },
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut q = c.prepare(&format!(
            "SELECT duration FROM requests WHERE {} AND duration IS NOT NULL ORDER BY duration",
            Filter::SQL
        ))?;
        let durations = q
            .query_map(
                rusqlite::params_from_iter(args.iter().map(sql_value)),
                |r| r.get::<_, f64>(0),
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let percentile = |p: f64| {
            durations
                .get(((durations.len().saturating_sub(1)) as f64 * p).ceil() as usize)
                .copied()
        };
        Ok(
            json!({"summary":summary,"groups":groups,"series":series,"bucket_ms":bucket,"p50_duration_ms":percentile(0.50),"p95_duration_ms":percentile(0.95),"source":"gateway","billing_cost":null}),
        )
    }
    pub fn retention(&self) -> Result<u64> {
        let days = self
            .setting("retention_days", json!(30))
            .as_u64()
            .unwrap_or(30)
            .clamp(1, 3650);
        let cutoff = now() - (days * 86400000) as i64;
        let c = self.conn.lock().map_err(|_| "trace lock poisoned")?;
        let removed = c.execute(
            "DELETE FROM requests WHERE started<?1 AND status!='inflight'",
            [cutoff],
        )?;
        c.execute("DELETE FROM events WHERE ts<?1", [cutoff])?;
        c.execute(
            "DELETE FROM sessions WHERE updated<?1 AND id NOT IN (SELECT session FROM events)",
            [cutoff],
        )?;
        Ok(removed as u64)
    }
}

impl Store {
    pub fn sessions(&self, harness: &str, search: &str, offset: u32) -> Result<Value> {
        let c = self.conn.lock().map_err(|_| "trace lock")?;
        let count:i64=c.query_row("SELECT COUNT(*) FROM sessions WHERE (?1='' OR harness=?1) AND (?2='' OR instr(lower(COALESCE(title,'')||native_id),lower(?2))>0)",params![harness,search],|r|r.get(0))?;
        let mut q=c.prepare("SELECT s.id,s.harness,s.native_id,s.title,s.cwd,s.parent,s.started,s.updated,s.coverage,(SELECT COUNT(*) FROM events WHERE session=s.id),(SELECT COUNT(*) FROM requests WHERE session=s.id) FROM sessions s WHERE (?1='' OR harness=?1) AND (?2='' OR instr(lower(COALESCE(title,'')||native_id),lower(?2))>0) ORDER BY s.updated DESC,s.id LIMIT 50 OFFSET ?3")?;
        let rows=q.query_map(params![harness,search,offset.min(1000000)],|r|Ok(json!({"id":r.get::<_,String>(0)?,"harness":r.get::<_,String>(1)?,"native_id":r.get::<_,String>(2)?,"title":r.get::<_,Option<String>>(3)?,"cwd":r.get::<_,Option<String>>(4)?,"parent":r.get::<_,Option<String>>(5)?,"started":r.get::<_,i64>(6)?,"updated":r.get::<_,i64>(7)?,"coverage":r.get::<_,String>(8)?,"events":r.get::<_,i64>(9)?,"requests":r.get::<_,i64>(10)?})))?.collect::<std::result::Result<Vec<_>,_>>()?;
        Ok(json!({"total":count,"items":rows}))
    }
    pub fn session(&self, id: &str, offset: u32) -> Result<Value> {
        let c = self.conn.lock().map_err(|_| "trace lock")?;
        let metadata=c.query_row("SELECT id,harness,native_id,title,cwd,parent,started,updated,coverage FROM sessions WHERE id=?1",[id],|r|Ok(json!({"id":r.get::<_,String>(0)?,"harness":r.get::<_,String>(1)?,"native_id":r.get::<_,String>(2)?,"title":r.get::<_,Option<String>>(3)?,"cwd":r.get::<_,Option<String>>(4)?,"parent":r.get::<_,Option<String>>(5)?,"started":r.get::<_,i64>(6)?,"updated":r.get::<_,i64>(7)?,"coverage":r.get::<_,String>(8)?})))?;
        let count: i64 =
            c.query_row("SELECT COUNT(*) FROM events WHERE session=?1", [id], |r| {
                r.get(0)
            })?;
        let mut q=c.prepare("SELECT id,parent,ts,kind,payload,response_id FROM events WHERE session=?1 ORDER BY ts,ordinal,id LIMIT 100 OFFSET ?2")?;
        let events=q.query_map(params![id,offset.min(1000000)],|r|Ok(json!({"id":r.get::<_,String>(0)?,"parent":r.get::<_,Option<String>>(1)?,"timestamp":r.get::<_,i64>(2)?,"kind":r.get::<_,String>(3)?,"payload":serde_json::from_str::<Value>(&r.get::<_,String>(4)?).unwrap_or(Value::Null),"response_id":r.get::<_,Option<String>>(5)?})))?.collect::<std::result::Result<Vec<_>,_>>()?;
        let mut q=c.prepare("SELECT id,started,model,status,input,output,cache_read,cache_write FROM requests WHERE session=?1 ORDER BY started,id")?;
        let requests=q.query_map([id],|r|Ok(json!({"id":r.get::<_,String>(0)?,"started":r.get::<_,i64>(1)?,"model":r.get::<_,Option<String>>(2)?,"status":r.get::<_,String>(3)?,"input":r.get::<_,Option<i64>>(4)?,"output":r.get::<_,Option<i64>>(5)?,"cache_read":r.get::<_,Option<i64>>(6)?,"cache_write":r.get::<_,Option<i64>>(7)?})))?.collect::<std::result::Result<Vec<_>,_>>()?;
        Ok(
            json!({"session":metadata,"events":events,"total_events":count,"offset":offset,"requests":requests}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (tempfile::TempDir, Store) {
        let d = tempfile::tempdir().unwrap();
        config::init(d.path()).unwrap();
        let s = Store::open(d.path()).unwrap();
        (d, s)
    }
    fn record(d: &Path, s: &Store, input: u64, read: Option<u64>) -> String {
        let mut h = axum::http::HeaderMap::new();
        h.insert("x-loom-harness", "pi".parse().unwrap());
        h.insert("x-loom-session-id", "session-fixture".parse().unwrap());
        let mut f = Flight::start(s.clone(), d, &h, "openai");
        f.request_body(
            &json!({"model":"fixture/model","messages":[{"role":"user","content":"hello"}]}),
        );
        f.forwarded("local-fixture", &json!({"model":"model"}));
        f.headers(200);
        let mut usage = json!({"prompt_tokens":input,"completion_tokens":10});
        if let Some(r) = read {
            usage["prompt_tokens_details"] = json!({"cached_tokens":r});
        }
        f.chunk(serde_json::to_string(&json!({"id":format!("response-{input}"),"choices":[{"message":{"content":"ok"},"finish_reason":"stop"}],"usage":usage})).unwrap().as_bytes());
        f.finish(None);
        f.record.id.clone()
    }
    #[test]
    fn weighted_stats_filters_and_repeat_reads_do_not_double_count() {
        let (d, s) = fixture();
        record(d.path(), &s, 100, Some(90));
        record(d.path(), &s, 900, Some(0));
        record(d.path(), &s, 100, None);
        let v = s.stats(&Filter::default()).unwrap();
        assert_eq!(v["summary"]["requests"], 3);
        assert_eq!(v["summary"]["input_tokens"], 1100);
        assert_eq!(v["summary"]["cache_observed"], 2);
        assert_eq!(v["summary"]["cache_hit_rate"], 0.09);
        assert_eq!(s.stats(&Filter::default()).unwrap(), v);
        let f = Filter {
            model: "' OR 1=1 --".into(),
            ..Filter::default()
        };
        assert_eq!(s.requests(&f).unwrap()["total"], 0);
        assert_eq!(
            s.session("pi:session-fixture", 0).unwrap()["requests"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
    }
    #[test]
    fn credentials_and_split_sse_secrets_are_redacted_before_storage() {
        let (d, s) = fixture();
        let hub = fs::read_to_string(d.path().join("hub.key")).unwrap();
        let key = hub.trim();
        let redactor = Redactor::new(d.path());
        let split = key.len() / 2;
        let v = redactor.events(&[
            json!({"delta":{"content":&key[..split]}}),
            json!({"delta":{"content":&key[split..]}}),
        ]);
        assert!(!serde_json::to_string(&v).unwrap().contains(&key[..split]));
        let mut f = Flight::start(s.clone(), d.path(), &axum::http::HeaderMap::new(), "openai");
        f.request_body(&json!({"model":"fixture/model","messages":[{"content":key}],"api_key":"sensitive-fixture","metadata":{"user_id":"secret-account"}}));
        f.headers(401);
        f.finish(Some("UPSTREAM_401"));
        let v = s.request(&f.record.id).unwrap();
        let text = serde_json::to_string(&v).unwrap();
        assert!(!text.contains(key));
        assert!(!text.contains("sensitive-fixture"));
        assert!(!text.contains("secret-account"));
        assert!(
            !fs::read(s.dir.join("traces.sqlite3"))
                .unwrap()
                .windows(key.len())
                .any(|s| s == key.as_bytes())
        );
    }
    #[test]
    fn cancellation_and_crash_are_distinct_from_success() {
        let (d, s) = fixture();
        let id = {
            let f = Flight::start(s.clone(), d.path(), &axum::http::HeaderMap::new(), "openai");
            f.record.id.clone()
        };
        assert_eq!(s.request(&id).unwrap()["status"], "cancelled");
        let mut r: Record = serde_json::from_value(s.request(&id).unwrap()).unwrap();
        r.status = "inflight".into();
        r.ended = None;
        s.save(&r).unwrap();
        let recovered = Store::open(d.path()).unwrap();
        assert_eq!(recovered.request(&id).unwrap()["status"], "interrupted");
    }
    #[test]
    fn disabled_body_capture_and_retention_keep_metrics_but_not_bodies() {
        let (d, s) = fixture();
        s.set_setting("capture_bodies", &json!(false)).unwrap();
        let id = record(d.path(), &s, 40, None);
        let mut r: Record = serde_json::from_value(s.request(&id).unwrap()).unwrap();
        assert!(r.request.is_null() && r.forwarded.is_null() && r.response.is_null());
        assert_eq!(r.usage.input, Some(40));
        r.started = now() - 31 * 86400000;
        s.save(&r).unwrap();
        assert_eq!(s.retention().unwrap(), 1);
        assert_eq!(s.requests(&Filter::default()).unwrap()["total"], 0);
    }
}

#[cfg(test)]
mod correlation_tests {
    use super::*;
    #[test]
    fn transcript_arriving_before_or_after_response_has_same_exact_link() {
        let t = tempfile::tempdir().unwrap();
        config::init(t.path()).unwrap();
        let s = Store::open(t.path()).unwrap();
        for early in [true, false] {
            let response = if early {
                "response_early"
            } else {
                "response_late"
            };
            let session = if early { "pi:early" } else { "pi:late" };
            let link = || {
                let c = s.conn.lock().unwrap();
                c.execute("INSERT INTO sessions(id,harness,native_id,started,updated,coverage) VALUES(?1,'pi',?2,1,1,'native')",params![session,response]).unwrap();
                c.execute("INSERT INTO events(id,session,ts,kind,payload,response_id) VALUES(?1,?2,1,'assistant','{}',?1)",params![response,session]).unwrap();
            };
            if early {
                link();
            }
            let mut f = Flight::start(s.clone(), t.path(), &axum::http::HeaderMap::new(), "openai");
            f.request_body(
                &json!({"model":"fixture/model","messages":[{"role":"user","content":"fixture"}]}),
            );
            f.headers(200);
            f.chunk(json!({"id":response,"usage":{"prompt_tokens":1,"completion_tokens":1},"choices":[]}).to_string().as_bytes());
            f.finish(None);
            if !early {
                link();
                s.conn
                    .lock()
                    .unwrap()
                    .execute(
                        "UPDATE requests SET session=?1,harness='pi' WHERE response_id=?2",
                        params![session, response],
                    )
                    .unwrap();
            }
            let v = s.request(&f.record.id).unwrap();
            assert_eq!(v["session"], session);
            assert_eq!(v["harness"], "pi");
        }
        assert_eq!(
            s.stats(&Filter::default()).unwrap()["summary"]["requests"],
            2
        );
    }
}
