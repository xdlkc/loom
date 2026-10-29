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
#[derive(Clone, Debug)]
pub struct Model {
    pub id: String,
    pub p: Value,
    pub m: Value,
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
        let derived = pid
            .strip_prefix("local-")
            .unwrap_or(pid)
            .strip_suffix("-plan")
            .unwrap_or(pid.strip_prefix("local-").unwrap_or(pid));
        let prefix = p["prefix"].as_str().unwrap_or(derived);
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
        for m in p["models"].as_array().ok_or("models required")? {
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
pub fn sync(d: &Path, h: &Path) -> Result<()> {
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
    let op = h.join(".config/opencode/opencode.json");
    if op.with_extension("jsonc").exists() {
        return Err("JSONC exists; not overwritten".into());
    }
    let mut pi = read_or(&pp)?;
    let mut settings = read_or(&sp)?;
    let mut oc = read_or(&op)?;
    let label = |m: &Model| {
        format!(
            "hub/{} · {}",
            m.m["name"].as_str().unwrap_or(m.m["id"].as_str().unwrap()),
            m.id.split('/').next().unwrap()
        )
    };
    pi["providers"]["hub"] = json!({"name":"hub","baseUrl":"http://127.0.0.1:4319/v1","api":"openai-completions","apiKey":format!("!cat {}",quote(&key)),"authHeader":true,"models":models.iter().map(|m|{let mut v=m.m.clone();v["id"]=json!(m.id);v["name"]=json!(label(m));v["compat"]=json!({"supportsStore":false,"supportsDeveloperRole":false,"supportsStrictMode":false,"maxTokensField":"max_tokens","requiresReasoningContentOnAssistantMessages":true});v}).collect::<Vec<_>>()});
    let entries:serde_json::Map<String,Value>=models.iter().map(|m|(m.id.clone(),json!({"name":label(m),"capabilities":{"tools":true,"input":m.m.get("input").cloned().unwrap_or(json!(["text"])),"output":["text"]},"limit":{"context":m.m["contextWindow"].as_u64().unwrap_or(128000),"output":m.m["maxTokens"].as_u64().unwrap_or(8192)}}))).collect();
    oc["providers"]["hub"] = json!({"name":"hub","package":"@opencode/ai/providers/openai-compatible","settings":{"baseURL":"http://127.0.0.1:4319/v1","apiKey":format!("{{file:{}}}",key.display())},"models":entries});
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
        c["env"]["ANTHROPIC_BASE_URL"] = json!("http://127.0.0.1:4319");
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
                o["label"] = json!(label(m));
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
mod tests {
    use super::*;
    #[test]
    fn sync_preserves_oauth_preferences_and_never_exports_upstream_keys() {
        let t = tempfile::tempdir().unwrap();
        let d = t.path().join("data");
        let h = t.path().join("home");
        init(&d).unwrap();
        let kp = d.join("keys/test.key");
        write(&kp, b"UPSTREAM_FIXTURE").unwrap();
        save(&d.join("providers.json"),&json!({"proxy":"","providers":{"local-test":{"prefix":"test","name":"Test","enabled":true,"baseURL":"https://api.deepseek.com","keyFile":kp,"models":[{"id":"deepseek-flash","name":"Flash"}]}}})).unwrap();
        save(
            &h.join(".pi/agent/models.json"),
            &json!({"providers":{"oauth":{"name":"original"}}}),
        )
        .unwrap();
        save(
            &h.join(".claude/settings.json"),
            &json!({"theme":"dark","permissions":{"defaultMode":"manual"}}),
        )
        .unwrap();
        sync(&d, &h).unwrap();
        let pi = read(&h.join(".pi/agent/models.json")).unwrap();
        assert_eq!(pi["providers"]["oauth"]["name"], "original");
        let c = read(&h.join(".claude/settings.json")).unwrap();
        assert_eq!(c["theme"], "dark");
        assert_eq!(c["permissions"]["defaultMode"], "manual");
        assert!(
            !serde_json::to_string(&c)
                .unwrap()
                .contains("UPSTREAM_FIXTURE")
        );
        assert!(c["apiKeyHelper"].as_str().unwrap().contains("hub.key"));
        let before = fs::read(h.join(".claude/settings.json")).unwrap();
        sync(&d, &h).unwrap();
        assert_eq!(before, fs::read(h.join(".claude/settings.json")).unwrap());
    }
    #[test]
    fn registry_rejects_duplicate_or_insecure_routes() {
        let mut r = json!({"providers":{"local-test":{"enabled":true,"baseURL":"http://unsafe.invalid","keyFile":"fixture","models":[{"id":"x"}]}}});
        assert!(catalog(&r).is_err());
        r["providers"]["local-test"]["baseURL"] = json!("https://safe.invalid");
        r["providers"]["local-test"]["models"] = json!([{"id":"x"},{"id":"x"}]);
        assert!(catalog(&r).is_err());
    }
}
