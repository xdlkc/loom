//! Shared provider configuration, client inspection, and safe connectivity diagnostics.
use crate::{
    bundled,
    config::{self, Result},
    gateway, network, validate_key,
};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

pub struct PresetInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub base: &'static str,
    pub billing: &'static str,
    pub key_hint: &'static str,
    pub docs_url: &'static str,
    pub key_url: &'static str,
    pub region_hint: &'static str,
    pub pitfall_hint: &'static str,
    pub models: &'static [(&'static str, &'static str)],
}

// Public API documentation and model IDs; never replace an existing provider with these.
// See README's preset sources. Limits are conservative defaults, not account entitlements.
pub const PRESETS: &[PresetInfo] = &[
    PresetInfo {
        id: "zenmux",
        name: "ZenMux",
        base: "https://zenmux.ai/api/v1",
        billing: "ZenMux 推理 Key 对应套餐",
        key_hint: "推理 API Key（不是 Management Key）",
        docs_url: "https://zenmux.ai/docs/guide/quickstart.html",
        key_url: "https://zenmux.ai/",
        region_hint: "聚合网关 · 按网络环境可选代理",
        pitfall_hint: "请填写推理 API Key，不要填写 Management Key。",
        models: &[
            ("anthropic/claude-sonnet-5.5", "Claude Sonnet 5.5"),
            ("anthropic/claude-opus-5.5", "Claude Opus 5.5"),
            ("anthropic/claude-haiku-5.5", "Claude Haiku 5.5"),
        ],
    },
    PresetInfo {
        id: "glm",
        name: "GLM · 国内 Coding Plan",
        base: "https://open.bigmodel.cn/api/coding/paas/v4",
        billing: "国内 Coding Plan",
        key_hint: "Coding Plan 套餐 API Key",
        docs_url: "https://docs.bigmodel.cn/cn/coding-plan/quick-start",
        key_url: "https://open.bigmodel.cn/",
        region_hint: "国内直连",
        pitfall_hint: "请确认开通 Coding Plan 套餐并使用对应 API Key。",
        models: &[
            ("glm-5.3", "GLM 5.3"),
            ("glm-5.3-flash", "GLM 5.3 Flash"),
            ("glm-5.3-flashx", "GLM 5.3 FlashX"),
            ("glm-5.2", "GLM 5.2"),
            ("glm-5.1", "GLM 5.1"),
            ("glm-5-turbo", "GLM 5 Turbo"),
            ("glm-5", "GLM 5"),
            ("glm-4.7", "GLM 4.7"),
            ("glm-4.6", "GLM 4.6"),
            ("glm-4.5", "GLM 4.5"),
            ("glm-4.5-air", "GLM 4.5 Air"),
        ],
    },
    PresetInfo {
        id: "minimax",
        name: "MiniMax · 国内订阅",
        base: "https://api.minimax.cn/v1",
        billing: "国内 Coding / Token / M Plan",
        key_hint: "订阅 Key（不是普通 API Key）",
        docs_url: "https://platform.minimax.cn/docs/m-plan/quickstart",
        key_url: "https://platform.minimax.cn/",
        region_hint: "国内直连",
        pitfall_hint: "请使用 Coding / Token / M Plan 订阅专属 Key，而非普通按量 API Key。",
        models: &[("MiniMax-M3.1-Flash", "MiniMax M3.1 Flash")],
    },
    PresetInfo {
        id: "deepseek",
        name: "DeepSeek · API 钱包",
        base: "https://api.deepseek.com",
        billing: "API 钱包",
        key_hint: "开放平台 API Key",
        docs_url: "https://api-docs.deepseek.com/quick_start/pricing-details-cny/",
        key_url: "https://platform.deepseek.com/api_keys",
        region_hint: "国内直连",
        pitfall_hint: "开放平台钱包余额需大于 0，支持原生思考档位自动适配。",
        models: &[
            ("deepseek-flash", "DeepSeek Flash"),
            ("deepseek-v4-pro", "DeepSeek V4 Pro"),
        ],
    },
    PresetInfo {
        id: "kimi",
        name: "Kimi Code · 国内订阅",
        base: "https://api.kimi.com/coding/v1",
        billing: "Kimi Code 订阅",
        key_hint: "Kimi Code API Key（不是 Moonshot 开放平台 Key）",
        docs_url: "https://www.kimi.com/code/docs/",
        key_url: "https://www.kimi.com/code",
        region_hint: "国内直连",
        pitfall_hint: "请使用 Kimi Code 会员订阅 Key，不要填 Moonshot 开放平台 Key。",
        models: &[
            ("k3.1-pro", "K3.1 Pro"),
            ("k3.1-pro-highspeed", "K3.1 Pro Highspeed"),
            ("k3", "K3"),
            ("k3-256k", "K3 256K"),
            ("kimi-for-coding", "K2.8 Preview"),
            ("kimi-for-coding-highspeed", "K2.7 Code Highspeed"),
        ],
    },
    PresetInfo {
        id: "longcat",
        name: "LongCat · 美团",
        base: "https://api.longcat.chat/openai/v1",
        billing: "LongCat API 钱包 / Token 资源包",
        key_hint: "LongCat 开放平台 API Key（不是网页登录令牌）",
        docs_url: "https://longcat.chat/platform/docs/zh/",
        key_url: "https://longcat.chat/platform/",
        region_hint: "国内直连",
        pitfall_hint: "请填写美团 LongCat 开放平台 API Key，不是网页登录 Cookie/令牌。",
        models: &[("LongCat-2.5", "LongCat 2.5")],
    },
];

pub fn models_json(ids: &[(&str, &str)], reasoning: bool) -> Value {
    json!(
        ids.iter()
            .map(|(id, name)| json!({
                "id": id,
                "name": name,
                "contextWindow": 128000,
                "maxTokens": 8192,
                "input": ["text"],
                "reasoning": reasoning
            }))
            .collect::<Vec<_>>()
    )
}

pub fn preset_provider(p: &PresetInfo, key: &Path) -> Value {
    let mut value = json!({
        "prefix": p.id,
        "name": p.name,
        "baseURL": p.base,
        "billing": p.billing,
        "enabled": true,
        "keyFile": key,
        "models": models_json(p.models, true)
    });
    if p.id == "longcat" {
        // LongCat uses the same enabled/disabled thinking object, not DeepSeek's effort levels.
        value["piCompat"] = json!({"thinkingFormat": "deepseek", "supportsReasoningEffort": false});
    }
    value
}

