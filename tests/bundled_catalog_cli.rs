use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    process::{Child, Command, Stdio},
    time::Duration,
};

const ADDITIONS: [&str; 4] = [
    "meta/muse-wonder-1.0",
    "inclusionai/ling-3.1-flash",
    "xiaomi/mimo-v2.6-flash",
    "xiaomi/mimo-v2.6-pro",
];
fn write(p: &Path, v: &Value) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, serde_json::to_vec(v).unwrap()).unwrap();
}
fn fixture(home: &Path) -> std::path::PathBuf {
    let data = home.join("data");
    fs::create_dir_all(&data).unwrap();
    fs::write(data.join("hub.key"), b"HUB_FIXTURE_ONLY").unwrap();
    fs::write(data.join("upstream.key"), b"UPSTREAM_FIXTURE_ONLY").unwrap();
    write(
        &data.join("providers.json"),
        &json!({"proxy":"","providers":{"local-zenmux":{
            "prefix":"zenmux","name":"Existing provider","enabled":true,"baseURL":"https://zenmux.ai/api/v1",
            "keyFile":data.join("upstream.key"),"models":[{"id":"anthropic/claude-sonnet-5.5","name":"Sonnet custom label","maxTokens":8192}]
        }}}),
    );
    data
}
fn command(binary: &Path, home: &Path, data: &Path) -> Command {
    let mut c = Command::new(binary);
    c.env_clear()
        .env("HOME", home)
        .env("PATH", "")
        .env("MODEL_HUB_DATA_DIR", data);
    c.stdin(Stdio::null());
    c
}
#[test]
fn upgraded_binary_syncs_missing_models_on_another_home_without_changing_credentials() {
    let t = tempfile::tempdir().unwrap();
    let h = t.path();
    let d = fixture(h);
    let files = [
        d.join("providers.json"),
        d.join("hub.key"),
        d.join("upstream.key"),
    ];
    let original: Vec<_> = files.iter().map(|p| fs::read(p).unwrap()).collect();
    let defaults = json!({"model":"zenmux/anthropic/claude-sonnet-5.5","permissions":{"defaultMode":"manual"},"theme":"dark"});
    write(&h.join(".claude/settings.json"), &defaults);
    write(
        &h.join(".pi/agent/settings.json"),
        &json!({"defaultProvider":"native-oauth","defaultModel":"keep","enabledModels":["native-oauth/keep","hub/zenmux/anthropic/claude-sonnet-5.5"]}),
    );
    write(
        &h.join(".config/opencode/opencode.json"),
        &json!({"model":"native-oauth/keep"}),
    );
    let binary = Path::new(env!("CARGO_BIN_EXE_loom"));
    let install = command(binary, h, &d)
        .args(["install", "--no-start"])
        .output()
        .unwrap();
    assert!(install.status.success());
    let installed = h.join(if cfg!(target_os = "macos") {
        "Library/Application Support/ModelHub/loom"
    } else {
        ".local/share/ModelHub/loom"
    });
    let sync = command(&installed, h, &d).arg("sync").output().unwrap();
    assert!(sync.status.success());
    for (p, b) in files.iter().zip(original) {
        assert_eq!(fs::read(p).unwrap(), b);
    }
    let load =
        |p: &str| -> Value { serde_json::from_slice(&fs::read(h.join(p)).unwrap()).unwrap() };
    let cc = load(".claude/settings.json");
    let pi = load(".pi/agent/models.json");
    let oc = load(".config/opencode/opencode.json");
    for m in ADDITIONS {
        let id = format!("zenmux/{m}");
        assert!(
            cc["modelPicker"]["options"]
                .as_array()
                .unwrap()
                .iter()
                .any(|m| m["model"] == id)
        );
        assert!(
            pi["providers"]["hub"]["models"]
                .as_array()
                .unwrap()
                .iter()
                .any(|m| m["id"] == id)
        );
        assert!(oc["providers"]["hub"]["models"].get(&id).is_some());
        assert!(
            load(".pi/agent/settings.json")["enabledModels"]
                .as_array()
                .unwrap()
                .contains(&json!(format!("hub/{id}")))
        );
    }
    for k in ["model", "permissions", "theme"] {
        assert_eq!(cc[k], defaults[k]);
    }
    assert_eq!(oc["model"], "native-oauth/keep");
    assert_eq!(
        load(".pi/agent/settings.json")["defaultProvider"],
        "native-oauth"
    );
    let before = fs::read(h.join(".claude/settings.json")).unwrap();
    assert!(
        command(&installed, h, &d)
            .arg("sync")
            .output()
            .unwrap()
            .status
            .success()
    );
    assert_eq!(fs::read(h.join(".claude/settings.json")).unwrap(), before);
}

struct Server(Child);
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
#[tokio::test]
async fn running_gateway_and_startup_client_sync_share_the_bundled_catalog() {
    let t = tempfile::tempdir().unwrap();
    let h = t.path();
    let d = fixture(h);
    write(&h.join(".claude/settings.json"), &json!({"theme":"dark"}));
    let before = fs::read(d.join("providers.json")).unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let mut server = Server(
        command(Path::new(env!("CARGO_BIN_EXE_loom")), h, &d)
            .args(["serve", "--port", &port.to_string(), "--admin-port", "0"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    let mut catalog = None;
    for _ in 0..100 {
        if let Ok(r) = client
            .get(format!("http://127.0.0.1:{port}/v1/models"))
            .bearer_auth("HUB_FIXTURE_ONLY")
            .send()
            .await
            && r.status().is_success()
        {
            catalog = Some(r.json::<Value>().await.unwrap());
            break;
        }
        assert!(server.0.try_wait().unwrap().is_none());
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let catalog = catalog.expect("gateway did not become ready");
    for m in ADDITIONS {
        assert!(
            catalog["data"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v["id"] == format!("zenmux/{m}"))
        );
    }
    // HTTP readiness and the background client sync are independent tasks.
    let mut count = 0;
    for _ in 0..100 {
        let cc =
            serde_json::from_slice::<Value>(&fs::read(h.join(".claude/settings.json")).unwrap())
                .unwrap();
        count = cc["modelPicker"]["options"].as_array().map_or(0, Vec::len);
        if count == 5 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(count, 5, "background client sync did not complete");
    assert_eq!(fs::read(d.join("providers.json")).unwrap(), before);
}
