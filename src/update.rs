//! Update from the fixed, authenticated repository, without touching user checkouts.
use crate::config::{self, Result};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
const REPOSITORY: &str = "https://github.com/xdlkc/loom.git";
struct WorkDir(PathBuf);
impl Drop for WorkDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
struct Step {
    program: PathBuf,
    args: Vec<String>,
    cwd: Option<PathBuf>,
    path: std::ffi::OsString,
}
fn run(step: &Step) -> Result<()> {
    let mut c = Command::new(&step.program);
    c.args(&step.args)
        .env("PATH", &step.path)
        .stdin(Stdio::inherit())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(cwd) = &step.cwd {
        c.current_dir(cwd);
    }
    if !c.status()?.success() {
        return Err("更新步骤失败".into());
    }
    Ok(())
}
struct Credentials(Vec<(PathBuf, Vec<u8>)>);
impl Credentials {
    fn capture(data: &Path) -> Result<Self> {
        let registry = data.join("providers.json");
        let r = config::read(&registry)?;
        config::catalog(&r)?;
        let mut paths = vec![registry, data.join("hub.key")];
        for p in r["providers"]
            .as_object()
            .ok_or("providers missing")?
            .values()
        {
            if let Some(k) = p["keyFile"].as_str() {
                paths.push(PathBuf::from(k));
            }
        }
        paths.sort();
        paths.dedup();
        let mut values = vec![];
        for p in paths {
            values.push((p.clone(), fs::read(p)?));
        }
        Ok(Self(values))
    }
    fn unchanged(&self) -> bool {
        self.0
            .iter()
            .all(|(p, b)| fs::read(p).is_ok_and(|v| v == *b))
    }
}
fn update_with(
    data: &Path,
    cargo: &Path,
    runner: &mut dyn FnMut(&Step) -> Result<()>,
) -> Result<()> {
    let data = fs::canonicalize(data)?;
    let credentials = Credentials::capture(&data)?;
    let root = data.join("updates");
    fs::create_dir_all(&root)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
    }
    let mut options = fs::OpenOptions::new();
    options.create(true).truncate(false).read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let lock = options.open(root.join("update.lock"))?;
    lock.try_lock()
        .map_err(|_| "已有 Loom 更新任务运行，请稍后重试")?;
    let work = WorkDir(root.join(format!("run-{:016x}", rand::random::<u64>())));
    fs::create_dir(&work.0)?;
    let source = work.0.join("source");
    let target = root.join("target");
    let home = config::home();
    let mut paths = vec![home.join(".cargo/bin"), home.join(".local/bin")];
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    let path = std::env::join_paths(paths)?;
    println!("Loom update：正在获取官方私有仓库，需要此机器已有 GitHub 访问权限…");
    runner(&Step{program:"git".into(),args:vec!["-c".into(),"core.hooksPath=/dev/null".into(),"clone".into(),"--depth".into(),"1".into(),"--single-branch".into(),"--branch".into(),"main".into(),"--".into(),REPOSITORY.into(),source.to_string_lossy().into()],cwd:None,path:path.clone()}).map_err(|_|"源码获取失败；请检查网络及 GitHub 认证（gh auth login / gh auth setup-git）。旧程序和服务未改动。")?;
    println!("正在构建新版本，旧网关继续运行…");
    runner(&Step {
        program: cargo.into(),
        args: vec![
            "build".into(),
            "--release".into(),
            "--locked".into(),
            "--target-dir".into(),
            target.to_string_lossy().into(),
        ],
        cwd: Some(source),
        path: path.clone(),
    })
    .map_err(|_| "构建失败；请检查 Rust 工具链和依赖下载网络。旧程序和服务未改动。")?;
    if !credentials.unchanged() {
        return Err(
            "配置或 Key 在构建期间发生变化，已停止更新，请重试。未主动修改任何 Key。".into(),
        );
    }
    let binary = target.join("release/loom");
    if !binary.is_file() {
        return Err("未找到构建产物，旧网关未改动".into());
    }
    println!("构建完成，正在安装并重启 Loom（自动备份；失败尝试恢复）…");
    runner(&Step {
        program: binary,
        args: vec![
            "--data-dir".into(),
            data.to_string_lossy().into(),
            "install".into(),
        ],
        cwd: None,
        path,
    })
    .map_err(
        |_| "安装未完成；安装器已尝试恢复旧版本，请检查本机 backups/loom-install-*。未输出凭据。",
    )?;
    if !credentials.unchanged() {
        return Err("更新后配置/Key 校验不同，请检查本机配置；更新器未主动改写 Key。".into());
    }
    println!("Loom 更新及服务重启完成，配置和密钥校验一致。无需修改 agent 配置。");
    Ok(())
}
pub fn update(data: &Path) -> Result<()> {
    let cargo = config::home().join(".cargo/bin/cargo");
    let cargo = if cargo.is_file() {
        cargo
    } else {
        PathBuf::from("cargo")
    };
    // Check dependencies before creating or modifying update state.
    if !Command::new(&cargo)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
    {
        return Err("更新需要 Rust 工具链，请从 rustup.rs 安装。旧网关未改动。".into());
    }
    if !Command::new("git")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
    {
        return Err("更新需要 Git。旧网关未改动。".into());
    }
    if let Err(e) = update_with(data, &cargo, &mut run) {
        eprintln!("{e}");
        return Err("update failed".into());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn fixture() -> tempfile::TempDir {
        let t = tempfile::tempdir().unwrap();
        config::init(t.path()).unwrap();
        let k = t.path().join("upstream.key");
        config::write(&k, b"UPSTREAM_FIXTURE_ONLY").unwrap();
        config::save(&t.path().join("providers.json"),&json!({"providers":{"local-demo":{"enabled":true,"baseURL":"https://demo.invalid/v1","keyFile":k,"models":[{"id":"fixture"}]}}})).unwrap();
        t
    }
    #[test]
    fn update_plan_uses_fixed_repo_locked_build_and_installer() {
        let t = fixture();
        let before = Credentials::capture(t.path()).unwrap();
        let mut phases = vec![];
        update_with(t.path(), Path::new("cargo"), &mut |s| {
            phases.push(s.args.clone());
            if s.args.first().is_some_and(|v| v == "build") {
                let i = s.args.iter().position(|v| v == "--target-dir").unwrap();
                config::write(
                    &PathBuf::from(&s.args[i + 1]).join("release/loom"),
                    b"binary fixture",
                )?;
            }
            Ok(())
        })
        .unwrap();
        assert!(before.unchanged());
        assert!(phases[0].contains(&REPOSITORY.to_string()));
        assert!(phases[1].contains(&"--locked".into()));
        assert_eq!(phases[2].last().unwrap(), "install");
        assert!(!phases[2].contains(&"init".into()));
        assert!(
            fs::read_dir(t.path().join("updates")).unwrap().all(|p| !p
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("run-"))
        );
    }
    #[test]
    fn build_failure_never_installs_and_preserves_keys() {
        let t = fixture();
        let before = Credentials::capture(t.path()).unwrap();
        let mut count = 0;
        let r = update_with(t.path(), Path::new("cargo"), &mut |_| {
            count += 1;
            if count == 2 {
                Err("fixture build failure".into())
            } else {
                Ok(())
            }
        });
        assert!(r.is_err());
        assert_eq!(count, 2);
        assert!(before.unchanged());
    }
    #[test]
    fn key_change_aborts_before_install() {
        let t = fixture();
        let mut count = 0;
        let r = update_with(t.path(), Path::new("cargo"), &mut |_| {
            count += 1;
            if count == 2 {
                config::write(&t.path().join("hub.key"), b"NEW_FIXTURE_KEY")?;
            }
            Ok(())
        });
        assert!(r.is_err());
        assert_eq!(count, 2);
    }
    #[test]
    fn concurrent_update_is_refused() {
        let t = fixture();
        let root = t.path().join("updates");
        fs::create_dir(&root).unwrap();
        let f = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(root.join("update.lock"))
            .unwrap();
        f.try_lock().unwrap();
        assert!(
            update_with(t.path(), Path::new("cargo"), &mut |_| panic!(
                "must not run"
            ))
            .is_err()
        );
    }
}