fn validate_prefix(prefix: &str) -> Result<&str> {
    let p = prefix.trim();
    if p.is_empty()
        || !p.as_bytes()[0].is_ascii_lowercase()
        || !p
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err("厂商标识（prefix）须以小写字母开头，仅含小写字母、数字或短横线。".into());
    }
    Ok(p)
}

fn validate_model_id(id: &str) -> Result<&str> {
    let id = id.trim();
    if id.is_empty() || id.chars().any(|c| c.is_whitespace() || "?#".contains(c)) {
        return Err("模型 ID 不能为空，且不可包含空格、? 或 #。".into());
    }
    Ok(id)
}

fn has_valid_key(provider: &Value) -> bool {
    provider["keyFile"]
        .as_str()
        .is_some_and(|p| fs::read_to_string(p).is_ok_and(|s| !s.trim().is_empty()))
}

fn normalize_model_entry(v: &Value) -> Result<Value> {
    let id = validate_model_id(v["id"].as_str().unwrap_or_default())?;
    let name = v["name"]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(id);
    let context_window = v["contextWindow"].as_u64().unwrap_or(128000).max(1024);
    let max_tokens = v["maxTokens"].as_u64().unwrap_or(8192).max(1);
    let input = match v["input"].as_array() {
        Some(arr) if !arr.is_empty() => Value::Array(arr.clone()),
        _ => json!(["text"]),
    };
    let mut item = json!({
        "id": id,
        "name": name,
        "contextWindow": context_window,
        "maxTokens": max_tokens,
        "input": input,
        "reasoning": v["reasoning"] == true,
    });
    if v["enabled"] == false {
        item["enabled"] = json!(false);
    }
    if let Some(compat) = v.get("compat").filter(|x| x.is_object()) {
        item["compat"] = compat.clone();
    }
    if let Some(levels) = v.get("thinkingLevelMap").filter(|x| x.is_object()) {
        item["thinkingLevelMap"] = levels.clone();
    }
    Ok(item)
}

fn provider_models_view(provider: &Value) -> Result<Vec<Value>> {
    let local = provider["models"]
        .as_array()
        .ok_or("models required")?
        .clone();
    let include_bundled = provider["includeBundledModels"] != false;
    let base = provider["baseURL"].as_str().unwrap_or_default();
    let mut out = Vec::new();
    for m in &local {
        let mut item = json!({
            "id": m["id"],
            "name": m["name"].as_str().unwrap_or(m["id"].as_str().unwrap_or("")),
            "contextWindow": m["contextWindow"].as_u64().unwrap_or(128000),
            "maxTokens": m["maxTokens"].as_u64().unwrap_or(8192),
            "input": m.get("input").cloned().unwrap_or_else(|| json!(["text"])),
            "reasoning": m["reasoning"] == true,
            "enabled": m["enabled"] != false,
            "source": "local"
        });
        if m.get("compat").is_some_and(|v| v.is_object()) {
            item["compat"] = m["compat"].clone();
        }
        out.push(item);
    }
    if include_bundled {
        for m in bundled::models_for_base(base)? {
            if !local.iter().any(|l| l["id"] == m["id"]) {
                out.push(json!({
                    "id": m["id"],
                    "name": m["name"].as_str().unwrap_or(m["id"].as_str().unwrap_or("")),
                    "contextWindow": m["contextWindow"].as_u64().unwrap_or(128000),
                    "maxTokens": m["maxTokens"].as_u64().unwrap_or(8192),
                    "input": m.get("input").cloned().unwrap_or_else(|| json!(["text"])),
                    "reasoning": m["reasoning"] == true,
                    "enabled": true,
                    "source": "bundled"
                }));
            }
        }
    }
    Ok(out)
}

fn safe_sync_warning(err: &(dyn std::error::Error + Send + Sync)) -> String {
    let s = err.to_string();
    if s.contains("opencode.jsonc contains comments") {
        "OpenCode 配置文件 opencode.jsonc 含有注释，Loom 已保护性跳过覆盖；如需自动同步，请先移除注释或迁移至 opencode.json。".into()
    } else if s.contains("both exist") {
        "检测到 opencode.json 与 opencode.jsonc 同时存在，Loom 已拒绝覆盖；请保留其中一份后重新同步。".into()
    } else if s.contains("add a provider first") {
        "尚未启用任何模型，已暂缓同步客户端。".into()
    } else {
        "配置已保存，但客户端同步未完成；请检查客户端配置格式后重试同步。".into()
    }
}

fn try_sync(
    data: &Path,
    home: &Path,
    gateway_port: u16,
    registry: &Value,
) -> (bool, Option<String>) {
    let count = config::catalog(registry).map(|m| m.len()).unwrap_or(0);
    if count == 0 {
        return (false, None);
    }
    match config::sync_for_port(data, home, gateway_port) {
        Ok(()) => (true, None),
        Err(e) => (false, Some(safe_sync_warning(&*e))),
    }
}

pub async fn probe_gateway(port: u16) -> bool {
    let Ok(client) = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_millis(800))
        .build()
    else {
        return false;
    };
    let Ok(resp) = client
        .get(format!("http://127.0.0.1:{port}/health"))
        .send()
        .await
    else {
        return false;
    };
    if !resp.status().is_success() {
        return false;
    }
    resp.json::<Value>()
        .await
        .is_ok_and(|v| v["ok"] == true && v["runtime"] == "rust")
}

