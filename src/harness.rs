//! Read-only native harness imports. Never modify transcript files or client databases.
use crate::{
    config::Result,
    telemetry::{Redactor, Store, hash, now},
};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Seek, SeekFrom},
    path::{Path, PathBuf},
};
fn timestamp(v: &Value) -> i64 {
    if let Some(n) = v.as_i64() {
        return n;
    }
    v.as_str()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|t| t.timestamp_millis())
        .unwrap_or_else(now)
}
fn files(root: &Path, out: &mut Vec<PathBuf>, depth: usize) -> Result<()> {
    if depth > 8 || !root.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(root)? {
        let e = entry?;
        let kind = e.file_type()?;
        if kind.is_dir() {
            files(&e.path(), out, depth + 1)?;
        } else if kind.is_file() && e.path().extension().is_some_and(|x| x == "jsonl") {
            out.push(e.path());
        }
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)] // Direct mapping of native session metadata.
fn session(
    store: &Store,
    harness: &str,
    native: &str,
    title: &str,
    cwd: &str,
    parent: Option<&str>,
    ts: i64,
    source: &str,
) -> Result<String> {
    let id = format!("{harness}:{native}");
    store.conn.lock().map_err(|_|"trace lock")?.execute("INSERT INTO sessions(id,harness,native_id,title,cwd,parent,started,updated,source,coverage) VALUES(?1,?2,?3,?4,?5,?6,?7,?7,?8,'native') ON CONFLICT(id) DO UPDATE SET title=CASE WHEN excluded.title!='' THEN excluded.title ELSE sessions.title END,cwd=excluded.cwd,parent=COALESCE(excluded.parent,sessions.parent),updated=MAX(excluded.updated,sessions.updated),source=excluded.source,coverage='native'",params![id,harness,native,title,cwd,parent,ts,source])?;
    Ok(id)
}
#[allow(clippy::too_many_arguments)] // Preserve native event identity and ancestry without rewriting payloads.
fn event(
    store: &Store,
    session: &str,
    id: &str,
    parent: Option<&str>,
    ts: i64,
    ordinal: i64,
    kind: &str,
    payload: &Value,
    response: Option<&str>,
    redactor: &Redactor,
) -> Result<()> {
    let c = store.conn.lock().map_err(|_| "trace lock")?;
    c.execute("INSERT INTO events(id,session,parent,ts,kind,payload,response_id,ordinal) VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload,kind=excluded.kind,parent=excluded.parent,ts=excluded.ts,response_id=excluded.response_id,ordinal=excluded.ordinal",params![format!("{session}:{id}"),session,parent,ts,kind,serde_json::to_string(&redactor.value(payload))?,response,ordinal])?;
    c.execute(
        "UPDATE sessions SET updated=MAX(updated,?2) WHERE id=?1",
        params![session, ts],
    )?;
    if let Some(response) = response {
        c.execute("UPDATE requests SET session=?1,harness=(SELECT harness FROM sessions WHERE id=?1) WHERE response_id=?2 AND (session IS NULL OR session=?1)",params![session,response])?;
    }
    Ok(())
}
fn jsonl(store: &Store, path: &Path, harness: &str, redactor: &Redactor) -> Result<usize> {
    let source = path.to_string_lossy().to_string();
    let meta = fs::metadata(path)?;
    let modified = meta
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis() as i64;
    let last = store
        .conn
        .lock()
        .map_err(|_| "trace lock")?
        .query_row(
            "SELECT modified,size,offset,session FROM imports WHERE path=?1",
            [&source],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?.max(0) as u64,
                    r.get::<_, i64>(2)?.max(0) as u64,
                    r.get::<_, String>(3)?,
                ))
            },
        )
        .optional()?;
    if last
        .as_ref()
        .is_some_and(|(m, n, _, _)| *m == modified && *n == meta.len())
    {
        return Ok(0);
    }
    let mut offset = last.as_ref().map(|(_, _, o, _)| *o).unwrap_or(0);
    let mut sid = last.as_ref().map(|(_, _, _, s)| s.clone());
    if offset > meta.len() {
        offset = 0;
        sid = None;
    }
    if let Some(id) = &sid {
        let exists = store.conn.lock().map_err(|_| "trace lock")?.query_row(
            "SELECT COUNT(*) FROM sessions WHERE id=?1",
            [id],
            |r| r.get::<_, i64>(0),
        )? > 0;
        if !exists {
            let native = id.split_once(':').map(|(_, n)| n).unwrap_or(id);
            session(store, harness, native, "", "", None, modified, &source)?;
        }
    }
    let mut f = BufReader::new(fs::File::open(path)?);
    f.seek(SeekFrom::Start(offset))?;
    let mut count = 0;
    loop {
        let mut line = String::new();
        let n = f.read_line(&mut line)?;
        if n == 0 || !line.ends_with('\n') {
            break;
        }
        let index = offset;
        offset += n as u64;
        let v = match serde_json::from_str::<Value>(&line) {
            Ok(v) => v,
            Err(_) => {
                if let Some(id) = &sid {
                    event(
                        store,
                        id,
                        &format!("invalid-line-{index}"),
                        None,
                        modified,
                        index as i64,
                        "import_error",
                        &json!({"error":"INVALID_NATIVE_JSONL","byte_offset":index,"byte_length":n}),
                        None,
                        redactor,
                    )?;
                    store
                        .conn
                        .lock()
                        .map_err(|_| "trace lock")?
                        .execute("UPDATE sessions SET coverage='partial' WHERE id=?1", [id])?;
                    count += 1;
                    continue;
                }
                return Err("invalid native session header".into());
            }
        };
        let ts = timestamp(&v["timestamp"]);
        if sid.is_none() {
            let id = if harness == "pi" {
                v["id"].as_str()
            } else {
                v["sessionId"].as_str()
            }
            .map(String::from)
            .unwrap_or_else(|| path.file_stem().unwrap().to_string_lossy().to_string());
            sid = Some(session(
                store,
                harness,
                &id,
                "",
                v["cwd"].as_str().unwrap_or(""),
                v["parentSessionId"].as_str(),
                ts,
                &source,
            )?);
        }
        let sid = sid.as_ref().unwrap();
        let native_id = v["id"]
            .as_str()
            .or_else(|| v["uuid"].as_str())
            .map(String::from)
            .unwrap_or_else(|| hash(&format!("{source}:{index}")));
        let parent = v["parentId"].as_str().or_else(|| v["parentUuid"].as_str());
        let kind = v["type"].as_str().unwrap_or("unknown");
        let response = v
            .pointer("/message/responseId")
            .or_else(|| v.pointer("/message/id"))
            .and_then(Value::as_str);
        event(
            store,
            sid,
            &native_id,
            parent,
            ts,
            index as i64,
            kind,
            &v,
            response,
            redactor,
        )?;
        count += 1;
        if v["message"]["role"] == "user" {
            let title = v["message"]["content"]
                .as_str()
                .or_else(|| v.pointer("/message/content/0/text").and_then(Value::as_str))
                .unwrap_or("");
            let title = redactor.text(&title.chars().take(120).collect::<String>());
            store.conn.lock().map_err(|_| "trace lock")?.execute(
                "UPDATE sessions SET title=?2 WHERE id=?1 AND (title IS NULL OR title='')",
                params![sid, title],
            )?;
        }
    }
    if let Some(sid) = sid {
        store.conn.lock().map_err(|_|"trace lock")?.execute("INSERT INTO imports(path,modified,size,offset,session) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(path) DO UPDATE SET modified=excluded.modified,size=excluded.size,offset=excluded.offset,session=excluded.session",params![source,modified,meta.len() as i64,offset as i64,sid])?;
    }
    Ok(count)
}
fn sqlite(store: &Store, path: &Path, redactor: &Redactor) -> Result<usize> {
    if !path.is_file() {
        return Ok(0);
    }
    let c = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    c.busy_timeout(std::time::Duration::from_millis(250))?;
    let exists = |t: &str| -> bool {
        c.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
            [t],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0)
            > 0
    };
    let version2 = exists("session_v2") && exists("session_message");
    if !version2 && !exists("session") {
        return Ok(0);
    }
    let sql = if version2 {
        "SELECT id,title,directory,parent_id,time_created,time_updated FROM session_v2"
    } else {
        "SELECT id,title,directory,parent_id,time_created,time_updated FROM session"
    };
    let mut query = c.prepare(sql)?;
    let mut rows = query.query([])?;
    let mut count = 0;
    while let Some(r) = rows.next()? {
        let id: String = r.get(0)?;
        let title: String = r.get::<_, Option<String>>(1)?.unwrap_or_default();
        let cwd: String = r.get(2)?;
        let parent: Option<String> = r.get(3)?;
        let start: i64 = r.get(4)?;
        let updated: i64 = r.get(5)?;
        let source = format!("{}#{id}", path.display());
        let previous = store
            .conn
            .lock()
            .map_err(|_| "trace lock")?
            .query_row(
                "SELECT modified FROM imports WHERE path=?1",
                [&source],
                |r| r.get::<_, i64>(0),
            )
            .optional()?;
        if previous == Some(updated) {
            continue;
        }
        let sid = session(
            store,
            "opencode2",
            &id,
            &redactor.text(&title),
            &cwd,
            parent.as_deref(),
            start,
            &source,
        )?;
        let sql = if version2 {
            "SELECT id,type,time_created,data,seq FROM session_message WHERE session_id=?1 ORDER BY seq"
        } else {
            "SELECT id,'message',time_created,data,time_created FROM message WHERE session_id=?1 ORDER BY time_created,id"
        };
        let mut query = c.prepare(sql)?;
        let mut messages = query.query([&id])?;
        while let Some(r) = messages.next()? {
            let mid: String = r.get(0)?;
            let kind: String = r.get(1)?;
            let ts: i64 = r.get(2)?;
            let text: String = r.get(3)?;
            if let Ok(mut v) = serde_json::from_str::<Value>(&text) {
                if !version2 && exists("part") {
                    let mut q = c.prepare(
                        "SELECT data FROM part WHERE message_id=?1 ORDER BY time_created,id",
                    )?;
                    let parts = q
                        .query_map([&mid], |r| r.get::<_, String>(0))?
                        .collect::<std::result::Result<Vec<_>, _>>()?;
                    v["parts"] = json!(
                        parts
                            .iter()
                            .filter_map(|s| serde_json::from_str::<Value>(s).ok())
                            .collect::<Vec<_>>()
                    );
                }
                event(
                    store,
                    &sid,
                    &mid,
                    v["parentID"].as_str(),
                    ts,
                    r.get::<_, i64>(4)?,
                    &kind,
                    &v,
                    None,
                    redactor,
                )?;
                count += 1;
            }
        }
        store.conn.lock().map_err(|_| "trace lock")?.execute(
            "UPDATE sessions SET updated=?2 WHERE id=?1",
            params![sid, updated],
        )?;
        store.conn.lock().map_err(|_|"trace lock")?.execute("INSERT INTO imports(path,modified,size,offset,session) VALUES(?1,?2,0,0,?3) ON CONFLICT(path) DO UPDATE SET modified=excluded.modified",params![source,updated,sid])?;
    }
    Ok(count)
}
pub fn import(store: &Store, home: &Path, data: &Path) -> Result<Value> {
    let redactor = Redactor::new(data);
    let mut imported = 0;
    let mut failures = 0;
    for (h, root) in [
        ("pi", home.join(".pi/agent/sessions")),
        ("claude-code", home.join(".claude/projects")),
    ] {
        let mut paths = vec![];
        if files(&root, &mut paths, 0).is_err() {
            failures += 1;
            continue;
        }
        for p in paths {
            match jsonl(store, &p, h, &redactor) {
                Ok(n) => imported += n,
                Err(_) => failures += 1,
            }
        }
    }
    for p in [
        home.join(".local/share/opencode/opencode.db"),
        home.join(".local/share/opencode2/opencode.db"),
    ] {
        match sqlite(store, &p, &redactor) {
            Ok(n) => imported += n,
            Err(_) => failures += 1,
        }
    }
    let result = json!({"imported_events":imported,"failed_sources":failures,"checked_at":now()});
    store.set_setting("last_import", &result)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config;
    use std::io::Write;
    fn append(path: &Path, events: &[Value]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut f = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap();
        for v in events {
            writeln!(f, "{}", v).unwrap();
        }
    }
    #[test]
    fn imports_full_pi_and_claude_transcripts_incrementally_without_double_counting() {
        let t = tempfile::tempdir().unwrap();
        let home = t.path().join("home");
        let data = t.path().join("data");
        config::init(&data).unwrap();
        let store = Store::open(&data).unwrap();
        let ts = chrono::Utc::now().to_rfc3339();
        let pi = home.join(".pi/agent/sessions/project/pi.jsonl");
        append(
            &pi,
            &[
                json!({"type":"session","id":"pi_fixture","timestamp":ts,"cwd":"/fixture/project"}),
                json!({"type":"message","id":"a","parentId":null,"timestamp":ts,"message":{"role":"user","content":[{"type":"text","text":"run fixture"}]}}),
                json!({"type":"message","id":"b","parentId":"a","timestamp":ts,"message":{"role":"assistant","responseId":"response_fixture","content":[{"type":"toolCall","id":"tool_1","name":"bash","arguments":{"command":"printf fixture"}}]}}),
                json!({"type":"message","id":"c","parentId":"b","timestamp":ts,"message":{"role":"toolResult","toolCallId":"tool_1","content":[{"type":"text","text":"fixture output"}]}}),
                json!({"type":"compaction","id":"d","parentId":"c","timestamp":ts,"summary":"fixture summary"}),
            ],
        );
        let cc = home.join(".claude/projects/project/cc.jsonl");
        append(
            &cc,
            &[
                json!({"type":"user","uuid":"u","parentUuid":null,"sessionId":"cc_fixture","timestamp":ts,"message":{"role":"user","content":"hello"}}),
                json!({"type":"assistant","uuid":"a","parentUuid":"u","sessionId":"cc_fixture","timestamp":ts,"message":{"id":"msg_fixture","role":"assistant","content":[{"type":"tool_use","id":"tu_1","name":"Read","input":{"file_path":"fixture.txt"}}]}}),
                json!({"type":"user","uuid":"r","parentUuid":"a","sessionId":"cc_fixture","timestamp":ts,"message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tu_1","content":"fixture text"}]}}),
            ],
        );
        let first = import(&store, &home, &data).unwrap();
        assert_eq!(first["imported_events"], 8);
        assert_eq!(first["failed_sources"], 0);
        assert_eq!(import(&store, &home, &data).unwrap()["imported_events"], 0);
        let session = store.session("pi:pi_fixture", 0).unwrap();
        assert_eq!(session["total_events"], 5);
        assert!(
            serde_json::to_string(&session)
                .unwrap()
                .contains("fixture output")
        );
        assert_eq!(
            store.session("claude-code:cc_fixture", 0).unwrap()["total_events"],
            3
        );
        assert_eq!(
            store.stats(&crate::telemetry::Filter::default()).unwrap()["summary"]["requests"],
            0
        );
        // An incomplete final JSONL line must not be consumed before the writer appends its newline.
        let mut f = fs::OpenOptions::new().append(true).open(&pi).unwrap();
        write!(f,"{{\"type\":\"message\",\"id\":\"new\",\"timestamp\":\"{ts}\",\"message\":{{\"role\":\"assistant\",\"content\":\"done\"}}}}").unwrap();
        drop(f);
        assert_eq!(import(&store, &home, &data).unwrap()["imported_events"], 0);
        fs::OpenOptions::new()
            .append(true)
            .open(&pi)
            .unwrap()
            .write_all(b"\n")
            .unwrap();
        assert_eq!(import(&store, &home, &data).unwrap()["imported_events"], 1);
    }
    #[test]
    fn reads_opencode_v2_read_only_and_preserves_tool_events() {
        let t = tempfile::tempdir().unwrap();
        let home = t.path().join("home");
        let data = t.path().join("data");
        config::init(&data).unwrap();
        let store = Store::open(&data).unwrap();
        let db = home.join(".local/share/opencode/opencode.db");
        fs::create_dir_all(db.parent().unwrap()).unwrap();
        let c = Connection::open(&db).unwrap();
        c.execute_batch("CREATE TABLE session_v2(id TEXT,title TEXT,directory TEXT,parent_id TEXT,time_created INTEGER,time_updated INTEGER);CREATE TABLE session_message(id TEXT,session_id TEXT,type TEXT,seq INTEGER,time_created INTEGER,data TEXT);").unwrap();
        c.execute(
            "INSERT INTO session_v2 VALUES('ses_untitled',NULL,'/fixture',NULL,1,1)",
            [],
        )
        .unwrap();
        c.execute(
            "INSERT INTO session_v2 VALUES('ses_fixture','fixture session','/fixture',NULL,1,2)",
            [],
        )
        .unwrap();
        c.execute("INSERT INTO session_message VALUES('m1','ses_fixture','assistant',1,1,?1)",[json!({"content":[{"type":"tool","name":"bash","input":{"command":"printf fixture"},"output":"fixture output"}]}).to_string()]).unwrap();
        drop(c);
        let original = fs::read(&db).unwrap();
        assert_eq!(import(&store, &home, &data).unwrap()["imported_events"], 1);
        assert_eq!(import(&store, &home, &data).unwrap()["imported_events"], 0);
        assert_eq!(fs::read(&db).unwrap(), original);
        let v = store.session("opencode2:ses_fixture", 0).unwrap();
        assert_eq!(v["total_events"], 1);
        assert!(v["events"][0]["payload"]["content"][0]["output"] == "fixture output");
        assert_eq!(store.sessions("opencode2", "", 0).unwrap()["total"], 2);
    }
    #[test]
    fn expired_session_can_resume_and_corrupt_rows_are_visible() {
        let t = tempfile::tempdir().unwrap();
        let home = t.path().join("home");
        let data = t.path().join("data");
        config::init(&data).unwrap();
        let store = Store::open(&data).unwrap();
        let pi = home.join(".pi/agent/sessions/project/session.jsonl");
        let ts = chrono::Utc::now().to_rfc3339();
        append(
            &pi,
            &[json!({"type":"session","id":"resume_fixture","timestamp":ts})],
        );
        import(&store, &home, &data).unwrap();
        let old = now() - 31 * 86400000;
        {
            let c = store.conn.lock().unwrap();
            c.execute("UPDATE sessions SET updated=?1", [old]).unwrap();
            c.execute("UPDATE events SET ts=?1", [old]).unwrap();
        }
        store.retention().unwrap();
        assert_eq!(store.sessions("", "", 0).unwrap()["total"], 0);
        append(
            &pi,
            &[
                json!({"type":"message","id":"new","timestamp":ts,"message":{"role":"user","content":"resumed"}}),
            ],
        );
        import(&store, &home, &data).unwrap();
        assert_eq!(
            store.session("pi:resume_fixture", 0).unwrap()["total_events"],
            1
        );
        fs::OpenOptions::new()
            .append(true)
            .open(&pi)
            .unwrap()
            .write_all(b"{invalid json}\n")
            .unwrap();
        import(&store, &home, &data).unwrap();
        let v = store.session("pi:resume_fixture", 0).unwrap();
        assert_eq!(v["session"]["coverage"], "partial");
        assert!(
            v["events"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["kind"] == "import_error")
        );
    }
}
