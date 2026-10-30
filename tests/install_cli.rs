use std::{fs, path::Path, process::Command};
fn run(binary: &Path, home: &Path) -> std::process::Output {
    Command::new(binary)
        .args(["install", "--no-start"])
        .env_clear()
        .env("HOME", home)
        .env("PATH", "")
        .output()
        .unwrap()
}
#[test]
fn installs_with_empty_path_and_can_reinstall_itself_without_changing_keys() {
    let t = tempfile::tempdir().unwrap();
    let h = t.path().join("fixture home & spaces");
    let base = h.join(if cfg!(target_os = "macos") {
        "Library/Application Support/ModelHub"
    } else {
        ".local/share/ModelHub"
    });
    let d = base.join(".local");
    fs::create_dir_all(d.join("keys")).unwrap();
    let registry = b"{\"proxy\":\"\",\"providers\":{}}";
    let key = b"LOCAL_FIXTURE_ONLY\n";
    let upstream = b"UPSTREAM_FIXTURE_ONLY";
    fs::write(d.join("providers.json"), registry).unwrap();
    fs::write(d.join("hub.key"), key).unwrap();
    fs::write(d.join("keys/fixture.key"), upstream).unwrap();
    let result = run(Path::new(env!("CARGO_BIN_EXE_loom")), &h);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(base.join("loom").is_file());
    let result = run(&base.join("loom"), &h);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(fs::read(d.join("providers.json")).unwrap(), registry);
    assert_eq!(fs::read(d.join("hub.key")).unwrap(), key);
    assert_eq!(fs::read(d.join("keys/fixture.key")).unwrap(), upstream);
    assert_eq!(
        fs::read_link(h.join(".local/bin/modelhub")).unwrap(),
        base.join("loom")
    );
    assert!(fs::read_dir(d.join("backups")).unwrap().count() >= 2);
}
#[test]
fn invalid_existing_config_does_not_touch_old_binary() {
    let t = tempfile::tempdir().unwrap();
    let h = t.path();
    let base = h.join(if cfg!(target_os = "macos") {
        "Library/Application Support/ModelHub"
    } else {
        ".local/share/ModelHub"
    });
    fs::create_dir_all(base.join(".local")).unwrap();
    fs::write(base.join("loom"), b"old binary fixture").unwrap();
    fs::write(base.join(".local/providers.json"), b"malformed json").unwrap();
    let r = run(Path::new(env!("CARGO_BIN_EXE_loom")), h);
    assert!(!r.status.success());
    assert_eq!(fs::read(base.join("loom")).unwrap(), b"old binary fixture");
}