pub fn inspect_clients(home: &Path, gateway_port: u16) -> Value {
    let expected_openai = format!("http://127.0.0.1:{gateway_port}/v1");
    let expected_anthropic = format!("http://127.0.0.1:{gateway_port}");

    let pi_models_path = home.join(".pi/agent/models.json");
    let pi_settings_path = home.join(".pi/agent/settings.json");
    let pi_models = config::read_or(&pi_models_path).unwrap_or_else(|_| json!({}));
    let pi_settings = config::read_or(&pi_settings_path).unwrap_or_else(|_| json!({}));
    let pi_synced = pi_models["providers"]["hub"]["baseUrl"]
        .as_str()
        .is_some_and(|u| u == expected_openai);
    let pi_count = pi_models["providers"]["hub"]["models"]
        .as_array()
        .map_or(0, Vec::len);

    let (oc_installed, oc_synced, oc_file_label, oc_model, oc_count, oc_conflict) =
        match config::opencode_file(home) {
            Ok(op) => {
                let exists = op.exists();
                let oc = config::read_or(&op).unwrap_or_else(|_| json!({}));
                let synced = oc["providers"]["hub"]["settings"]["baseURL"]
                    .as_str()
                    .is_some_and(|u| u == expected_openai);
                let count = oc["providers"]["hub"]["models"]
                    .as_object()
                    .map_or(0, |m| m.len());
                let label = op
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("opencode.json")
                    .to_string();
                (
                    exists || home.join(".config/opencode").exists(),
                    synced,
                    label,
                    oc["model"].as_str().map(String::from),
                    count,
                    Option::<String>::None,
                )
            }
            Err(e) => (
                true,
                false,
                "opencode.jsonc".to_string(),
                None,
                0,
                Some(safe_sync_warning(&*e)),
            ),
        };

    let cc_path = home.join(".claude/settings.json");
    let cc_exists = cc_path.is_file();
    let cc = if cc_exists {
        config::read_or(&cc_path).unwrap_or_else(|_| json!({}))
    } else {
        json!({})
    };
    let cc_synced = cc_exists
        && cc["env"]["ANTHROPIC_BASE_URL"]
            .as_str()
            .is_some_and(|u| u == expected_anthropic);
    let cc_count = cc["modelPicker"]["options"].as_array().map_or(0, Vec::len);

    json!({
        "pi": {
            "name": "pi",
            "installed": home.join(".pi").exists(),
            "synced": pi_synced,
            "config_path": "~/.pi/agent/models.json",
            "default_provider": pi_settings["defaultProvider"].as_str(),
            "default_model": pi_settings["defaultModel"].as_str(),
            "model_count": pi_count,
            "note": "同步模型列表与会话追踪扩展，不修改原生 OAuth 与默认偏好。"
        },
        "opencode2": {
            "name": "OpenCode V2",
            "installed": oc_installed,
            "synced": oc_synced,
            "config_path": format!("~/.config/opencode/{oc_file_label}"),
            "default_model": oc_model,
            "model_count": oc_count,
            "conflict": oc_conflict,
            "note": "同步 providers.hub 与观测插件；自动保留原有主题、插件与模型设置。"
        },
        "claude_code": {
            "name": "Claude Code",
            "installed": cc_exists,
            "synced": cc_synced,
            "config_path": "~/.claude/settings.json",
            "default_model": cc["model"].as_str(),
            "model_count": cc_count,
            "note": if cc_exists {
                "检测到已存在 Claude Code 配置，将保留原有主题与权限模式增量写入。"
            } else {
                "未检测到 ~/.claude/settings.json；首次运行过 Claude Code 后会自动参与同步。"
            }
        }
    })
}

fn desktop_app_installed(home: &Path, app_dir: Option<&Path>) -> bool {
    let base = app_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join("Applications"));
    base.join("Loom.app/Contents/MacOS/Loom").is_file()
}

pub async fn setup_snapshot(
    data: &Path,
    home: &Path,
    gateway_port: u16,
    app_dir: Option<&Path>,
) -> Result<Value> {
    config::init(data)?;
    let data = fs::canonicalize(data)?;
    let registry = config::read(&data.join("providers.json"))?;
    let providers_obj = registry["providers"]
        .as_object()
        .ok_or("providers required")?;

    let mut providers = Vec::new();
    let mut ready_model_count = 0usize;
    let mut enabled_model_count = 0usize;

    for (pid, p) in providers_obj {
        let prefix = config::provider_prefix(pid, p).to_string();
        let enabled = p["enabled"] == true;
        let has_key = has_valid_key(p);
        let models = provider_models_view(p)?;
        let active_count = models.iter().filter(|m| m["enabled"] == true).count();
        if enabled {
            enabled_model_count += active_count;
            if has_key {
                ready_model_count += active_count;
            }
        }
        let preset_meta = PRESETS.iter().find(|pr| pr.id == prefix);
        providers.push(json!({
            "id": pid,
            "prefix": prefix,
            "preset_id": preset_meta.map(|pr| pr.id),
            "name": p["name"].as_str().unwrap_or(pid),
            "base_url": p["baseURL"].as_str().unwrap_or(""),
            "anthropic_base_url": p["anthropicBaseURL"].as_str(),
            "anthropic_supported": config::anthropic(p).ok().flatten().is_some(),
            "billing": p["billing"].as_str().unwrap_or("自定义通道"),
            "enabled": enabled,
            "has_key": has_key,
            "include_bundled_models": p["includeBundledModels"] != false,
            "supports_bundled_catalog": p["baseURL"].as_str().is_some_and(|b| !bundled::models_for_base(b).unwrap_or_default().is_empty()),
            "active_model_count": active_count,
            "models": models,
        }));
    }

    let presets: Vec<Value> = PRESETS
        .iter()
        .map(|pr| {
            let matches: Vec<&Value> = providers
                .iter()
                .filter(|v| v["prefix"] == pr.id)
                .collect();
            let first = matches.first().copied();
            let bundled_extras = bundled::models_for_base(pr.base).unwrap_or_default();
            let mut preview_models: Vec<Value> = pr
                .models
                .iter()
                .map(|(id, name)| json!({"id": id, "name": name}))
                .collect();
            for extra in bundled_extras {
                if !preview_models.iter().any(|m| m["id"] == extra["id"]) {
                    preview_models.push(json!({
                        "id": extra["id"],
                        "name": extra["name"].as_str().unwrap_or(extra["id"].as_str().unwrap_or(""))
                    }));
                }
            }
            json!({
                "id": pr.id,
                "name": pr.name,
                "base_url": pr.base,
                "billing": pr.billing,
                "key_hint": pr.key_hint,
                "docs_url": pr.docs_url,
                "key_url": pr.key_url,
                "region_hint": pr.region_hint,
                "pitfall_hint": pr.pitfall_hint,
                "configured": !matches.is_empty(),
                "ambiguous": matches.len() > 1,
                "provider_id": first.and_then(|v| v["id"].as_str()),
                "enabled": first.is_some_and(|v| v["enabled"] == true),
                "has_key": first.is_some_and(|v| v["has_key"] == true),
                "active_model_count": first.and_then(|v| v["active_model_count"].as_u64()).unwrap_or(0),
                "default_models": preview_models,
            })
        })
        .collect();

    let gateway_healthy = probe_gateway(gateway_port).await;
    let clients = inspect_clients(home, gateway_port);
    let data_dir_hint = if cfg!(target_os = "macos") {
        "~/Library/Application Support/ModelHub/.local"
    } else {
        "~/.local/share/ModelHub/.local"
    };

    Ok(json!({
        "needs_setup": ready_model_count == 0,
        "ready_model_count": ready_model_count,
        "enabled_model_count": enabled_model_count,
        "gateway": {
            "port": gateway_port,
            "healthy": gateway_healthy,
            "openai_base_url": format!("http://127.0.0.1:{gateway_port}/v1"),
            "anthropic_base_url": format!("http://127.0.0.1:{gateway_port}")
        },
        "proxy": {
            "configured": registry["proxy"].as_str().is_some_and(|v| !v.is_empty())
        },
        "desktop_app": {
            "supported": cfg!(target_os = "macos"),
            "installed": desktop_app_installed(home, app_dir)
        },
        "hub_key_hint": format!("{data_dir_hint}/hub.key"),
        "presets": presets,
        "providers": providers,
        "clients": clients
    }))
}

