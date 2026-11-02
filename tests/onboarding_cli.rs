use std::process::{Command, Stdio};

#[test]
fn hidden_keys_require_a_terminal_and_never_accept_piped_credentials() {
    let t = tempfile::tempdir().unwrap();
    for args in [
        vec!["add"],
        vec!["add", "zenmux", "deepseek"],
        vec!["configure", "kimi"],
        vec!["add", "longcat"],
        vec!["add", "--custom"],
    ] {
        let r = Command::new(env!("CARGO_BIN_EXE_loom"))
            .args(args)
            .env("HOME", t.path())
            .env("MODEL_HUB_DATA_DIR", t.path().join("data"))
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(!r.status.success());
        assert!(String::from_utf8_lossy(&r.stderr).contains("本机交互终端"));
        assert!(!t.path().join("data").exists());
        assert!(!t.path().join(".pi").exists());
    }
}

#[test]
fn add_help_explains_presets_and_keeps_custom_configuration_available() {
    let r = Command::new(env!("CARGO_BIN_EXE_loom"))
        .args(["add", "--help"])
        .output()
        .unwrap();
    assert!(r.status.success());
    let help = String::from_utf8_lossy(&r.stdout);
    assert!(help.contains("[PROVIDERS]...") && help.contains("--custom"));
    let r = Command::new(env!("CARGO_BIN_EXE_loom"))
        .args(["add", "zenmux", "--custom"])
        .output()
        .unwrap();
    assert!(!r.status.success());
}
