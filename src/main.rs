mod config;
mod gateway;
mod install;
mod update;
use clap::{Parser, Subcommand};
use config::Result;
use serde_json::json;
use std::{path::PathBuf, sync::Arc};
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
    Add,
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
        Command::Install { no_start } => install::install(d, no_start).await?,
        Command::Init => {
            config::init(&d)?;
            println!("已初始化本机私有配置。运行 loom add 添加 provider；Key 隐藏输入。");
        }
        Command::Add => {
            config::init(&d)?;
            let prefix =
                config::prompt("Provider 标识（如 zenmux / glm / minimax / deepseek / kimi）")?;
            let name = config::prompt("显示名称")?;
            let base = config::endpoint(&config::prompt("OpenAI 兼容 Base URL（HTTPS）")?)?;
            let native = config::prompt("自定义 Anthropic Base URL（常见厂商可留空）")?;
            if !native.is_empty() {
                config::endpoint(&native)?;
            }
            let billing = config::prompt("计费来源")?;
            let ids = config::prompt("Model ID（逗号分隔）")?;
            let models:Vec<_>=ids.split(',').map(str::trim).filter(|v|!v.is_empty()).map(|id|json!({"id":id,"name":id,"contextWindow":128000,"maxTokens":8192,"input":["text"],"reasoning":false})).collect();
            if models.is_empty() {
                return Err("models required".into());
            }
            let mut r = config::read(&d.join("providers.json"))?;
            let pid = format!("local-{prefix}");
            if r["providers"].get(&pid).is_some() {
                return Err("provider exists".into());
            }
            let kp = d.join("keys").join(format!("{pid}.key"));
            let mut p = json!({"prefix":prefix,"name":name,"baseURL":base,"billing":billing,"enabled":true,"keyFile":kp,"models":models});
            if !native.is_empty() {
                p["anthropicBaseURL"] = json!(native);
            }
            r["providers"][&pid] = p;
            config::catalog(&r)?;
            let key =
                rpassword::prompt_password("推理 API Key（隐藏输入，不是 Management Key）: ")?;
            validate_key(&key)?;
            if kp.exists() {
                return Err("key already exists".into());
            }
            config::backup(&d.join("providers.json"), &d)?;
            config::write(&kp, key.trim().as_bytes())?;
            config::save(&d.join("providers.json"), &r)?;
            config::sync(&d, &config::home())?;
            println!("已保存并同步客户端，运行中的会话请刷新模型列表。");
        }
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
