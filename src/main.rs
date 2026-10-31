mod config;
mod gateway;
mod install;
mod onboarding;
mod update;
use clap::{Parser, Subcommand};
use config::Result;
use std::{io::IsTerminal, path::PathBuf, sync::Arc};
#[derive(Parser)]
#[command(version, about = "Loom — local-only unified model gateway.")]
struct Cli {
    #[arg(long, global = true)]
    data_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Init,
    /// Fetch, build, install and restart Loom while preserving local configuration.
    Update,
    /// Install this binary and user autostart service, preserving local configuration.
    Install {
        #[arg(
            long,
            help = "Deploy files and generate service configuration without starting it"
        )]
        no_start: bool,
    },
    Serve {
        #[arg(long, default_value_t = 4319)]
        port: u16,
    },
    /// Choose providers and enter hidden API Keys; names, URLs and models are preset.
    #[command(alias = "configure")]
    Add {
        /// Optional provider names, e.g. zenmux deepseek. Omit to choose interactively.
        #[arg(value_delimiter = ',', conflicts_with = "custom")]
        providers: Vec<String>,
        /// Manually configure an unlisted provider.
        #[arg(long)]
        custom: bool,
    },
    SetKey {
        provider: String,
    },
    Sync,
    Status,
    Check,
}
#[tokio::main]
async fn main() {
    if run().await.is_err() {
        eprintln!("操作失败：请检查本机配置、网络或文件权限。未输出凭据。");
        std::process::exit(1);
    }
}
async fn run() -> Result<()> {
    let cli = Cli::parse();
    let d = cli.data_dir.unwrap_or_else(config::data_dir);
    match cli.command {
        Command::Update => update::update(&d)?,
        Command::Install { no_start } => {
            if !no_start
                && std::io::stdin().is_terminal()
                && std::io::stdout().is_terminal()
                && onboarding::needs_setup(&d)?
            {
                onboarding::configure(&d, &[], false)?;
            }
            install::install(d, no_start).await?;
        }
        Command::Init => {
            config::init(&d)?;
            println!("已初始化。运行 loom add，选择供应商并隐藏输入 Key 即可。");
        }
        Command::Add { providers, custom } => onboarding::configure(&d, &providers, custom)?,
        Command::SetKey { provider } => {
            let r = config::read(&d.join("providers.json"))?;
            let p = r["providers"].get(&provider).ok_or("provider missing")?;
            let kp = PathBuf::from(p["keyFile"].as_str().ok_or("keyFile missing")?);
            let key = rpassword::prompt_password("API Key（隐藏输入）: ")?;
            validate_key(&key)?;
            config::backup(&kp, &d)?;
            config::write(&kp, key.trim().as_bytes())?;
            println!("Key 已保存在本机。");
        }
        Command::Sync => {
            config::sync(&d, &config::home())?;
            println!("已同步 pi / opencode2 和已有 Claude Code 配置。");
        }
        Command::Status => {
            let r = reqwest::Client::builder()
                .no_proxy()
                .timeout(std::time::Duration::from_secs(3))
                .build()?
                .get("http://127.0.0.1:4319/health")
                .send()
                .await?;
            if !r.status().is_success() {
                return Err("unhealthy".into());
            }
            println!("本机网关运行正常：http://127.0.0.1:4319/v1");
        }
        Command::Check => {
            let r = config::read(&d.join("providers.json"))?;
            let models = config::catalog(&r)?;
            for m in &models {
                if !std::path::Path::new(m.p["keyFile"].as_str().unwrap()).is_file() {
                    return Err("key missing".into());
                }
            }
            println!("配置有效，{} 个模型；不输出 Key。", models.len());
        }
        Command::Serve { port } => {
            config::init(&d)?;
            let bind = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await?;
            let sync_dir = d.clone();
            tokio::spawn(async move {
                let mut last = vec![];
                loop {
                    if let Ok(raw) = tokio::fs::read(sync_dir.join("providers.json")).await
                        && raw != last
                        && config::sync(&sync_dir, &config::home()).is_ok()
                    {
                        last = raw;
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
            });
            println!("本机模型网关：http://127.0.0.1:{port}/v1");
            axum::serve(
                bind,
                gateway::router(gateway::App {
                    #[cfg(test)]
                    upstream_override: None,
                    dir: d,
                    port,
                    permits: Arc::new(tokio::sync::Semaphore::new(8)),
                }),
            )
            .with_graceful_shutdown(async {
                let _ = tokio::signal::ctrl_c().await;
            })
            .await?;
        }
    }
    Ok(())
}
fn validate_key(s: &str) -> Result<()> {
    if s.trim().is_empty()
        || !s.trim().bytes().all(|b| b.is_ascii_graphic())
        || s.contains(['"', '\\'])
    {
        return Err("invalid key".into());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn safe_urls() {
        for s in ["http://a", "https://user:pass@a", "https://a/?key=x"] {
            assert!(config::endpoint(s).is_err());
        }
    }
    #[test]
    fn native_routes() {
        for (base, native) in [
            (
                "https://api.deepseek.com",
                "https://api.deepseek.com/anthropic/v1/messages",
            ),
            (
                "https://api.kimi.com/coding/v1",
                "https://api.kimi.com/coding/v1/messages",
            ),
        ] {
            assert_eq!(
                config::anthropic(&json!({"baseURL":base}))
                    .unwrap()
                    .unwrap(),
                native
            );
        }
    }
    #[test]
    fn config_private_idempotent() {
        let d = tempfile::tempdir().unwrap();
        config::init(d.path()).unwrap();
        let key = std::fs::read(d.path().join("hub.key")).unwrap();
        config::init(d.path()).unwrap();
        assert_eq!(key, std::fs::read(d.path().join("hub.key")).unwrap());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(d.path().join("hub.key"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }
}
