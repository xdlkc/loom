//! Public model additions shipped with Loom; private provider files stay untouched.
use crate::config::Result;
use serde_json::Value;

pub fn supplement(provider: &Value, models: &mut Vec<Value>) -> Result<()> {
    if provider["includeBundledModels"] == false {
        return Ok(());
    }
    let catalog: Value = serde_json::from_str(include_str!("../catalog/zenmux.json"))?;
    if provider["baseURL"]
        .as_str()
        .map(|s| s.trim_end_matches('/'))
        != catalog["baseURL"].as_str()
    {
        return Ok(());
    }
    for m in catalog["models"]
        .as_array()
        .ok_or("bundled models required")?
    {
        // Even a disabled local entry takes precedence over the public catalog.
        if !models.iter().any(|local| local["id"] == m["id"]) {
            models.push(m.clone());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config;
    use serde_json::json;

    fn registry() -> Value {
        json!({"providers":{"local-zenmux":{"enabled":true,"baseURL":"https://zenmux.ai/api/v1/",
            "keyFile":"FIXTURE_KEY_FILE","models":[{"id":"anthropic/claude-sonnet-5.5"}]}}})
    }
    #[test]
    fn supplements_legacy_registry_without_mutating_it_or_creating_providers() {
        let r = registry();
        let before = r.clone();
        let models = config::catalog(&r).unwrap();
        assert_eq!(r, before);
        assert_eq!(models.len(), 5);
        assert!(models.iter().any(|m| m.id == "zenmux/meta/muse-wonder-1.0"));
        assert!(!models.iter().any(|m| m.id.contains("clef")));
        for m in models {
            assert!(config::anthropic(&m.p).unwrap().is_some());
        }
        assert!(
            config::catalog(&json!({"providers":{}}))
                .unwrap()
                .is_empty()
        );
    }
    #[test]
    fn local_overrides_and_disabled_stubs_take_precedence() {
        let mut r = registry();
        r["providers"]["local-zenmux"]["models"] = json!([
            {"id":"xiaomi/mimo-v2.6-flash","enabled":false},
            {"id":"meta/muse-wonder-1.0","name":"Custom name","maxTokens":777,"contextWindow":12345},
            {"id":"my-custom-model","maxTokens":444}
        ]);
        let models = config::catalog(&r).unwrap();
        assert!(
            !models
                .iter()
                .any(|m| m.id == "zenmux/xiaomi/mimo-v2.6-flash")
        );
        let custom = models
            .iter()
            .find(|m| m.id == "zenmux/meta/muse-wonder-1.0")
            .unwrap();
        assert_eq!(custom.m["name"], "Custom name");
        assert_eq!(custom.m["maxTokens"], 777);
        assert_eq!(models.iter().filter(|m| m.id == custom.id).count(), 1);
        assert!(models.iter().any(|m| m.id == "zenmux/my-custom-model"));
    }
    #[test]
    fn opt_out_disabled_and_unrelated_routes_do_not_gain_models() {
        let mut r = registry();
        r["providers"]["local-zenmux"]["includeBundledModels"] = json!(false);
        assert_eq!(config::catalog(&r).unwrap().len(), 1);
        r["providers"]["local-zenmux"]["includeBundledModels"] = json!(true);
        r["providers"]["local-zenmux"]["baseURL"] = json!("https://custom.invalid/v1");
        assert_eq!(config::catalog(&r).unwrap().len(), 1);
        r["providers"]["local-zenmux"]["baseURL"] = json!("https://zenmux.ai/api/v1");
        r["providers"]["local-zenmux"]["enabled"] = json!(false);
        assert!(config::catalog(&r).unwrap().is_empty());
    }
}
