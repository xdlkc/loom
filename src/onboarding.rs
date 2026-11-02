//! Local, hidden-input onboarding. Presets contain public routing metadata only.
use crate::{
    config::{self, Result},
    validate_key,
};
use serde_json::{Value, json};
use std::{
    io::IsTerminal,
    path::{Path, PathBuf},
};

struct Preset {
    id: &'static str,
    name: &'static str,
    base: &'static str,
    billing: &'static str,
    key_hint: &'static str,
    models: &'static [(&'static str, &'static str)],
}
// Public API documentation and model IDs; never replace an existing provider with these.
// See README's preset sources. Limits are conservative defaults, not account entitlements.
const PRESETS: &[Preset] = &[
    Preset {
        id: "zenmux",
        name: "ZenMux",
        base: "https://zenmux.ai/api/v1",
        billing: "ZenMux 推理 Key 对应套餐",
        key_hint: "推理 API Key（不是 Management Key）",
        models: &[
            ("anthropic/claude-sonnet-5.5", "Claude Sonnet 5.5"),
            ("anthropic/claude-opus-5.5", "Claude Opus 5.5"),
            ("anthropic/claude-haiku-5.5", "Claude Haiku 5.5"),
        ],
    },
    Preset {
        id: "glm",
        name: "GLM · 国内 Coding Plan",
        base: "https://open.bigmodel.cn/api/coding/paas/v4",
        billing: "国内 Coding Plan",
        key_hint: "Coding Plan 套餐 API Key",
        models: &[
            ("glm-5.3", "GLM 5.3"),
            ("glm-5.3-flash", "GLM 5.3 Flash"),
            ("glm-5.3-flashx", "GLM 5.3 FlashX"),
        ],
    },
    Preset {
        id: "minimax",
        name: "MiniMax · 国内订阅",
        base: "https://api.minimax.cn/v1",
        billing: "国内 Coding / Token / M Plan",
        key_hint: "订阅 Key（不是普通 API Key）",
        models: &[("MiniMax-M3.1-Flash", "MiniMax M3.1 Flash")],
    },
    Preset {
        id: "deepseek",
        name: "DeepSeek · API 钱包",
        base: "https://api.deepseek.com",
        billing: "API 钱包",
        key_hint: "开放平台 API Key",
        models: &[
            ("deepseek-flash", "DeepSeek Flash"),
            ("deepseek-v4-pro", "DeepSeek V4 Pro"),
        ],
    },
    Preset {
        id: "kimi",
        name: "Kimi Code · 国内订阅",
        base: "https://api.kimi.com/coding/v1",
        billing: "Kimi Code 订阅",
        key_hint: "Kimi Code API Key（不是 Moonshot 开放平台 Key）",
        models: &[
            ("k3.1-pro", "K3.1 Pro"),
            ("k3.1-pro-highspeed", "K3.1 Pro Highspeed"),
        ],
    },
    Preset {
        id: "longcat",
        name: "LongCat · 美团",
        base: "https://api.longcat.chat/openai/v1",
        billing: "LongCat API 钱包 / Token 资源包",
        key_hint: "LongCat 开放平台 API Key（不是网页登录令牌）",
        models: &[("LongCat-2.5", "LongCat 2.5")],
    },
];

trait Input {
    fn line(&mut self, prompt: &str) -> Result<String>;
    fn secret(&mut self, prompt: &str) -> Result<String>;
    fn say(&mut self, message: &str);
}
struct Terminal;
impl Input for Terminal {
    fn line(&mut self, prompt: &str) -> Result<String> {
        config::prompt(prompt)
    }
    fn secret(&mut self, prompt: &str) -> Result<String> {
        Ok(rpassword::prompt_password(format!("{prompt}: "))?)
    }
    fn say(&mut self, message: &str) {
        println!("{message}");
    }
}