pub fn upsert_preset(
    data: &Path,
    home: &Path,
    gateway_port: u16,
    preset_id: &str,
    api_key: Option<&str>,
    enabled: Option<bool>,
) -> Result<Value> {
    config::init(data)?;
    let data = fs::canonicalize(data)?;
    let preset = PRESETS
        .iter()
        .find(|p| p.id.eq_ignore_ascii_case(preset_id.trim()))
        .ok_or("未找到对应的预设模型厂商。")?;

    let registry_path = data.join("providers.json");
    let mut r = config::read(&registry_path)?;
    let matches: Vec<String> = r["providers"]
        .as_object()
        .ok_or("providers required")?
        .iter()
        .filter(|(pid, v)| config::provider_prefix(pid, v) == preset.id)
        .map(|(pid, _)| pid.clone())
        .collect();
    if matches.len() > 1 {
        return Err("存在多个同前缀供应商，请在「模型厂商」页按具体条目编辑或更换 Key。".into());
    }

    let trimmed_key = api_key.map(str::trim).filter(|k| !k.is_empty());
    if let Some(key) = trimmed_key {
        validate_key(key)
            .map_err(|_| "API Key 格式无效：仅允许可见 ASCII 字符，且不能包含引号或反斜杠。")?;
    }

    let existing = matches.first().cloned();
    let pid = existing
        .clone()
        .unwrap_or_else(|| format!("local-{}", preset.id));

    if let Some(ref existing_pid) = existing {
        let key_path = PathBuf::from(
            r["providers"][existing_pid]["keyFile"]
                .as_str()
                .ok_or("keyFile required")?,
        );
        if let Some(key) = trimmed_key {
            config::backup(&key_path, &data)?;
            config::write(&key_path, key.as_bytes())?;
        }
        if let Some(en) = enabled {
            r["providers"][existing_pid]["enabled"] = json!(en);
            config::catalog(&r)?;
            config::backup(&registry_path, &data)?;
            config::save(&registry_path, &r)?;
        }
    } else {
        let key = trimmed_key.ok_or("首次添加预设供应商需要输入 API Key。")?;
        if r["providers"].get(&pid).is_some() {
            return Err("目标供应商 ID 已被占用。".into());
        }
        let key_path = data.join("keys").join(format!("{pid}.key"));
        if key_path.exists() {
            return Err("密钥文件已存在，为防误覆盖已中止。".into());
        }
        let mut entry = preset_provider(preset, &key_path);
        if let Some(en) = enabled {
            entry["enabled"] = json!(en);
        }
        r["providers"][&pid] = entry;
        config::catalog(&r)?;
        config::backup(&registry_path, &data)?;
        config::write(&key_path, key.as_bytes())?;
        if let Err(e) = config::save(&registry_path, &r) {
            let _ = fs::remove_file(&key_path);
            return Err(e);
        }
    }

    let (synced, sync_warning) = try_sync(&data, home, gateway_port, &r);
    Ok(json!({
        "ok": true,
        "provider_id": pid,
        "synced": synced,
        "sync_warning": sync_warning
    }))
}

fn parse_custom_models(req: &Value) -> Result<Vec<Value>> {
    if let Some(arr) = req["models"].as_array() {
        let mut out = Vec::with_capacity(arr.len());
        for item in arr {
            if let Some(id) = item.as_str() {
                let id = validate_model_id(id)?;
                out.push(json!({
                    "id": id,
                    "name": id,
                    "contextWindow": 128000,
                    "maxTokens": 8192,
                    "input": ["text"],
                    "reasoning": false
                }));
            } else {
                out.push(normalize_model_entry(item)?);
            }
        }
        if out.is_empty() {
            return Err("请至少填写一个模型 ID。".into());
        }
        return Ok(out);
    }
    if let Some(text) = req["models_text"].as_str() {
        let ids: Vec<(&str, &str)> = text
            .split([',', '\n', '，'])
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| (s, s))
            .collect();
        if ids.is_empty() {
            return Err("请至少填写一个模型 ID。".into());
        }
        for (id, _) in &ids {
            validate_model_id(id)?;
        }
        let reasoning = req["reasoning"] == true;
        return Ok(models_json(&ids, reasoning)
            .as_array()
            .cloned()
            .unwrap_or_default());
    }
    Err("请提供至少一个模型配置。".into())
}

