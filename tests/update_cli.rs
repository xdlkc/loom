#![cfg(unix)]
use std::{fs, os::unix::fs::PermissionsExt, path::Path, process::Command};
fn executable(p: &Path, body: &str) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, body).unwrap();
    fs::set_permissions(p, fs::Permissions::from_mode(0o700)).unwrap();
}
#[test]
fn update_cli_builds_then_installs_without_copying_credentials() {
    let t = tempfile::tempdir().unwrap();
    let home = t.path().join("home with spaces");
    let data = home.join("data");
    fs::create_dir_all(&data).unwrap();
    fs::write(
        data.join("providers.json"),
        b"{\"proxy\":\"\",\"providers\":{}}",
    )
    .unwrap();
    fs::write(data.join("hub.key"), b"LOCAL_FIXTURE_ONLY").unwrap();
    let bin = home.join(".local/bin");
    executable(
        &bin.join("git"),
        r#"#!/bin/sh
case "$1" in --version) echo 'git fixture'; exit 0;; esac
for arg do dest="$arg"; done
/bin/mkdir -p "$dest"
printf 'clone' > "$HOME/clone-phase"
"#,
    );
    executable(
        &home.join(".cargo/bin/cargo"),
        r#"#!/bin/sh
case "$1" in --version) echo 'cargo fixture'; exit 0;; esac
while [ "$#" -gt 0 ]; do
 if [ "$1" = '--target-dir' ]; then shift; target="$1"; fi
 shift
done
/bin/mkdir -p "$target/release"
/bin/cat > "$target/release/loom" <<'INNER'
#!/bin/sh
[ "$1" = '--data-dir' ] && [ "$3" = 'install' ] || exit 1
printf 'installed' > "$HOME/install-phase"
INNER
/bin/chmod 700 "$target/release/loom"
"#,
    );
    let result = Command::new(env!("CARGO_BIN_EXE_loom"))
        .args(["--data-dir", data.to_str().unwrap(), "update"])
        .env_clear()
        .env("HOME", &home)
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(fs::read(home.join("install-phase")).unwrap(), b"installed");
    assert_eq!(
        fs::read(data.join("hub.key")).unwrap(),
        b"LOCAL_FIXTURE_ONLY"
    );
    assert_eq!(
        fs::read(data.join("providers.json")).unwrap(),
        b"{\"proxy\":\"\",\"providers\":{}}"
    );
}
