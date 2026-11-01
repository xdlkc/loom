use std::{fs, process::Command};

#[test]
fn proxy_cli_preserves_provider_keys_and_rejects_credentials_without_echoing() {
    let t = tempfile::tempdir().unwrap();
    let d = t.path().join("data");
    fs::create_dir_all(&d).unwrap();
    let key = d.join("provider.key");
    fs::write(&key, "PRIVATE_UPSTREAM_FIXTURE").unwrap();
    fs::write(d.join("hub.key"), "PRIVATE_LOCAL_FIXTURE").unwrap();
    let registry = serde_json::json!({"proxy":"","providers":{"local-test":{"prefix":"test","enabled":true,"name":"Test","baseURL":"https://provider.invalid/v1","keyFile":key,"models":[{"id":"model","name":"My model","maxTokens":8192}]}},"extra":"preserve"});
    fs::write(
        d.join("providers.json"),
        serde_json::to_vec(&registry).unwrap(),
    )
    .unwrap();
    let run = |value: &str| {
        Command::new(env!("CARGO_BIN_EXE_loom"))
            .args(["--data-dir", d.to_str().unwrap(), "proxy", value])
            .env_clear()
            .env("HOME", t.path())
            .output()
            .unwrap()
    };
    assert!(run("http://127.0.0.1:17890").status.success());
    let changed: serde_json::Value =
        serde_json::from_slice(&fs::read(d.join("providers.json")).unwrap()).unwrap();
    let mut expected = registry.clone();
    expected["proxy"] = "http://127.0.0.1:17890".into();
    assert_eq!(changed, expected);
    let rejected = run("http://user:PRIVATE_PROXY_FIXTURE@localhost:17890");
    assert!(!rejected.status.success());
    assert!(!String::from_utf8_lossy(&rejected.stdout).contains("PRIVATE_PROXY_FIXTURE"));
    assert!(!String::from_utf8_lossy(&rejected.stderr).contains("PRIVATE_PROXY_FIXTURE"));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&fs::read(d.join("providers.json")).unwrap())
            .unwrap(),
        expected
    );
    assert!(run("off").status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&fs::read(d.join("providers.json")).unwrap())
            .unwrap(),
        registry
    );
    assert_eq!(
        fs::read_to_string(&key).unwrap(),
        "PRIVATE_UPSTREAM_FIXTURE"
    );
    assert_eq!(
        fs::read_to_string(d.join("hub.key")).unwrap(),
        "PRIVATE_LOCAL_FIXTURE"
    );
}