pub fn upsert_custom(data: &Path, home: &Path, gateway_port: u16, req: &Value) -> Result<Value> {
    config::init(data)?;
    let data = fs::canonicalize(data)?;
    let registry_path = data.join("providers.json");
    let mut r = config::read(&registry_path)?;

    let prefix = validate_prefix(req["prefix"].as_str().unwrap_or_default())?.to_string();
    let base_url = config::endpoint(req["base_url"].as_str().unwrap_or_default().trim())
        .map_err(|_| "OpenAI 兼容 Base URL 必须是无凭据、无查询参数的 https:// 地址。")?;
    let anthropic_raw = req["anthropic_base_url"].as_str().unwrap_or("").trim();
    let anthropic_url = if anthropic_raw.is_empty() {
        None
    } else {
        Some(
            config::endpoint(anthropic_raw)
                .map_err(|_| "Anthropic Base URL 必须是无凭据、无查询参数的 https:// 地址。")?,
        )
    };
    let name = req["name"]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(&prefix)
        .to_string();
    let billing = req["billing"]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("独立 API 钱包")
        .to_string();
    let models = parse_custom_models(req)?;
    let enabled = req.get("enabled").and_then(Value::as_bool).unwrap_or(true);

    let trimmed_key = req["api_key"]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if let Some(key) = trimmed_key {
        validate_key(key)
            .map_err(|_| "API Key 格式无效：仅允许可见 ASCII 字符，且不能包含引号或反斜杠。")?;
    }

    let edit_pid = req["provider_id"]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from);

    let pid = if let Some(pid) = edit_pid {
        let existing = r["providers"]
            .get(&pid)
            .cloned()
            .ok_or("未找到要修改的供应商配置。")?;
        let key_path = PathBuf::from(existing["keyFile"].as_str().ok_or("keyFile required")?);
        let mut updated = existing;
        updated["prefix"] = json!(prefix);
        updated["name"] = json!(name);
        updated["baseURL"] = json!(base_url);
        updated["billing"] = json!(billing);
        updated["enabled"] = json!(enabled);
        updated["models"] = Value::Array(models);
        if let Some(a) = anthropic_url {
            updated["anthropicBaseURL"] = json!(a);
        } else if let Some(obj) = updated.as_object_mut() {
            obj.remove("anthropicBaseURL");
        }
        r["providers"][&pid] = updated;
        config::catalog(&r)?;
        if let Some(key) = trimmed_key {
            config::backup(&key_path, &data)?;
            config::write(&key_path, key.as_bytes())?;
        }
        config::backup(&registry_path, &data)?;
        config::save(&registry_path, &r)?;
        pid
    } else {
        let key = trimmed_key.ok_or("新增自定义供应商必须提供 API Key。")?;
        let pid = format!("local-{prefix}");
        let key_path = data.join("keys").join(format!("{pid}.key"));
        if r["providers"].get(&pid).is_some() || key_path.exists() {
            return Err("该厂商标识已存在，请直接在列表中编辑或更换 Key。".into());
        }
        let mut entry = json!({
            "prefix": prefix,
            "name": name,
            "baseURL": base_url,
            "billing": billing,
            "enabled": enabled,
            "keyFile": key_path,
            "models": models
        });
        if let Some(a) = anthropic_url {
            entry["anthropicBaseURL"] = json!(a);
        }
        r["providers"][&pid] = entry;
        config::catalog(&r)?;
        config::backup(&registry_path, &data)?;
        config::write(&key_path, key.as_bytes())?;
        if let Err(e) = config::save(&registry_path, &r) {
            let _ = fs::remove_file(&key_path);
            return Err(e);
        }
        pid
    };

    let (synced, sync_warning) = try_sync(&data, home, gateway_port, &r);
    Ok(json!({
        "ok": true,
        "provider_id": pid,
        "synced": synced,
        "sync_warning": sync_warning
    }))
}

pub fn update_provider(data: &Path, home: &Path, gateway_port: u16, req: &Value) -> Result<Value> {
    config::init(data)?;
    let data = fs::canonicalize(data)?;
    let registry_path = data.join("providers.json");
    let mut r = config::read(&registry_path)?;
    let pid = req["provider_id"]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or("provider_id required")?
        .to_string();
    let mut provider = r["providers"]
        .get(&pid)
        .cloned()
        .ok_or("未找到该供应商配置。")?;

    let mut registry_changed = false;
    if let Some(en) = req.get("enabled").and_then(Value::as_bool) {
        provider["enabled"] = json!(en);
        registry_changed = true;
    }
    if let Some(inc) = req.get("include_bundled_models").and_then(Value::as_bool) {
        provider["includeBundledModels"] = json!(inc);
        registry_changed = true;
    }
    if let Some(arr) = req.get("models").and_then(Value::as_array) {
        let mut normalized = Vec::with_capacity(arr.len());
        for m in arr {
            normalized.push(normalize_model_entry(m)?);
        }
        provider["models"] = Value::Array(normalized);
        registry_changed = true;
    }
    if let Some(toggle) = req.get("toggle_model").filter(|v| v.is_object()) {
        let target_id = validate_model_id(toggle["id"].as_str().unwrap_or_default())?;
        let next_enabled = toggle["enabled"].as_bool().ok_or("enabled bool required")?;
        let mut list = provider["models"].as_array().cloned().unwrap_or_default();
        if let Some(existing) = list.iter_mut().find(|m| m["id"] == target_id) {
            if next_enabled {
                if let Some(obj) = existing.as_object_mut() {
                    obj.remove("enabled");
                }
            } else {
                existing["enabled"] = json!(false);
            }
        } else {
            let bundled_item =
                bundled::models_for_base(provider["baseURL"].as_str().unwrap_or(""))?
                    .into_iter()
                    .find(|m| m["id"] == target_id);
            let mut entry =
                bundled_item.unwrap_or_else(|| json!({"id": target_id, "name": target_id}));
            if !next_enabled {
                entry["enabled"] = json!(false);
            }
            list.push(entry);
        }
        provider["models"] = Value::Array(list);
        registry_changed = true;
    }
    if let Some(add_m) = req.get("add_model").filter(|v| v.is_object()) {
        let item = normalize_model_entry(add_m)?;
        let mut list = provider["models"].as_array().cloned().unwrap_or_default();
        if let Some(pos) = list.iter().position(|m| m["id"] == item["id"]) {
            list[pos] = item;
        } else {
            list.push(item);
        }
        provider["models"] = Value::Array(list);
        registry_changed = true;
    }
    if let Some(remove_id) = req.get("remove_model_id").and_then(Value::as_str) {
        let mut list = provider["models"].as_array().cloned().unwrap_or_default();
        let before = list.len();
        list.retain(|m| m["id"] != remove_id);
        if list.len() == before {
            return Err("未找到要移除的本地模型条目。".into());
        }
        provider["models"] = Value::Array(list);
        registry_changed = true;
    }

    if let Some(key) = req["api_key"]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        validate_key(key)
            .map_err(|_| "API Key 格式无效：仅允许可见 ASCII 字符，且不能包含引号或反斜杠。")?;
        let key_path = PathBuf::from(provider["keyFile"].as_str().ok_or("keyFile required")?);
        config::backup(&key_path, &data)?;
        config::write(&key_path, key.as_bytes())?;
    }

    if registry_changed {
        r["providers"][&pid] = provider;
        config::catalog(&r)?;
        config::backup(&registry_path, &data)?;
        config::save(&registry_path, &r)?;
    }

    let (synced, sync_warning) = try_sync(&data, home, gateway_port, &r);
    Ok(json!({
        "ok": true,
        "provider_id": pid,
        "synced": synced,
        "sync_warning": sync_warning
    }))
}

