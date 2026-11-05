use rand::RngCore;
use serde_json::{Value, json};
use std::{
    fs, io,
    path::{Path, PathBuf},
};
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
pub fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .expect("HOME required")
}
pub fn data_dir() -> PathBuf {
    std::env::var_os("MODEL_HUB_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            home().join(if cfg!(target_os = "macos") {
                "Library/Application Support/ModelHub/.local"
            } else {
                ".local/share/ModelHub/.local"
            })
        })
}
pub fn read(p: &Path) -> Result<Value> {
    Ok(serde_json::from_slice(&fs::read(p)?)?)
}
pub fn read_or(p: &Path) -> Result<Value> {
    if p.exists() { read(p) } else { Ok(json!({})) }
}
pub fn write(p: &Path, data: &[u8]) -> Result<()> {
    use std::io::Write;
    fs::create_dir_all(p.parent().ok_or("parent required")?)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(p.parent().unwrap(), fs::Permissions::from_mode(0o700))?;
    }
    let t = p.with_extension(format!("tmp-{:016x}", rand::random::<u64>()));
    let mut opts = fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(&t)?;
    f.write_all(data)?;
    f.sync_all()?;
    fs::rename(t, p)?;
    Ok(())
}
pub fn save(p: &Path, v: &Value) -> Result<()> {
    write(p, &serde_json::to_vec_pretty(v)?)
}
pub fn backup(p: &Path, d: &Path) -> Result<()> {
    if p.exists() {
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        write(
            &d.join("backups")
                .join(format!("{n}-{}", p.file_name().unwrap().to_string_lossy())),
            &fs::read(p)?,
        )?;
    }
    Ok(())
}
pub fn init(d: &Path) -> Result<()> {
    if !d.join("providers.json").exists() {
        save(
            &d.join("providers.json"),
            &json!({"proxy":"","providers":{}}),
        )?;
    }
    if !d.join("hub.key").exists() {
        let mut b = [0u8; 32];
        rand::rng().fill_bytes(&mut b);
        write(
            &d.join("hub.key"),
            format!(
                "hub-{}\n",
                b.iter().map(|v| format!("{v:02x}")).collect::<String>()
            )
            .as_bytes(),
        )?;
    }
    Ok(())
}
pub fn endpoint(base: &str) -> Result<String> {
    let u = reqwest::Url::parse(base)?;
    if u.scheme() != "https"
        || !u.username().is_empty()
        || u.password().is_some()
        || u.query().is_some()
        || u.fragment().is_some()
    {
        return Err("HTTPS endpoint without credentials required".into());
    }
    Ok(base.trim_end_matches('/').into())
}
pub fn anthropic(p: &Value) -> Result<Option<String>> {
    let base = p["baseURL"].as_str().ok_or("baseURL required")?;
    let native = p["anthropicBaseURL"]
        .as_str()
        .or(match base.trim_end_matches('/') {
            "https://zenmux.ai/api/v1" => Some("https://zenmux.ai/api/anthropic"),
            "https://open.bigmodel.cn/api/coding/paas/v4" => {
                Some("https://open.bigmodel.cn/api/anthropic")
            }
            "https://api.z.ai/api/coding/paas/v4" => Some("https://api.z.ai/api/anthropic"),
            "https://api.deepseek.com" => Some("https://api.deepseek.com/anthropic"),
            "https://api.kimi.com/coding/v1" => Some("https://api.kimi.com/coding"),
            "https://api.minimax.cn/v1" => Some("https://api.minimax.cn/anthropic"),
            "https://api.minimaxi.com/v1" => Some("https://api.minimaxi.com/anthropic"),
            "https://api.minimax.io/v1" => Some("https://api.minimax.io/anthropic"),
            "https://api.longcat.chat/openai/v1" => Some("https://api.longcat.chat/anthropic"),
            _ => None,
        });
    native
        .map(|v| {
            let b = endpoint(v)?;
            Ok(format!(
                "{b}{}",
                if b.ends_with("/v1") {
                    "/messages"
                } else {
                    "/v1/messages"
                }
            ))
        })
        .transpose()
}
// LongCat's Anthropic-compatible endpoint explicitly requires Bearer authentication.
// Determine this from the resolved upstream URL, not a user-selected provider prefix.
pub fn anthropic_bearer(p: &Value) -> bool {
    anthropic(p)
        .ok()
        .flatten()
        .and_then(|s| reqwest::Url::parse(&s).ok())
        .is_some_and(|u| u.host_str() == Some("api.longcat.chat"))
}
#[derive(Clone, Debug)]
pub struct Model {
    pub id: String,
    pub p: Value,
    pub m: Value,
}
pub fn provider_prefix<'a>(pid: &'a str, p: &'a Value) -> &'a str {
    let short = pid.strip_prefix("local-").unwrap_or(pid);
    p["prefix"]
        .as_str()
        .unwrap_or_else(|| short.strip_suffix("-plan").unwrap_or(short))
}
pub fn catalog(r: &Value) -> Result<Vec<Model>> {
    let mut out = vec![];
    let mut seen = std::collections::HashSet::new();
    for (pid, p) in r["providers"].as_object().ok_or("providers required")? {
        if p["enabled"] != true {
            continue;
        }
        endpoint(p["baseURL"].as_str().ok_or("baseURL required")?)?;
        anthropic(p)?;
        let prefix = provider_prefix(pid, p);
        if prefix.is_empty()
            || !prefix.as_bytes()[0].is_ascii_lowercase()
            || !prefix
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return Err("invalid prefix".into());
        }
        if p["keyFile"].as_str().is_none() {
            return Err("keyFile required".into());
        }
        let mut models = p["models"].as_array().ok_or("models required")?.clone();
        crate::bundled::supplement(p, &mut models)?;
        for m in &models {
            if m["enabled"] == false {
                continue;
            }
            let mid = m["id"].as_str().ok_or("model id required")?;
            if mid.is_empty() || mid.chars().any(|c| c.is_whitespace() || "?#".contains(c)) {
                return Err("invalid model id".into());
            }
            let id = format!("{prefix}/{mid}");
            if !seen.insert(id.clone()) {
                return Err("duplicate model".into());
            }
            out.push(Model {
                id,
                p: p.clone(),
                m: m.clone(),
            });
        }
    }
    Ok(out)
}
fn quote(p: &Path) -> String {
    format!("'{}'", p.to_string_lossy().replace('\'', "'\\''"))
}
// Clients only see loopback, so their URL-based vendor detection cannot work.
// Use the actual upstream host, never a user-chosen route prefix or model name.
fn direct_deepseek(m: &Model) -> bool {
    m.m["reasoning"] == true
        && m.p["baseURL"]
            .as_str()
            .and_then(|s| reqwest::Url::parse(s).ok())
            .is_some_and(|u| u.host_str() == Some("api.deepseek.com"))
}
fn pi_model(m: &Model, label: String) -> Value {
    let mut v = m.m.clone();
    v["id"] = json!(m.id);
    v["name"] = json!(label);
    let mut compat = json!({"supportsStore":false,"supportsDeveloperRole":false,"supportsStrictMode":false,"maxTokensField":"max_tokens","requiresReasoningContentOnAssistantMessages":true});
    if direct_deepseek(m) {
        compat["thinkingFormat"] = json!("deepseek");
        compat["supportsReasoningEffort"] = json!(true);
        // Match DeepSeek's documented effort mapping without changing user defaults.
        let mut levels = json!({"minimal":"low","low":"low","medium":"high","high":"high","xhigh":"high","max":"max"});
        if let Some(custom) = m.m["thinkingLevelMap"].as_object() {
            levels.as_object_mut().unwrap().extend(custom.clone());
        }
        v["thinkingLevelMap"] = levels;
    }
    for overrides in [&m.p["piCompat"], &m.m["compat"]] {
        if let Some(extra) = overrides.as_object() {
            compat.as_object_mut().unwrap().extend(extra.clone());
        }
    }
    v["compat"] = compat;
    v
}
fn opencode_model(m: &Model, label: String) -> Value {
    let mut v = json!({"name":label,"capabilities":{"tools":true,"reasoning":{"supported":m.m["reasoning"] == true},"input":m.m.get("input").cloned().unwrap_or(json!(["text"])),"output":["text"]},"limit":{"context":m.m["contextWindow"].as_u64().unwrap_or(128000),"output":m.m["maxTokens"].as_u64().unwrap_or(8192)}});
    if direct_deepseek(m) {
        v["capabilities"]["reasoning"] =
            json!({"supported":true,"toggle":true,"effort":["low","high","max"]});
        v["compatibility"] = json!({"reasoningField":"reasoning_content","requireReasoning":true});
        // The shared OpenAI-compatible package cannot infer DeepSeek's toggle.
        // Explicit body overlays also avoid sending an effort alongside disabled.
        let mut variants = vec![json!({"id":"none","body":{"thinking":{"type":"disabled"}}})];
        for effort in ["low", "high", "max"] {
            variants.push(json!({"id":effort,"body":{"thinking":{"type":"enabled"},"reasoning_effort":effort}}));
        }
        v["variants"] = json!(variants);
    }
    v
}
// A JSONC file without comments is ordinary JSON. Comments are detected outside
// string literals so URLs such as https://example.invalid remain valid.
fn has_jsonc_comment(raw: &[u8]) -> bool {
    let mut string = false;
    let mut escape = false;
    for (i, b) in raw.iter().copied().enumerate() {
        if string {
            if escape {
                escape = false;
            } else if b == b'\\' {
                escape = true;
            } else if b == b'"' {
                string = false;
            }
        } else if b == b'"' {
            string = true;
        } else if b == b'/'
            && matches!(raw.get(i + 1).copied(), Some(b'/' | b'*'))
        {
            return true;
        }
    }
    false
}
fn opencode_config_path(h: &Path) -> Result<PathBuf> {
    let json = h.join(".config/opencode/opencode.json");
    let jsonc = json.with_extension("jsonc");
    match (json.exists(), jsonc.exists()) {
        (true, true) => Err("JSON and JSONC client configs both exist; not overwritten".into()),
        (false, true) => {
            let raw = fs::read(&jsonc)?;
            if has_jsonc_comment(&raw) {
                return Err("JSONC client config contains comments; not overwritten".into());
            }
            if serde_json::from_slice::<Value>(&raw).is_err() {
                return Err("invalid JSONC client config; not overwritten".into());
            }
            Ok(jsonc)
        }
        _ => Ok(json),
    }
}
pub fn sync(d: &Path, h: &Path) -> Result<()> {
    sync_for_port(d, h, 4319)
}
pub fn sync_for_port(d: &Path, h: &Path, port: u16) -> Result<()> {
    let models = catalog(&read(&d.join("providers.json"))?)?;
    if models.is_empty() {
        return Err("add a provider first".into());
    }
    let fallback = models
        .iter()
        .find(|x| x.id == "glm/glm-5.3")
        .unwrap_or(&models[0])
        .id
        .clone();
    let key = d.join("hub.key");
    let pp = h.join(".pi/agent/models.json");
    let sp = h.join(".pi/agent/settings.json");
    let op = opencode_config_path(h)?;
    let mut pi = read_or(&pp)?;
    let mut settings = read_or(&sp)?;
    let mut oc = read_or(&op)?;
    let label = |m: &Model| {
        let raw = m.m["name"].as_str().unwrap_or(m.m["id"].as_str().unwrap());
        let name = raw
            .split_once(": ")
            .map(|(_, v)| v)
            .unwrap_or(raw)
            .trim_start_matches("Claude ")
            .trim_start_matches("claude-")
            .replace("glm-", "GLM ")
            .replace("DeepSeek-", "DeepSeek ")
            .replace('-', " ");
        format!("hub/{name} · {}", m.id.split('/').next().unwrap())
    };
    pi["providers"]["hub"] = json!({"name":"hub","baseUrl":format!("http://127.0.0.1:{port}/v1"),"api":"openai-completions","apiKey":format!("!cat {}",quote(&key)),"authHeader":true,"models":models.iter().map(|m| pi_model(m,label(m))).collect::<Vec<_>>()});
    let entries: serde_json::Map<String, Value> = models
        .iter()
        .map(|m| (m.id.clone(), opencode_model(m, label(m))))
        .collect();
    oc["providers"]["hub"] = json!({"name":"hub","package":"@opencode/ai/providers/openai-compatible","settings":{"baseURL":format!("http://127.0.0.1:{port}/v1"),"apiKey":format!("{{file:{}}}",key.display())},"models":entries});
    if let Some(enabled) = settings["enabledModels"].as_array() {
        let mut list: Vec<Value> = enabled
            .iter()
            .filter(|x| !x.as_str().is_some_and(|s| s.starts_with("hub/")))
            .cloned()
            .collect();
        list.extend(models.iter().map(|m| json!(format!("hub/{}", m.id))));
        settings["enabledModels"] = json!(list);
    }
    if settings["defaultProvider"].is_null() || settings["defaultProvider"] == "hub" {
        settings["defaultProvider"] = json!("hub");
        if !models
            .iter()
            .any(|m| Some(m.id.as_str()) == settings["defaultModel"].as_str())
        {
            settings["defaultModel"] = json!(fallback);
        }
    }
    if oc["model"].is_null()
        || oc["model"].as_str().is_some_and(|v| {
            v.starts_with("hub/") && !models.iter().any(|m| format!("hub/{}", m.id) == v)
        })
    {
        oc["model"] = json!(format!("hub/{fallback}"));
    }
    let pi_extension = h.join(".pi/agent/extensions/loom-observability.ts");
    let oc_plugin = h.join(".config/opencode/plugins/loom-observability");
    for (file, source) in [
        (&pi_extension, include_str!("../integrations/pi.ts")),
        (
            &oc_plugin.join("index.ts"),
            include_str!("../integrations/opencode2.ts"),
        ),
    ] {
        if std::fs::read_to_string(file).ok().as_deref() != Some(source) {
            backup(file, d)?;
            write(file, source.as_bytes())?;
        }
    }
    let mut plugins = oc["plugins"].as_array().cloned().unwrap_or_default();
    if !plugins
        .iter()
        .any(|p| p.as_str() == oc_plugin.to_str() || p["package"].as_str() == oc_plugin.to_str())
    {
        plugins.push(json!({"package":oc_plugin,"options":{}}));
    }
    oc["plugins"] = json!(plugins);
    for (p, v) in [(&pp, &pi), (&sp, &settings), (&op, &oc)] {
        if read_or(p)? != *v {
            backup(p, d)?;
            save(p, v)?;
        }
    }
    let cp = h.join(".claude/settings.json");
    if cp.exists() {
        let mut c = read(&cp)?;
        c["apiKeyHelper"] = json!(format!("cat {}", quote(&key)));
        c["env"]["ANTHROPIC_BASE_URL"] = json!(format!("http://127.0.0.1:{port}"));
        c["env"]["ANTHROPIC_API_KEY"] = json!("");
        c["env"]["ANTHROPIC_AUTH_TOKEN"] = json!("");
        let native: Vec<_> = models
            .iter()
            .filter(|m| anthropic(&m.p).ok().flatten().is_some())
            .collect();
        if !native.is_empty() {
            let mut options = c["modelPicker"]["options"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            let prefixes: Vec<_> = models
                .iter()
                .map(|m| m.id.split('/').next().unwrap())
                .collect();
            let old = options.clone();
            options.retain(|o| {
                !o["model"]
                    .as_str()
                    .is_some_and(|id| prefixes.iter().any(|p| id.starts_with(&format!("{p}/"))))
            });
            for m in &native {
                let mut o = old
                    .iter()
                    .find(|o| o["model"] == m.id)
                    .cloned()
                    .unwrap_or(json!({}));
                o["model"] = json!(m.id);
                if o["label"].is_null() {
                    o["label"] = json!(label(m));
                }
                o["description"] = json!(format!(
                    "{}/{}",
                    m.id.split('/').next().unwrap(),
                    m.id.split('/').next_back().unwrap()
                ));
                options.push(o);
            }
            c["modelPicker"]["options"] = json!(options);
            c["modelPicker"]["replaceBuiltInOptions"] = json!(true);
            if c["model"].is_null() {
                c["model"] = json!(native[0].id);
            }
        }
        if read(&cp)? != c {
            backup(&cp, d)?;
            save(&cp, &c)?;
        }
    }
    Ok(())
}
pub fn prompt(s: &str) -> Result<String> {
    use std::io::Write;
    print!("{s}: ");
    io::stdout().flush()?;
    let mut v = String::new();
    io::stdin().read_line(&mut v)?;
    Ok(v.trim().into())
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
