use axum::{Router, http::StatusCode, routing::post, serve::Listener};
use rustls::{
    ServerConfig,
    pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject},
};
use std::{fs, net::SocketAddr, process::Command, sync::Arc, time::Duration};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::{TlsAcceptor, server::TlsStream};

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

struct TlsListener(TcpListener, TlsAcceptor);

impl Listener for TlsListener {
    type Io = TlsStream<TcpStream>;
    type Addr = SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            let (socket, address) = self.0.accept().await.unwrap();
            if let Ok(tls) = self.1.accept(socket).await {
                return (tls, address);
            }
        }
    }

    fn local_addr(&self) -> std::io::Result<Self::Addr> {
        self.0.local_addr()
    }
}

#[tokio::test]
async fn doctor_accepts_enterprise_v1_root_only_when_trusted() {
    let dir = tempfile::tempdir().unwrap();
    let root_pem = dir.path().join("system-roots.pem");
    let root = include_bytes!("fixtures/tls/root.pem");
    let certs = vec![
        include_bytes!("fixtures/tls/leaf.der").as_slice().into(),
        include_bytes!("fixtures/tls/intermediate.der")
            .as_slice()
            .into(),
        CertificateDer::from_pem_slice(root).unwrap(),
    ];
    // This private key belongs to the localhost test fixture.
    let key =
        PrivateKeyDer::try_from(include_bytes!("fixtures/tls/leaf-key.der").as_slice()).unwrap();
    let tls = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(
            TlsListener(listener, TlsAcceptor::from(Arc::new(tls))),
            Router::new().route("/v1/chat/completions", post(|| async { StatusCode::OK })),
        )
        .await
        .unwrap();
    });

    let key_file = dir.path().join("provider.key");
    fs::write(&key_file, "TEST_PROVIDER_KEY").unwrap();
    fs::write(
        dir.path().join("providers.json"),
        serde_json::to_vec(&serde_json::json!({"proxy":"","providers":{"test":{
            "prefix":"test", "enabled":true, "name":"Test",
            "baseURL":format!("https://{upstream}/v1"), "keyFile":key_file,
            "models":[{"id":"model","name":"Test Model"}]
        }}}))
        .unwrap(),
    )
    .unwrap();

    // Set the trust-store path only in the child process to isolate the test
    // from the host trust store.
    let probe = || async {
        let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_loom"));
        command
            .args([
                "--data-dir",
                dir.path().to_str().unwrap(),
                "doctor",
                "--port",
                "0",
                "--admin-port",
                "0",
                "--ping",
                "test",
            ])
            .env_clear()
            .env("HOME", dir.path())
            .env("SSL_CERT_FILE", &root_pem)
            .kill_on_drop(true);
        tokio::time::timeout(Duration::from_secs(10), command.output())
            .await
            .unwrap()
            .unwrap()
    };
    let assert_rejected = |rejected: std::process::Output| {
        let stdout = String::from_utf8_lossy(&rejected.stdout);
        assert!(!rejected.status.success(), "{stdout}");
        assert!(stdout.contains("TLS 握手或证书验证失败"), "{stdout}");
    };
    assert_rejected(probe().await);
    // Valid PEM framing with invalid certificate DER should also allow the
    // client to build and reach certificate verification.
    fs::write(
        &root_pem,
        "-----BEGIN CERTIFICATE-----\nMAA=\n-----END CERTIFICATE-----\n",
    )
    .unwrap();
    assert_rejected(probe().await);

    fs::write(&root_pem, root).unwrap();
    let trusted = probe().await;
    let stdout = String::from_utf8_lossy(&trusted.stdout);
    assert!(trusted.status.success(), "{stdout}");
    assert!(stdout.contains("连接成功"), "{stdout}");
    server.abort();
}