pub fn delete_provider(
    data: &Path,
    home: &Path,
    gateway_port: u16,
    provider_id: &str,
) -> Result<Value> {
    config::init(data)?;
    let data = fs::canonicalize(data)?;
    let registry_path = data.join("providers.json");
    let mut r = config::read(&registry_path)?;
    let providers_map = r["providers"].as_object_mut().ok_or("providers required")?;
    let removed = providers_map
        .remove(provider_id.trim())
        .ok_or("未找到要删除的供应商配置。")?;

    let key_path = removed["keyFile"].as_str().map(PathBuf::from);
    let still_referenced = key_path.as_ref().is_some_and(|kp| {
        providers_map
            .values()
            .any(|v| v["keyFile"].as_str().map(Path::new) == Some(kp.as_path()))
    });

    config::catalog(&r)?;
    config::backup(&registry_path, &data)?;
    config::save(&registry_path, &r)?;

    if !still_referenced
        && let Some(kp) = key_path
        && let Ok(canon_key) = fs::canonicalize(&kp)
        && let Ok(canon_keys_dir) = fs::canonicalize(data.join("keys"))
        && canon_key.starts_with(&canon_keys_dir)
    {
        config::backup(&canon_key, &data)?;
        let _ = fs::remove_file(&canon_key);
    }

    let (synced, sync_warning) = try_sync(&data, home, gateway_port, &r);
    Ok(json!({
        "ok": true,
        "synced": synced,
        "sync_warning": sync_warning
    }))
}

pub async fn verify_provider(
    data: &Path,
    provider_id: &str,
    model_id: Option<&str>,
    #[cfg(test)] endpoint_override: Option<&str>,
) -> Result<Value> {
    let data = fs::canonicalize(data)?;
    let registry = config::read(&data.join("providers.json"))?;
    let pid = provider_id.trim();
    let provider = registry["providers"]
        .get(pid)
        .ok_or("未找到指定供应商配置。")?;
    let prefix = config::provider_prefix(pid, provider);
    let base_url = config::endpoint(provider["baseURL"].as_str().ok_or("baseURL required")?)?;

    let models = provider_models_view(provider)?;
    let requested = model_id
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.strip_prefix(&format!("{prefix}/")).unwrap_or(s));
    let target_model = if let Some(req_id) = requested {
        models
            .iter()
            .find(|m| m["id"].as_str() == Some(req_id))
            .and_then(|m| m["id"].as_str())
            .unwrap_or(req_id)
            .to_string()
    } else {
        models
            .iter()
            .find(|m| m["enabled"] == true)
            .or_else(|| models.first())
            .and_then(|m| m["id"].as_str())
            .ok_or("该供应商尚未配置任何模型，请先添加至少一个模型。")?
            .to_string()
    };

    let key_file = provider["keyFile"].as_str().unwrap_or("");
    let upstream_key = match fs::read_to_string(key_file) {
        Ok(k) if !k.trim().is_empty() => k.trim().to_string(),
        _ => {
            return Ok(json!({
                "ok": false,
                "provider_id": pid,
                "model": target_model,
                "category": "key_missing",
                "message": "未找到本机密钥文件或密钥为空，请重新粘贴并保存 API Key。"
            }));
        }
    };

    let mut builder = network::client_builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(20));
    let proxy_configured = registry["proxy"].as_str().is_some_and(|v| !v.is_empty());
    if let Some(proxy) = registry["proxy"].as_str().filter(|v| !v.is_empty()) {
        match reqwest::Proxy::all(proxy) {
            Ok(p) => builder = builder.proxy(p),
            Err(_) => {
                return Ok(json!({
                    "ok": false,
                    "provider_id": pid,
                    "model": target_model,
                    "category": "proxy_invalid",
                    "message": "当前配置的出站代理格式无效，请重新检查代理设置或切换为直连。"
                }));
            }
        }
    }
    let client = builder.build()?;

    let target_url = {
        #[cfg(test)]
        {
            endpoint_override
                .map(String::from)
                .unwrap_or_else(|| format!("{base_url}/chat/completions"))
        }
        #[cfg(not(test))]
        {
            format!("{base_url}/chat/completions")
        }
    };

    let started = Instant::now();
    let payload = json!({
        "model": target_model,
        "messages": [{"role": "user", "content": "ping"}],
        "max_tokens": 1,
        "stream": false
    });
    let response = match client
        .post(&target_url)
        .bearer_auth(&upstream_key)
        .json(&payload)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            let (category, hint) = network::failure(&e);
            return Ok(json!({
                "ok": false,
                "provider_id": pid,
                "model": target_model,
                "latency_ms": started.elapsed().as_millis() as u64,
                "proxy_configured": proxy_configured,
                "category": category,
                "message": hint
            }));
        }
    };

    let status = response.status().as_u16();
    let latency_ms = started.elapsed().as_millis() as u64;
    if response.status().is_success() {
        return Ok(json!({
            "ok": true,
            "provider_id": pid,
            "model": target_model,
            "http": status,
            "latency_ms": latency_ms,
            "category": "ok",
            "message": "连接成功！API Key 有效，模型路由可正常响应。"
        }));
    }

    let (category, message) = match status {
        401 | 403 => (
            "auth_failed",
            "鉴权被拒绝（HTTP 401/403）：请检查是否粘贴了正确的 API Key，并留意区分「套餐订阅 Key」与「开放平台钱包 Key」。".to_string(),
        ),
        402 => {
            let bytes = response.bytes().await.unwrap_or_default();
            let body: Value = serde_json::from_slice(&bytes[..bytes.len().min(16384)])
                .unwrap_or(Value::Null);
            let (cat, msg) = gateway::classify_402(&body);
            (cat, msg.to_string())
        }
        404 => (
            "not_found",
            "接口或模型不存在（HTTP 404）：请核对 Base URL 是否正确，以及当前账号是否有权访问该模型。".to_string(),
        ),
        400 => (
            "bad_request",
            "上游已收到鉴权请求，但拒绝了默认测试参数（HTTP 400）：可切换该厂商下其他模型再测，或直接在实际客户端验证。".to_string(),
        ),
        429 => (
            "rate_limited",
            "API Key 鉴权有效，但当前触发上游频率或并发限制（HTTP 429），稍后即可正常使用。".to_string(),
        ),
        500..=599 => (
            "upstream_error",
            "上游厂商服务暂时异常（HTTP 5xx），本机配置无需重填，请稍后再试。".to_string(),
        ),
        _ => (
            "http_error",
            format!("上游返回 HTTP {status}，请核对套餐权限、模型名称或网络代理状态。"),
        ),
    };

    Ok(json!({
        "ok": false,
        "provider_id": pid,
        "model": target_model,
        "http": status,
        "latency_ms": latency_ms,
        "category": category,
        "message": message
    }))
}

