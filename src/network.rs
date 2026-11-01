use crate::config::{self, Result};
use std::{error::Error, path::Path};

// Error chains can contain URLs and credentials. Inspect in memory; return only
// fixed categories and guidance, never a source's display text.
pub fn failure(e: &reqwest::Error) -> (&'static str, &'static str) {
    if e.is_timeout() {
        return (
            "timeout",
            "连接或等待上游响应超时，请检查网络和代理是否可用。",
        );
    }
    if e.is_builder() {
        return (
            "request_invalid",
            "请求构建失败，请检查配置和 Key 格式；不要粘贴原始密钥到日志。",
        );
    }
    let mut source: Option<&(dyn Error + 'static)> = e.source();
    while let Some(error) = source {
        let text = error.to_string().to_ascii_lowercase();
        if [
            "certificate",
            "tls",
            "ssl",
            "handshake",
            "invalidcontenttype",
            "invalid content type",
        ]
        .iter()
        .any(|s| text.contains(s))
        {
            return (
                "tls_error",
                "TLS 握手或证书验证失败，请检查系统时间、代理和证书链；保持证书验证开启。",
            );
        }
        if [
            "dns error",
            "failed to lookup address",
            "name or service not known",
            "nodename nor servname",
        ]
        .iter()
        .any(|s| text.contains(s))
        {
            return (
                "dns_error",
                "域名解析失败，请检查 DNS 或配置可用的本机代理。",
            );
        }
        source = error.source();
    }
    if e.is_connect() {
        (
            "connect_failed",
            "连接上游或配置的代理失败，请检查代理是否运行及其端口。Loom 不读取终端 HTTPS_PROXY；使用 loom proxy 设置持久代理。",
        )
    } else {
        (
            "transport_failed",
            "请求未获得有效上游响应，具体网络原因尚未确认。请检查网络与本机代理，不必重新添加供应商。",
        )
    }
}

pub fn proxy(data: &Path, value: Option<&str>) -> Result<()> {
    let path = data.join("providers.json");
    let mut registry = config::read(&path)?;
    if let Some(value) = value {
        let value = if value == "off" { "" } else { value };
        if !value.is_empty() && !valid_proxy(value) {
            eprintln!(
                "代理地址须为 HTTP/HTTPS URL，不含用户名、密码、查询参数或自定义路径。未修改配置。"
            );
            return Err("invalid proxy".into());
        }
        config::catalog(&registry)?;
        registry["proxy"] = value.into();
        config::backup(&path, data)?;
        config::save(&path, &registry)?;
    }
    if registry["proxy"].as_str().is_some_and(|v| !v.is_empty()) {
        println!("Loom 已配置持久代理；无需重启，下一次请求生效。未显示代理地址或凭据。");
    } else {
        println!(
            "Loom 当前直连，不读取终端 HTTP_PROXY / HTTPS_PROXY。需要代理时运行 loom proxy http://127.0.0.1:<实际端口>。"
        );
    }
    Ok(())
}

fn valid_proxy(value: &str) -> bool {
    reqwest::Url::parse(value).is_ok_and(|u| {
        ["http", "https"].contains(&u.scheme())
            && u.host_str().is_some()
            && u.username().is_empty()
            && u.password().is_none()
            && u.query().is_none()
            && u.fragment().is_none()
            && u.path() == "/"
            && reqwest::Proxy::all(value).is_ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn proxy_changes_only_routing_and_rejects_credential_bearing_urls() {
        let d = tempfile::tempdir().unwrap();
        config::init(d.path()).unwrap();
        let p = d.path().join("providers.json");
        let original = config::read(&p).unwrap();
        let key = std::fs::read(d.path().join("hub.key")).unwrap();
        proxy(d.path(), Some("http://127.0.0.1:17890")).unwrap();
        let configured = config::read(&p).unwrap();
        assert_eq!(configured["proxy"], "http://127.0.0.1:17890");
        assert_eq!(configured["providers"], original["providers"]);
        let before = std::fs::read(&p).unwrap();
        for invalid in [
            "http://user:private@localhost:7890",
            "http://localhost:7890/?key=private",
            "http://localhost:7890/private",
            "socks5://localhost:7890",
        ] {
            assert!(proxy(d.path(), Some(invalid)).is_err());
            assert_eq!(std::fs::read(&p).unwrap(), before);
        }
        proxy(d.path(), Some("off")).unwrap();
        assert_eq!(config::read(&p).unwrap(), original);
        assert_eq!(std::fs::read(d.path().join("hub.key")).unwrap(), key);
    }
    #[tokio::test]
    async fn connection_errors_do_not_reflect_private_urls() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let error = reqwest::Client::builder()
            .no_proxy()
            .build()
            .unwrap()
            .get(format!("http://{address}/?key=fixture-ssl-secret"))
            .send()
            .await
            .unwrap_err();
        let (category, guidance) = failure(&error);
        assert_eq!(category, "connect_failed");
        assert!(
            !guidance.contains("fixture-ssl-secret") && !guidance.contains(&address.to_string())
        );
    }
    #[tokio::test]
    async fn stalled_upstream_is_reported_as_timeout() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let error = reqwest::Client::builder()
            .no_proxy()
            .timeout(std::time::Duration::from_millis(100))
            .build()
            .unwrap()
            .get(format!("http://{}/", listener.local_addr().unwrap()))
            .send()
            .await
            .unwrap_err();
        assert_eq!(failure(&error).0, "timeout");
    }
}
