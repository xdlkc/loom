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
#[test]
fn thinking_metadata_follows_upstream_and_preserves_preferences() {
    let t = tempfile::tempdir().unwrap();
    let d = t.path().join("data");
    let h = t.path().join("home");
    init(&d).unwrap();
    let registry = json!({"providers":{
        "custom-route":{"prefix":"wallet","enabled":true,"baseURL":"https://api.deepseek.com/v1","keyFile":"fixture","models":[{"id":"deepseek-flash","reasoning":true},{"id":"deepseek-v4-pro","reasoning":true}]},
        "not-deepseek":{"prefix":"deepseek","enabled":true,"baseURL":"https://other.invalid","keyFile":"fixture","models":[{"id":"deepseek-flash","reasoning":true},{"id":"plain","reasoning":false}]}
    }});
    save(&d.join("providers.json"), &registry).unwrap();
    let preferences = json!({"defaultProvider":"native","defaultModel":"keep","defaultThinkingLevel":"off","modelThinkingLevels":{"hub/wallet/deepseek-flash":"high"},"theme":"dark"});
    save(&h.join(".pi/agent/settings.json"), &preferences).unwrap();
    sync(&d, &h).unwrap();
    assert_eq!(read(&d.join("providers.json")).unwrap(), registry);
    assert_eq!(
        read(&h.join(".pi/agent/settings.json")).unwrap(),
        preferences
    );
    let pi = read(&h.join(".pi/agent/models.json")).unwrap();
    let oc = read(&h.join(".config/opencode/opencode.json")).unwrap();
    for id in ["wallet/deepseek-flash", "wallet/deepseek-v4-pro"] {
        let m = pi["providers"]["hub"]["models"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["id"] == id)
            .unwrap();
        assert_eq!(m["compat"]["thinkingFormat"], "deepseek");
        assert_eq!(m["compat"]["supportsReasoningEffort"], true);
        assert_eq!(m["thinkingLevelMap"]["medium"], "high");
        assert_eq!(m["thinkingLevelMap"]["max"], "max");
        let m = &oc["providers"]["hub"]["models"][id];
        assert_eq!(m["capabilities"]["reasoning"]["supported"], true);
        assert_eq!(m["compatibility"]["reasoningField"], "reasoning_content");
        let off = &m["variants"][0];
        assert_eq!(off["id"], "none");
        assert_eq!(off["body"]["thinking"]["type"], "disabled");
        assert!(off["body"].get("reasoning_effort").is_none());
    }
    let other = pi["providers"]["hub"]["models"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["id"] == "deepseek/deepseek-flash")
        .unwrap();
    assert!(other["compat"].get("thinkingFormat").is_none());
    assert!(
        oc["providers"]["hub"]["models"]["deepseek/deepseek-flash"]
            .get("variants")
            .is_none()
    );
    assert_eq!(
        oc["providers"]["hub"]["models"]["deepseek/plain"]["capabilities"]["reasoning"]["supported"],
        false
    );
    let before = fs::read(h.join(".pi/agent/models.json")).unwrap();
    sync(&d, &h).unwrap();
    assert_eq!(fs::read(h.join(".pi/agent/models.json")).unwrap(), before);
}
#[test]
fn explicit_pi_compat_and_level_overrides_survive_sync() {
    let m = Model {
        id: "custom/deepseek-flash".into(),
        p: json!({"baseURL":"https://api.deepseek.com","piCompat":{"supportsUsageInStreaming":false,"thinkingFormat":"zai"}}),
        m: json!({"id":"deepseek-flash","reasoning":true,"compat":{"thinkingFormat":"deepseek","supportsReasoningEffort":false},"thinkingLevelMap":{"off":null,"medium":"max"}}),
    };
    let pi = pi_model(&m, "fixture".into());
    assert_eq!(pi["compat"]["thinkingFormat"], "deepseek");
    assert_eq!(pi["compat"]["supportsUsageInStreaming"], false);
    assert_eq!(pi["compat"]["supportsReasoningEffort"], false);
    assert_eq!(pi["thinkingLevelMap"]["medium"], "max");
    assert_eq!(pi["thinkingLevelMap"]["off"], Value::Null);
}
#[test]
fn sync_merges_comment_free_jsonc_in_place_without_creating_json() {
    let t = tempfile::tempdir().unwrap();
    let d = t.path().join("data");
    let h = t.path().join("home");
    init(&d).unwrap();
    let kp = d.join("keys/test.key");
    write(&kp, b"UPSTREAM_FIXTURE").unwrap();
    save(
        &d.join("providers.json"),
        &json!({"providers":{"local-test":{"prefix":"test","name":"Test","enabled":true,"baseURL":"https://api.deepseek.com","keyFile":kp,"models":[{"id":"deepseek-flash","name":"Flash","reasoning":true}]}}}),
    )
    .unwrap();
    let json = h.join(".config/opencode/opencode.json");
    let jsonc = json.with_extension("jsonc");
    save(
        &jsonc,
        &json!({
            "theme":"fixture-theme",
            "instructions":["https://example.invalid/a//b"],
            "plugins":[{"package":"existing/plugin","options":{"keep":true}}],
            "providers":{"existing":{"name":"keep"}},
            "model":"existing/model"
        }),
    )
    .unwrap();
    sync(&d, &h).unwrap();
    assert!(!json.exists());
    let oc = read(&jsonc).unwrap();
    assert_eq!(oc["theme"], "fixture-theme");
    assert_eq!(oc["instructions"][0], "https://example.invalid/a//b");
    assert_eq!(oc["providers"]["existing"]["name"], "keep");
    assert_eq!(oc["model"], "existing/model");
    assert_eq!(
        oc["providers"]["hub"]["models"]["test/deepseek-flash"]["name"],
        "hub/Flash · test"
    );
    assert!(
        oc["plugins"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["package"] == "existing/plugin")
    );
    let before = fs::read(&jsonc).unwrap();
    sync(&d, &h).unwrap();
    assert_eq!(fs::read(&jsonc).unwrap(), before);
}
#[test]
fn sync_rejects_jsonc_comments_and_dual_client_configs() {
    let t = tempfile::tempdir().unwrap();
    let d = t.path().join("data");
    let h = t.path().join("home");
    init(&d).unwrap();
    let kp = d.join("keys/test.key");
    write(&kp, b"UPSTREAM_FIXTURE").unwrap();
    save(
        &d.join("providers.json"),
        &json!({"providers":{"local-test":{"prefix":"test","enabled":true,"baseURL":"https://api.deepseek.com","keyFile":kp,"models":[{"id":"deepseek-flash"}]}}}),
    )
    .unwrap();
    let json = h.join(".config/opencode/opencode.json");
    let jsonc = json.with_extension("jsonc");
    let commented = b"{\n  // preserve this comment\n  \"theme\": \"keep\"\n}";
    write(&jsonc, commented).unwrap();
    assert!(sync(&d, &h).is_err());
    assert_eq!(fs::read(&jsonc).unwrap(), commented);
    assert!(!json.exists());
    assert!(!h.join(".pi/agent/models.json").exists());
    fs::remove_file(&jsonc).unwrap();
    let jsonc_value = json!({"theme":"jsonc"});
    save(&jsonc, &jsonc_value).unwrap();
    save(&json, &json!({"theme":"json"})).unwrap();
    let jsonc_before = fs::read(&jsonc).unwrap();
    let json_before = fs::read(&json).unwrap();
    assert!(sync(&d, &h).is_err());
    assert_eq!(fs::read(&jsonc).unwrap(), jsonc_before);
    assert_eq!(fs::read(&json).unwrap(), json_before);
    assert!(!h.join(".pi/agent/models.json").exists());
}