#[cfg(unix)]
fn unix_mode(p: &Path) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(p).ok().map(|m| m.permissions().mode() & 0o777)
}

pub async fn run_doctor(
    data: &Path,
    home: &Path,
    gateway_port: u16,
    admin_port: u16,
    ping: bool,
    target_provider: Option<&str>,
) -> Result<()> {
    println!("=== Loom 本机环境与连通性自检 ===\n");
    println!("[1/4] 本机私有目录与密钥安全");
    if !data.exists() {
        println!("  [!] 数据目录尚未初始化。运行 loom init 或 ./setup.sh 即可初始化。");
    } else {
        #[cfg(unix)]
        if let Some(m) = unix_mode(data) {
            println!(
                "  [{}] 数据目录权限: {:03o}{}",
                if m == 0o700 { "✓" } else { "!" },
                m,
                if m == 0o700 {
                    ""
                } else {
                    "（建议保持 700）"
                }
            );
        }
        let hub_key_ok =
            fs::read_to_string(data.join("hub.key")).is_ok_and(|s| !s.trim().is_empty());
        println!(
            "  [{}] 本机入口密钥 hub.key: {}",
            if hub_key_ok { "✓" } else { "✗" },
            if hub_key_ok {
                "已就绪（仅限本机客户端引用）"
            } else {
                "缺失"
            }
        );
    }

    let snap = setup_snapshot(data, home, gateway_port, None).await?;
    println!("\n[2/4] 模型厂商与路由配置");
    let providers = snap["providers"].as_array().cloned().unwrap_or_default();
    if providers.is_empty() {
        println!(
            "  [!] 尚未配置任何模型厂商。运行 loom add 或打开网页工作台粘贴 API Key 即可开始。"
        );
    } else {
        for p in &providers {
            let pid = p["id"].as_str().unwrap_or("unknown");
            let name = p["name"].as_str().unwrap_or(pid);
            let enabled = p["enabled"] == true;
            let has_key = p["has_key"] == true;
            let count = p["active_model_count"].as_u64().unwrap_or(0);
            let status_mark = if enabled && has_key && count > 0 {
                "✓"
            } else {
                "!"
            };
            println!(
                "  [{status_mark}] {name} ({pid}) · {} · {} · {count} 个可用模型{}",
                if enabled { "已启用" } else { "已停用" },
                if has_key {
                    "Key 已保存"
                } else {
                    "缺少 Key 文件"
                },
                if p["anthropic_supported"] == true {
                    " · 支持原生 Anthropic"
                } else {
                    ""
                }
            );
        }
    }
    println!(
        "  [i] 出站网络模式: {}",
        if snap["proxy"]["configured"] == true {
            "已配置本机持久代理（未回显具体地址）"
        } else {
            "直连（Loom 不读取终端 HTTPS_PROXY）"
        }
    );
    let (system_roots, had_load_errors) = network::system_root_status();
    println!(
        "  [i] TLS 证书与主机名校验已开启。本次自检加载了 {system_roots} 张系统根证书和内置 Mozilla WebPKI 根证书。"
    );
    if system_roots == 0 {
        println!("  [!] 本次自检使用内置公共根证书。需要信任企业 CA 时，请检查系统信任库。");
    } else if had_load_errors {
        println!("  [!] 部分系统根证书加载失败，已使用成功加载的证书继续校验。");
    }

    println!("\n[3/4] 后台服务与客户端同步");
    let gw_ok = snap["gateway"]["healthy"] == true;
    println!(
        "  [{}] 推理网关 (127.0.0.1:{gateway_port}): {}",
        if gw_ok { "✓" } else { "!" },
        if gw_ok {
            "运行正常"
        } else {
            "未运行（运行 loom install 或 loom serve 启动）"
        }
    );
    let admin_ok = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_millis(600))
        .build()
        .ok()
        .is_some();
    if admin_ok {
        let admin_alive = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_millis(600))
            .build()?
            .get(format!("http://127.0.0.1:{admin_port}/"))
            .send()
            .await
            .is_ok_and(|r| r.status().is_success());
        println!(
            "  [{}] 本机管控台 (127.0.0.1:{admin_port}): {}",
            if admin_alive { "✓" } else { "!" },
            if admin_alive {
                "可访问（运行 loom console 或打开 Loom.app）"
            } else {
                "未启动"
            }
        );
    }

    for key in ["pi", "opencode2", "claude_code"] {
        let c = &snap["clients"][key];
        let name = c["name"].as_str().unwrap_or(key);
        let synced = c["synced"] == true;
        let installed = c["installed"] == true;
        let conflict = c["conflict"].as_str();
        if let Some(msg) = conflict {
            println!("  [!] {name}: {msg}");
        } else if synced {
            println!(
                "  [✓] {name}: 已同步 ({} 个模型)",
                c["model_count"].as_u64().unwrap_or(0)
            );
        } else if installed {
            println!("  [!] {name}: 检测到配置目录但尚未同步，运行 loom sync 即可写入。");
        } else {
            println!("  [-] {name}: 未在本机检测到（需要时再配置即可）");
        }
    }

    println!("\n[4/4] 上游连通性实测");
    if !ping {
        println!(
            "  [i] 默认只做离线体检，未向厂商发请求；运行 `loom doctor --ping` 可实测真实 Key 连通性。"
        );
        return Ok(());
    }

    let targets: Vec<String> = providers
        .iter()
        .filter(|p| {
            let pid = p["id"].as_str().unwrap_or("");
            let prefix = p["prefix"].as_str().unwrap_or("");
            if let Some(filter) = target_provider {
                pid.eq_ignore_ascii_case(filter) || prefix.eq_ignore_ascii_case(filter)
            } else {
                p["enabled"] == true
            }
        })
        .filter_map(|p| p["id"].as_str().map(String::from))
        .collect();

    if targets.is_empty() {
        println!("  [!] 没有可实测的已启用厂商。");
        return Ok(());
    }

    let mut any_failed = false;
    for pid in targets {
        let res = verify_provider(
            data,
            &pid,
            None,
            #[cfg(test)]
            None,
        )
        .await?;
        let ok = res["ok"] == true;
        if !ok {
            any_failed = true;
        }
        println!(
            "  [{}] {pid} ({}) : {} ms — {}",
            if ok { "✓" } else { "✗" },
            res["model"].as_str().unwrap_or("-"),
            res["latency_ms"].as_u64().unwrap_or(0),
            res["message"].as_str().unwrap_or("")
        );
    }
    if any_failed {
        return Err("部分模型厂商连通性验证失败".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Json, Router, http::StatusCode, routing::post};

    #[tokio::test]
    async fn preset_custom_update_delete_lifecycle_is_safe_and_preserves_keys() {
        let t = tempfile::tempdir().unwrap();
        let d = t.path().join("data");
        let h = t.path().join("home");

        let initial = setup_snapshot(&d, &h, 4319, None).await.unwrap();
        assert_eq!(initial["needs_setup"], true);
        assert_eq!(initial["presets"].as_array().unwrap().len(), 6);

        let res = upsert_preset(&d, &h, 4319, "deepseek", Some("FIXTURE_DS_KEY"), None).unwrap();
        assert_eq!(res["ok"], true);
        assert_eq!(res["synced"], true);
        assert!(h.join(".pi/agent/models.json").is_file());

        let snap = setup_snapshot(&d, &h, 4319, None).await.unwrap();
        assert_eq!(snap["needs_setup"], false);
        assert_eq!(snap["clients"]["pi"]["synced"], true);
        assert!(
            !serde_json::to_string(&snap)
                .unwrap()
                .contains("FIXTURE_DS_KEY")
        );

        // Rotate key while preserving models.
        upsert_preset(&d, &h, 4319, "deepseek", Some("FIXTURE_DS_KEY_2"), None).unwrap();
        assert_eq!(
            fs::read_to_string(d.join("keys/local-deepseek.key")).unwrap(),
            "FIXTURE_DS_KEY_2"
        );

        // Add custom provider.
        let custom = upsert_custom(
            &d,
            &h,
            4319,
            &json!({
                "prefix": "demo",
                "name": "Demo Gateway",
                "base_url": "https://demo.invalid/v1",
                "models_text": "demo-1, demo-2",
                "api_key": "FIXTURE_CUSTOM_KEY"
            }),
        )
        .unwrap();
        assert_eq!(custom["provider_id"], "local-demo");

        // Toggle model off & add model via update_provider.
        update_provider(
            &d,
            &h,
            4319,
            &json!({
                "provider_id": "local-demo",
                "toggle_model": {"id": "demo-2", "enabled": false},
                "add_model": {"id": "demo-3", "name": "Demo 3", "maxTokens": 16384, "reasoning": true}
            }),
        )
        .unwrap();
        let snap2 = setup_snapshot(&d, &h, 4319, None).await.unwrap();
        let demo = snap2["providers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == "local-demo")
            .unwrap();
        assert_eq!(demo["active_model_count"], 2);

        // Delete custom provider removes managed key inside data/keys after backup.
        delete_provider(&d, &h, 4319, "local-demo").unwrap();
        assert!(!d.join("keys/local-demo.key").exists());
        assert!(d.join("backups").read_dir().unwrap().count() >= 1);
    }

    #[tokio::test]
    async fn verify_provider_classifies_statuses_without_leaking_secrets() {
        let t = tempfile::tempdir().unwrap();
        let d = t.path().join("data");
        let h = t.path().join("home");
        upsert_preset(&d, &h, 4319, "deepseek", Some("SECRET_PROBE_FIXTURE"), None).unwrap();

        let app = Router::new()
            .route(
                "/ok",
                post(|| async { Json(json!({"choices": [{"message": {"content": "pong"}}]})) }),
            )
            .route(
                "/unauth",
                post(|| async { (StatusCode::UNAUTHORIZED, Json(json!({"error": {"message": "bad SECRET_PROBE_FIXTURE"}}))) }),
            )
            .route(
                "/quota",
                post(|| async {
                    (
                        StatusCode::PAYMENT_REQUIRED,
                        Json(json!({"error": {"type": "quota_exceeded", "message": "raw internal text"}})),
                    )
                }),
            )
            .route(
                "/rate",
                post(|| async { (StatusCode::TOO_MANY_REQUESTS, Json(json!({}))) }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let ok = verify_provider(
            &d,
            "local-deepseek",
            None,
            Some(&format!("http://{addr}/ok")),
        )
        .await
        .unwrap();
        assert_eq!(ok["ok"], true);
        assert_eq!(ok["category"], "ok");

        let unauth = verify_provider(
            &d,
            "local-deepseek",
            None,
            Some(&format!("http://{addr}/unauth")),
        )
        .await
        .unwrap();
        assert_eq!(unauth["ok"], false);
        assert_eq!(unauth["category"], "auth_failed");
        assert!(
            !serde_json::to_string(&unauth)
                .unwrap()
                .contains("SECRET_PROBE_FIXTURE")
        );

        let quota = verify_provider(
            &d,
            "local-deepseek",
            None,
            Some(&format!("http://{addr}/quota")),
        )
        .await
        .unwrap();
        assert_eq!(quota["category"], "subscription_quota_exhausted");
        assert!(
            !serde_json::to_string(&quota)
                .unwrap()
                .contains("raw internal text")
        );

        let rate = verify_provider(
            &d,
            "local-deepseek",
            None,
            Some(&format!("http://{addr}/rate")),
        )
        .await
        .unwrap();
        assert_eq!(rate["category"], "rate_limited");

        server.abort();
    }
}