fn selection(s: &str) -> Result<Vec<usize>> {
    if s.trim().is_empty() || s.trim().eq_ignore_ascii_case("all") || s.trim() == "全部" {
        return Ok((0..PRESETS.len()).collect());
    }
    if s.trim() == "0" {
        return Ok(vec![]);
    }
    let mut chosen = vec![];
    for part in s
        .split(|c: char| c == ',' || c == '，' || c.is_whitespace())
        .filter(|p| !p.is_empty())
    {
        let index = part
            .parse::<usize>()
            .ok()
            .and_then(|n| n.checked_sub(1))
            .filter(|n| *n < PRESETS.len())
            .or_else(|| PRESETS.iter().position(|p| p.id.eq_ignore_ascii_case(part)))
            .ok_or("请选择列表中的编号或供应商名称")?;
        if !chosen.contains(&index) {
            chosen.push(index);
        }
    }
    if chosen.is_empty() {
        return Err("empty selection".into());
    }
    Ok(chosen)
}
fn models(ids: &[(&str, &str)], reasoning: bool) -> Value {
    json!(
        ids.iter()
            .map(|(id, name)| json!({
                "id":id, "name":name, "contextWindow":128000, "maxTokens":8192,
                "input":["text"], "reasoning":reasoning
            }))
            .collect::<Vec<_>>()
    )
}
fn preset_provider(p: &Preset, key: &Path) -> Value {
    let mut value = json!({"prefix":p.id,"name":p.name,"baseURL":p.base,"billing":p.billing,
        "enabled":true,"keyFile":key,"models":models(p.models, true)});
    if p.id == "longcat" {
        // LongCat uses the same enabled/disabled thinking object, not DeepSeek's effort levels.
        value["piCompat"] = json!({"thinkingFormat":"deepseek","supportsReasoningEffort":false});
    }
    value
}
pub fn needs_setup(d: &Path) -> Result<bool> {
    if !d.join("providers.json").exists() {
        return Ok(true);
    }
    let r = config::read(&d.join("providers.json"))?;
    Ok(r["providers"]
        .as_object()
        .ok_or("providers required")?
        .is_empty())
}
pub fn configure(d: &Path, requested: &[String], custom: bool) -> Result<()> {
    let selected = if requested.is_empty() {
        None
    } else {
        let chosen = selection(&requested.join(",")).map_err(|_| {
            eprintln!("供应商可选：zenmux / glm / minimax / deepseek / kimi / longcat；其他供应商用 loom add --custom。");
            "unknown provider"
        })?;
        Some(chosen)
    };
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        eprintln!("请在本机交互终端运行 loom add；API Key 只接受隐藏输入，不接受命令参数或管道。");
        return Err("interactive terminal required".into());
    }
    wizard(d, &config::home(), selected, custom, &mut Terminal)
}
fn wizard(
    d: &Path,
    h: &Path,
    selected: Option<Vec<usize>>,
    custom: bool,
    io: &mut dyn Input,
) -> Result<()> {
    config::init(d)?;
    // Freeze absolute key paths even when --data-dir is relative.
    let d = std::fs::canonicalize(d)?;
    let original = config::read(&d.join("providers.json"))?;
    config::catalog(&original)?;
    if custom {
        custom_provider(&d, io)?;
    } else {
        let selected = match selected {
            Some(s) => s,
            None => {
                io.say("选择供应商（默认全部）。只需填 Key，名称、地址和模型自动配置。");
                for (i, p) in PRESETS.iter().enumerate() {
                    io.say(&format!("  [✓] {}. {}", i + 1, p.name));
                }
                loop {
                    let answer = io.line("回车全部 / 编号多选如 1,4 / 0 退出")?;
                    match selection(&answer) {
                        Ok(s) => break s,
                        Err(_) => io.say(&format!(
                            "请输入 1–{} 的编号（逗号分隔）或供应商名称；回车选择全部。",
                            PRESETS.len()
                        )),
                    }
                }
            }
        };
        for index in selected {
            configure_preset(&d, &PRESETS[index], io)?;
        }
    }
    if config::catalog(&config::read(&d.join("providers.json"))?)?.is_empty() {
        io.say("未添加供应商。之后运行 loom add 即可继续。");
    } else {
        config::sync(&d, h).inspect_err(|_| {
            io.say("Key 已保存在本机；客户端同步未完成，处理客户端配置后运行 loom sync。")
        })?;
        io.say("已保存并同步客户端。配置即时生效，运行中的 agent 请刷新模型列表。");
    }
    Ok(())
}
fn read_key(io: &mut dyn Input, prompt: &str) -> Result<String> {
    loop {
        let key = io.secret(prompt)?;
        if key.trim().is_empty() {
            return Ok(String::new());
        }
        if validate_key(&key).is_ok() {
            return Ok(key.trim().to_string());
        }
        io.say("Key 格式无效，请重新粘贴；回车跳过。未保存此次输入。");
    }
}
fn configure_preset(d: &Path, p: &Preset, io: &mut dyn Input) -> Result<()> {
    let registry = d.join("providers.json");
    let before = std::fs::read(&registry)?;
    let mut r: Value = serde_json::from_slice(&before)?;
    let matches: Vec<_> = r["providers"]
        .as_object()
        .ok_or("providers required")?
        .iter()
        .filter(|(pid, v)| config::provider_prefix(pid, v) == p.id)
        .map(|(pid, _)| pid.clone())
        .collect();
    if matches.len() > 1 {
        return Err("ambiguous existing providers; use loom set-key with exact id".into());
    }
    let existing = matches.first();
    let pid = existing
        .cloned()
        .unwrap_or_else(|| format!("local-{}", p.id));
    let key_path = if existing.is_some() {
        PathBuf::from(
            r["providers"][&pid]["keyFile"]
                .as_str()
                .ok_or("keyFile required")?,
        )
    } else {
        if r["providers"].get(&pid).is_some() {
            return Err("provider id already occupied".into());
        }
        d.join("keys").join(format!("{pid}.key"))
    };
    let action = if existing.is_some() {
        "已配置，回车保留 / 输入新 Key 更换"
    } else {
        "回车跳过"
    };
    io.say(&format!("\n{} · {}", p.name, p.key_hint));
    let key = read_key(io, &format!("API Key（隐藏输入；{action}）"))?;
    if key.is_empty() {
        return Ok(());
    }
    if std::fs::read(&registry)? != before {
        return Err("configuration changed; retry".into());
    }
    if existing.is_some() {
        // Key rotation must preserve every user-defined field, path, disabled state and model.
        config::backup(&key_path, d)?;
        config::write(&key_path, key.as_bytes())?;
    } else {
        if key_path.exists() {
            return Err("key file already exists; not overwritten".into());
        }
        r["providers"][&pid] = preset_provider(p, &key_path);
        config::catalog(&r)?;
        config::backup(&registry, d)?;
        config::write(&key_path, key.as_bytes())?;
        if let Err(e) = config::save(&registry, &r) {
            let _ = std::fs::remove_file(&key_path);
            return Err(e);
        }
    }
    io.say("已保存。Key 只保留在本机。");
    Ok(())
}
fn with_default(io: &mut dyn Input, prompt: &str, default: &str) -> Result<String> {
    let s = io.line(&format!("{prompt} [{default}]"))?;
    Ok(if s.is_empty() { default.into() } else { s })
}
fn custom_provider(d: &Path, io: &mut dyn Input) -> Result<()> {
    let prefix = io.line("自定义供应商标识（小写字母、数字、短横线）")?;
    let name = with_default(io, "显示名称", &prefix)?;
    let base = config::endpoint(&io.line("OpenAI 兼容 Base URL（HTTPS）")?)?;
    let native = io.line("Anthropic Base URL（可留空）")?;
    if !native.is_empty() {
        config::endpoint(&native)?;
    }
    let billing = with_default(io, "计费来源", "独立 API 钱包")?;
    let ids = io.line("Model ID（逗号分隔）")?;
    let ids: Vec<_> = ids
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| (s, s))
        .collect();
    if ids.is_empty() {
        return Err("models required".into());
    }
    let registry = d.join("providers.json");
    let before = std::fs::read(&registry)?;
    let mut r: Value = serde_json::from_slice(&before)?;
    let pid = format!("local-{prefix}");
    let key_path = d.join("keys").join(format!("{pid}.key"));
    if r["providers"].get(&pid).is_some() || key_path.exists() {
        return Err("provider exists; use loom set-key".into());
    }
    let mut v = json!({"prefix":prefix,"name":name,"baseURL":base,"billing":billing,
        "keyFile":key_path,"enabled":true,"models":models(&ids, false)});
    if !native.is_empty() {
        v["anthropicBaseURL"] = json!(native);
    }
    r["providers"][&pid] = v;
    config::catalog(&r)?;
    let key = read_key(io, "推理 API Key（隐藏输入；回车取消）")?;
    if key.is_empty() {
        return Ok(());
    }
    if std::fs::read(&registry)? != before {
        return Err("configuration changed; retry".into());
    }
    config::backup(&registry, d)?;
    config::write(&key_path, key.as_bytes())?;
    if let Err(e) = config::save(&registry, &r) {
        let _ = std::fs::remove_file(&key_path);
        return Err(e);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    struct Script {
        lines: VecDeque<&'static str>,
        keys: VecDeque<&'static str>,
        output: String,
    }
    impl Script {
        fn new(lines: &[&'static str], keys: &[&'static str]) -> Self {
            Self {
                lines: lines.iter().copied().collect(),
                keys: keys.iter().copied().collect(),
                output: String::new(),
            }
        }
    }
    impl Input for Script {
        fn line(&mut self, _: &str) -> Result<String> {
            Ok(self
                .lines
                .pop_front()
                .expect("unexpected non-Key prompt")
                .into())
        }
        fn secret(&mut self, _: &str) -> Result<String> {
            Ok(self.keys.pop_front().expect("unexpected Key prompt").into())
        }
        fn say(&mut self, s: &str) {
            self.output.push_str(s);
        }
    }
    #[test]
    fn selects_all_by_default_and_supports_names_numbers_and_deduplication() {
        assert_eq!(
            selection("").unwrap(),
            (0..PRESETS.len()).collect::<Vec<_>>()
        );
        assert_eq!(selection("1,4,1").unwrap(), vec![0, 3]);
        assert_eq!(selection("kimi，DeepSeek").unwrap(), vec![4, 3]);
        assert!(selection("0").unwrap().is_empty());
        assert_eq!(selection("LongCat").unwrap(), vec![5]);
        for bad in ["7", "1,unknown", "0,1", ","] {
            assert!(selection(bad).is_err());
        }
    }
    #[test]
    fn default_flow_only_asks_for_selection_and_keys_skips_empty_and_syncs() {
        let t = tempfile::tempdir().unwrap();
        let d = t.path().join("data");
        let h = t.path().join("home");
        config::save(
            &h.join(".claude/settings.json"),
            &json!({"permissions":{"defaultMode":"manual"}}),
        )
        .unwrap();
        let mut io = Script::new(
            &[""],
            &["ZENMUX_FIXTURE", "", "", "DEEPSEEK_FIXTURE", "", ""],
        );
        wizard(&d, &h, None, false, &mut io).unwrap();
        assert!(io.lines.is_empty() && io.keys.is_empty());
        let r = config::read(&d.join("providers.json")).unwrap();
        assert_eq!(r["providers"].as_object().unwrap().len(), 2);
        assert_eq!(r["providers"]["local-zenmux"]["baseURL"], PRESETS[0].base);
        assert_eq!(
            r["providers"]["local-zenmux"]["models"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
        assert_eq!(
            config::catalog(&r)
                .unwrap()
                .iter()
                .filter(|m| m.id.starts_with("zenmux/"))
                .count(),
            7
        );
        assert!(!io.output.contains("ZENMUX_FIXTURE"));
        for file in [
            ".pi/agent/models.json",
            ".config/opencode/opencode.json",
            ".claude/settings.json",
        ] {
            let bytes = std::fs::read_to_string(h.join(file)).unwrap();
            assert!(!bytes.contains("ZENMUX_FIXTURE") && !bytes.contains("DEEPSEEK_FIXTURE"));
        }
        assert_eq!(
            config::read(&h.join(".claude/settings.json")).unwrap()["permissions"]["defaultMode"],
            "manual"
        );
    }
    #[test]
    fn all_presets_validate_with_separate_billing_and_native_routes() {
        let t = tempfile::tempdir().unwrap();
        let mut io = Script::new(
            &[],
            &[
                "FIXTURE_A",
                "FIXTURE_B",
                "FIXTURE_C",
                "FIXTURE_D",
                "FIXTURE_E",
                "FIXTURE_F",
            ],
        );
        wizard(
            t.path(),
            &t.path().join("home"),
            Some((0..PRESETS.len()).collect()),
            false,
            &mut io,
        )
        .unwrap();
        let r = config::read(&t.path().join("providers.json")).unwrap();
        let catalog = config::catalog(&r).unwrap();
        assert_eq!(catalog.len(), 16);
        for m in catalog {
            assert!(config::anthropic(&m.p).unwrap().is_some());
        }
        assert_eq!(
            r["providers"]["local-glm"]["baseURL"],
            "https://open.bigmodel.cn/api/coding/paas/v4"
        );
        assert_eq!(
            r["providers"]["local-longcat"]["models"][0]["id"],
            "LongCat-2.5"
        );
        assert_eq!(
            config::anthropic(&r["providers"]["local-longcat"])
                .unwrap()
                .unwrap(),
            "https://api.longcat.chat/anthropic/v1/messages"
        );
        assert_eq!(
            r["providers"]["local-longcat"]["piCompat"]["supportsReasoningEffort"],
            false
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            for p in PRESETS {
                let mode = std::fs::metadata(t.path().join(format!("keys/local-{}.key", p.id)))
                    .unwrap()
                    .permissions()
                    .mode();
                assert_eq!(mode & 0o777, 0o600);
            }
        }
    }
    #[test]
    fn keep_and_rotate_legacy_provider_without_replacing_any_configuration() {
        let t = tempfile::tempdir().unwrap();
        let d = t.path();
        let h = d.join("home");
        config::init(d).unwrap();
        let kp = d.join("keys/original.key");
        config::write(&kp, b"ORIGINAL_FIXTURE").unwrap();
        let r = json!({"proxy":"https://proxy.invalid", "extra":"keep", "providers":{"local-glm-plan":{
            "enabled":true,"baseURL":PRESETS[1].base,"keyFile":kp,"name":"My Name","models":[{"id":"custom-model"}],"extra":"keep"}}});
        config::save(&d.join("providers.json"), &r).unwrap();
        let original = std::fs::read(d.join("providers.json")).unwrap();
        let token = std::fs::read(d.join("hub.key")).unwrap();
        let mut io = Script::new(&[], &[""]);
        wizard(d, &h, Some(vec![1]), false, &mut io).unwrap();
        assert_eq!(std::fs::read(&kp).unwrap(), b"ORIGINAL_FIXTURE");
        let mut io = Script::new(&[], &["NEW_FIXTURE"]);
        wizard(d, &h, Some(vec![1]), false, &mut io).unwrap();
        assert_eq!(std::fs::read(&kp).unwrap(), b"NEW_FIXTURE");
        assert_eq!(std::fs::read(d.join("providers.json")).unwrap(), original);
        assert_eq!(std::fs::read(d.join("hub.key")).unwrap(), token);
        assert!(
            std::fs::read_dir(d.join("backups"))
                .unwrap()
                .any(|p| std::fs::read(p.unwrap().path()).unwrap() == b"ORIGINAL_FIXTURE")
        );
    }
    #[test]
    fn no_key_means_no_provider_or_client_configuration() {
        let t = tempfile::tempdir().unwrap();
        let h = t.path().join("home");
        let mut io = Script::new(&[""], &["", "", "", "", "", ""]);
        wizard(t.path(), &h, None, false, &mut io).unwrap();
        assert!(needs_setup(t.path()).unwrap());
        assert!(!h.exists());
        assert!(!t.path().join("keys").exists());
    }
    #[test]
    fn invalid_input_retries_without_saving_or_printing_it() {
        let t = tempfile::tempdir().unwrap();
        let mut io = Script::new(&["7", "4"], &["BAD FIXTURE", "VALID_FIXTURE"]);
        wizard(t.path(), &t.path().join("home"), None, false, &mut io).unwrap();
        assert_eq!(
            std::fs::read(t.path().join("keys/local-deepseek.key")).unwrap(),
            b"VALID_FIXTURE"
        );
        assert!(!io.output.contains("BAD FIXTURE") && !io.output.contains("VALID_FIXTURE"));
    }
    #[test]
    fn custom_provider_keeps_manual_routes_with_default_name_and_billing() {
        let t = tempfile::tempdir().unwrap();
        let mut io = Script::new(
            &[
                "demo",
                "",
                "https://example.invalid/v1",
                "",
                "",
                "demo-model",
            ],
            &["CUSTOM_FIXTURE"],
        );
        wizard(t.path(), &t.path().join("home"), None, true, &mut io).unwrap();
        let r = config::read(&t.path().join("providers.json")).unwrap();
        assert_eq!(r["providers"]["local-demo"]["name"], "demo");
        assert_eq!(r["providers"]["local-demo"]["billing"], "独立 API 钱包");
    }
    #[test]
    fn ambiguous_existing_routes_are_rejected_before_requesting_a_key() {
        let t = tempfile::tempdir().unwrap();
        config::init(t.path()).unwrap();
        let r = json!({"providers":{
            "first":{"prefix":"zenmux","enabled":false,"keyFile":"first-fixture"},
            "second":{"prefix":"zenmux","enabled":false,"keyFile":"second-fixture"}
        }});
        config::save(&t.path().join("providers.json"), &r).unwrap();
        let mut io = Script::new(&[], &[]);
        assert!(
            wizard(
                t.path(),
                &t.path().join("home"),
                Some(vec![0]),
                false,
                &mut io
            )
            .is_err()
        );
        assert_eq!(config::read(&t.path().join("providers.json")).unwrap(), r);
        assert!(!t.path().join("keys").exists());
    }
}
