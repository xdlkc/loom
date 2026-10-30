//! User-level installer. No shell, Python, credential export or system-wide changes.
use crate::config::{self, Result};
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};
const LABEL: &str = "local.model-access.hub";
#[derive(Clone, Copy)]
enum Platform {
    Mac,
    Linux,
}
impl Platform {
    fn current() -> Result<Self> {
        if cfg!(target_os = "macos") {
            Ok(Self::Mac)
        } else if cfg!(target_os = "linux") {
            Ok(Self::Linux)
        } else {
            Err("安装仅支持 macOS / Linux".into())
        }
    }
}
struct Layout {
    home: PathBuf,
    base: PathBuf,
    data: PathBuf,
    binary: PathBuf,
    unit: PathBuf,
    platform: Platform,
}
impl Layout {
    fn new(home: PathBuf, data: PathBuf, platform: Platform) -> Self {
        let base = home.join(match platform {
            Platform::Mac => "Library/Application Support/ModelHub",
            Platform::Linux => ".local/share/ModelHub",
        });
        let unit = home.join(match platform {
            Platform::Mac => "Library/LaunchAgents/local.model-access.hub.plist",
            Platform::Linux => ".config/systemd/user/local.model-access.hub.service",
        });
        Self {
            binary: base.join("loom"),
            base,
            home,
            data,
            unit,
            platform,
        }
    }
    fn files(&self) -> Vec<PathBuf> {
        vec![
            self.binary.clone(),
            self.home.join(".local/bin/loom"),
            self.home.join(".local/bin/modelhub"),
            self.unit.clone(),
        ]
    }
    fn service(&self) -> Result<String> {
        for p in [&self.home, &self.base, &self.data, &self.binary] {
            if p.to_string_lossy().contains(['\n', '\r', '\0']) {
                return Err("安装路径包含控制字符".into());
            }
        }
        let xml = |p: &Path| {
            p.to_string_lossy()
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace('"', "&quot;")
                .replace('\'', "&apos;")
        };
        let q = |p: &Path| {
            format!(
                "\"{}\"",
                p.to_string_lossy()
                    .replace('\\', "\\\\")
                    .replace('"', "\\\"")
                    .replace('%', "%%")
                    .replace('$', "$$")
            )
        };
        Ok(match self.platform {
            Platform::Mac => format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>Label</key><string>{LABEL}</string>
<key>ProgramArguments</key><array><string>{}</string><string>serve</string></array>
<key>WorkingDirectory</key><string>{}</string>
<key>RunAtLoad</key><true/><key>KeepAlive</key><true/><key>ThrottleInterval</key><integer>10</integer>
<key>EnvironmentVariables</key><dict><key>MODEL_HUB_DATA_DIR</key><string>{}</string><key>PATH</key><string>{}:/usr/bin:/bin</string></dict>
<key>StandardOutPath</key><string>{}</string><key>StandardErrorPath</key><string>{}</string>
</dict></plist>
"#,
                xml(&self.binary),
                xml(&self.base),
                xml(&self.data),
                xml(&self.home.join(".local/bin")),
                xml(&self.data.join("rust-service.log")),
                xml(&self.data.join("rust-service-error.log"))
            ),
            Platform::Linux => format!(
                "[Unit]\nDescription=Loom local model gateway\n[Service]\nType=simple\nWorkingDirectory={}\nExecStart={} serve\nEnvironment=\"MODEL_HUB_DATA_DIR={}\"\nRestart=on-failure\nUMask=0077\n[Install]\nWantedBy=default.target\n",
                q(&self.base),
                q(&self.binary),
                self.data
                    .to_string_lossy()
                    .replace('\\', "\\\\")
                    .replace('"', "\\\"")
                    .replace('%', "%%")
            ),
        })
    }
}
#[derive(Clone)]
enum Content {
    Absent,
    File(Vec<u8>, u32),
    Link(PathBuf),
}
struct Snapshot {
    path: PathBuf,
    content: Content,
}
#[cfg(unix)]
fn mode(p: &Path) -> Result<u32> {
    use std::os::unix::fs::PermissionsExt;
    Ok(fs::metadata(p)?.permissions().mode() & 0o777)
}
#[cfg(not(unix))]
fn mode(_: &Path) -> Result<u32> {
    Ok(0o600)
}
fn set_mode(p: &Path, mode: u32) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(p, fs::Permissions::from_mode(mode))?;
    }
    #[cfg(not(unix))]
    let _ = (p, mode);
    Ok(())
}
fn capture(path: PathBuf) -> Result<Snapshot> {
    let content = match fs::symlink_metadata(&path) {
        Ok(m) if m.file_type().is_symlink() => Content::Link(fs::read_link(&path)?),
        Ok(m) if m.is_file() => Content::File(fs::read(&path)?, mode(&path)?),
        Ok(_) => return Err("安装目标不是普通文件或链接，未覆盖".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Content::Absent,
        Err(e) => return Err(e.into()),
    };
    Ok(Snapshot { path, content })
}
fn link(target: &Path, path: &Path) -> Result<()> {
    fs::create_dir_all(path.parent().ok_or("parent required")?)?;
    let tmp = path.with_extension(format!("link-{:016x}", rand::random::<u64>()));
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, &tmp)?;
        fs::rename(&tmp, path)?;
    }
    #[cfg(not(unix))]
    {
        let _ = (target, tmp);
        return Err("此平台不支持安装".into());
    }
    Ok(())
}
impl Snapshot {
    fn restore(&self) -> Result<()> {
        match &self.content {
            Content::Absent => match fs::remove_file(&self.path) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(e.into()),
            },
            Content::File(v, m) => {
                config::write(&self.path, v)?;
                set_mode(&self.path, *m)
            }
            Content::Link(target) => link(target, &self.path),
        }
    }
}
fn snapshot(layout: &Layout) -> Result<(PathBuf, Vec<Snapshot>)> {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let backup = layout
        .data
        .join("backups")
        .join(format!("loom-install-{n}"));
    let mut files = vec![];
    let mut meta = vec![];
    for (i, p) in layout.files().into_iter().enumerate() {
        let s = capture(p)?;
        match &s.content {
            Content::File(v, m) => {
                config::write(&backup.join(format!("file-{i}")), v)?;
                meta.push(json!({"path":s.path,"file":format!("file-{i}"),"mode":m}));
            }
            Content::Link(t) => meta.push(json!({"path":s.path,"symlink":t})),
            Content::Absent => meta.push(json!({"path":s.path,"absent":true})),
        }
        files.push(s);
    }
    config::save(&backup.join("manifest.json"), &json!({"files":meta}))?;
    Ok((backup, files))
}
type Runner<'a> = dyn FnMut(&str, &[String]) -> Result<bool> + 'a;
fn args(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}
fn system_run(program: &str, args: &[String]) -> Result<bool> {
    Ok(Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?
        .success())
}
fn mac_reload(
    layout: &Layout,
    runner: &mut Runner<'_>,
    pause: &mut dyn FnMut(Duration),
) -> Result<()> {
    // id is queried without libc and contains no user credentials.
    let out = Command::new("/usr/bin/id").arg("-u").output()?;
    let uid = std::str::from_utf8(&out.stdout)?.trim();
    if !out.status.success() || !uid.bytes().all(|b| b.is_ascii_digit()) || uid.is_empty() {
        return Err("无法确定用户服务域".into());
    }
    mac_reload_in(&format!("gui/{uid}"), &layout.unit, runner, pause)
}
fn mac_reload_in(
    domain: &str,
    unit: &Path,
    runner: &mut Runner<'_>,
    pause: &mut dyn FnMut(Duration),
) -> Result<()> {
    let task = format!("{domain}/{LABEL}");
    let _ = runner("/bin/launchctl", &args(&["bootout", &task]))?;
    let mut stopped = false;
    for _ in 0..40 {
        if !runner("/bin/launchctl", &args(&["print", &task]))? {
            stopped = true;
            break;
        }
        pause(Duration::from_millis(100));
    }
    if !stopped {
        return Err("旧网关未停止，系统可能未允许管理启动项".into());
    }
    for _ in 0..8 {
        if runner(
            "/bin/launchctl",
            &args(&["bootstrap", domain, &unit.to_string_lossy()]),
        )? {
            return Ok(());
        }
        pause(Duration::from_millis(250));
    }
    Err("Loom 启动项加载失败".into())
}
fn reload(layout: &Layout, runner: &mut Runner<'_>, pause: &mut dyn FnMut(Duration)) -> Result<()> {
    match layout.platform {
        Platform::Mac => mac_reload(layout, runner, pause),
        Platform::Linux => {
            for a in [
                args(&["--user", "daemon-reload"]),
                args(&["--user", "enable", "--now", LABEL]),
                args(&["--user", "restart", LABEL]),
            ] {
                if !runner("systemctl", &a)? {
                    return Err("systemd 用户服务加载失败".into());
                }
            }
            Ok(())
        }
    }
}
fn deploy(layout: &Layout, source: &Path) -> Result<()> {
    let binary = fs::read(source)?;
    config::write(&layout.binary, &binary)?;
    set_mode(&layout.binary, 0o700)?;
    for name in ["loom", "modelhub"] {
        link(&layout.binary, &layout.home.join(".local/bin").join(name))?;
    }
    config::init(&layout.data)?;
    config::write(&layout.unit, layout.service()?.as_bytes())
}
fn restore(files: &[Snapshot]) -> Result<()> {
    let mut failed = false;
    for s in files.iter().rev() {
        failed |= s.restore().is_err();
    }
    if failed {
        Err("部分安装文件恢复失败，请使用私密备份恢复".into())
    } else {
        Ok(())
    }
}
fn stop_new_service(layout: &Layout, runner: &mut Runner<'_>) -> Result<()> {
    match layout.platform {
        Platform::Mac => {
            let out = Command::new("/usr/bin/id").arg("-u").output()?;
            let uid = std::str::from_utf8(&out.stdout)?.trim();
            if !out.status.success() || uid.is_empty() || !uid.bytes().all(|b| b.is_ascii_digit()) {
                return Err("无法确定用户服务域".into());
            }
            let _ = runner(
                "/bin/launchctl",
                &args(&["bootout", &format!("gui/{uid}/{LABEL}")]),
            )?;
        }
        Platform::Linux => {
            let _ = runner("systemctl", &args(&["--user", "disable", "--now", LABEL]))?;
        }
    }
    Ok(())
}
async fn healthy() -> Result<()> {
    let c = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(2))
        .build()?;
    for _ in 0..30 {
        if let Ok(r) = c.get("http://127.0.0.1:4319/health").send().await
            && r.status().is_success()
            && let Ok(v) = r.json::<serde_json::Value>().await
            && v["runtime"] == "rust"
        {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
    Err("Loom 健康检查失败".into())
}
pub async fn install(data: PathBuf, no_start: bool) -> Result<()> {
    let layout = Layout::new(config::home(), data, Platform::current()?);
    layout.service()?;
    let source = std::env::current_exe()?;
    // Existing config is read only, never overwritten or copied from the checkout.
    if layout.data.join("providers.json").exists() {
        config::catalog(&config::read(&layout.data.join("providers.json"))?)?;
    }
    let (backup, files) = snapshot(&layout)?;
    let existed = layout.unit.exists();
    let mut touched_service = false;
    let result: Result<()> = async {
        deploy(&layout, &source)?;
        if !no_start {
            touched_service = true;
            reload(&layout, &mut system_run, &mut std::thread::sleep)?;
            healthy().await?;
        }
        Ok(())
    }
    .await;
    if result.is_err() {
        if touched_service && !existed {
            let _ = stop_new_service(&layout, &mut system_run);
        }
        let restored = restore(&files);
        if restored.is_ok() && touched_service && existed {
            let _ = reload(&layout, &mut system_run, &mut std::thread::sleep);
        }
        eprintln!(
            "安装未完成，已尝试恢复原程序、命令和启动项。私密备份：{}",
            backup.display()
        );
        restored?;
        return Err("安装或服务检查失败；配置和密钥未覆盖".into());
    }
    println!(
        "Loom 已安装{}。配置与密钥保留，备份：{}",
        if no_start {
            "（未启动服务）"
        } else {
            "并启用登录自启动"
        },
        backup.display()
    );
    println!("入口：http://127.0.0.1:4319/v1；命令：loom / modelhub；额度服务未修改。");
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn layout(t: &tempfile::TempDir, p: Platform) -> Layout {
        Layout::new(
            t.path().join("home with & spaces"),
            t.path().join("private data"),
            p,
        )
    }
    #[test]
    fn both_platforms_escape_paths_and_use_gateway_label() {
        let t = tempfile::tempdir().unwrap();
        let mac = layout(&t, Platform::Mac);
        let xml = mac.service().unwrap();
        assert!(xml.contains("&amp;"));
        assert!(xml.contains(LABEL));
        assert!(!xml.contains("local.model-quota.manager"));
        let linux = layout(&t, Platform::Linux);
        let unit = linux.service().unwrap();
        assert!(unit.contains("ExecStart=\""));
        assert!(unit.contains("UMask=0077"));
    }
    #[test]
    fn install_rerun_preserves_config_keys_and_executable_mode() {
        let t = tempfile::tempdir().unwrap();
        let l = layout(&t, Platform::Mac);
        let src = t.path().join("source");
        fs::write(&src, b"binary fixture").unwrap();
        config::init(&l.data).unwrap();
        let key = fs::read(l.data.join("hub.key")).unwrap();
        let reg = fs::read(l.data.join("providers.json")).unwrap();
        deploy(&l, &src).unwrap();
        deploy(&l, &src).unwrap();
        assert_eq!(fs::read(l.data.join("hub.key")).unwrap(), key);
        assert_eq!(fs::read(l.data.join("providers.json")).unwrap(), reg);
        assert_eq!(mode(&l.binary).unwrap(), 0o700);
        assert_eq!(
            fs::read_link(l.home.join(".local/bin/modelhub")).unwrap(),
            l.binary
        );
    }
    #[test]
    fn transaction_restores_symlink_files_and_absent_targets() {
        let t = tempfile::tempdir().unwrap();
        let l = layout(&t, Platform::Mac);
        let src = t.path().join("source");
        fs::write(&src, b"new binary").unwrap();
        config::write(&l.binary, b"old binary").unwrap();
        set_mode(&l.binary, 0o755).unwrap();
        config::write(&l.unit, b"old unit").unwrap();
        let old = t.path().join("old.mjs");
        fs::write(&old, b"old node launcher").unwrap();
        link(&old, &l.home.join(".local/bin/modelhub")).unwrap();
        let (_, s) = snapshot(&l).unwrap();
        deploy(&l, &src).unwrap();
        restore(&s).unwrap();
        assert_eq!(fs::read(&l.binary).unwrap(), b"old binary");
        assert_eq!(mode(&l.binary).unwrap(), 0o755);
        assert_eq!(fs::read(&l.unit).unwrap(), b"old unit");
        assert_eq!(
            fs::read_link(l.home.join(".local/bin/modelhub")).unwrap(),
            old
        );
        assert!(!l.home.join(".local/bin/loom").exists());
    }
    #[test]
    fn linux_service_commands_only_target_gateway() {
        let t = tempfile::tempdir().unwrap();
        let l = layout(&t, Platform::Linux);
        let mut calls = vec![];
        reload(
            &l,
            &mut |program, a| {
                assert_eq!(program, "systemctl");
                calls.push(a.to_vec());
                Ok(true)
            },
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(
            calls,
            vec![
                args(&["--user", "daemon-reload"]),
                args(&["--user", "enable", "--now", LABEL]),
                args(&["--user", "restart", LABEL])
            ]
        );
        calls.clear();
        stop_new_service(&l, &mut |_, a| {
            calls.push(a.to_vec());
            Ok(true)
        })
        .unwrap();
        assert_eq!(calls, vec![args(&["--user", "disable", "--now", LABEL])]);
    }
    #[test]
    fn launchagent_waits_then_retries() {
        let mut codes = [true, true, false, false, true].into_iter();
        let mut calls = vec![];
        mac_reload_in(
            "gui/501",
            Path::new("fixture.plist"),
            &mut |_, a| {
                calls.push(a[0].clone());
                Ok(codes.next().unwrap())
            },
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(
            calls,
            vec!["bootout", "print", "print", "bootstrap", "bootstrap"]
        );
    }
    #[test]
    fn launchagent_does_not_bootstrap_over_running_job() {
        let mut calls = vec![];
        assert!(
            mac_reload_in(
                "gui/501",
                Path::new("fixture"),
                &mut |_, a| {
                    calls.push(a[0].clone());
                    Ok(true)
                },
                &mut |_| {}
            )
            .is_err()
        );
        assert!(!calls.iter().any(|v| v == "bootstrap"));
    }
}
