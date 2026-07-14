use std::path::PathBuf;

use std::time::Duration;

use tokio::time::sleep;
use velux2mqtt::klf200::{CommandId, Klf200Client, Klf200Config, Request, Response};

#[tokio::test]
#[ignore = "requires V2M_KLF_HOST and V2M_KLF_PASSWORD and connects to physical hardware"]
async fn login_versions_and_clean_shutdown() {
    let host = std::env::var("V2M_KLF_HOST").expect("V2M_KLF_HOST is required");
    let password = std::env::var("V2M_KLF_PASSWORD").expect("V2M_KLF_PASSWORD is required");
    let mut config = Klf200Config::new(host);
    if let Some(certificate) = std::env::var_os("V2M_KLF_CERTIFICATE") {
        config.certificate = Some(PathBuf::from(certificate));
    }

    let client = Klf200Client::connect(config).await.expect("connect to KLF 200");
    client.login(password).await.expect("authenticate");
    let gateway = client.version().await.expect("gateway version");
    let protocol = client.protocol_version().await.expect("protocol version");
    assert_ne!(gateway.software, [0; 6]);
    assert_ne!(protocol.major, 0);
    client.shutdown().await.expect("clean shutdown");
}

#[tokio::test]
#[ignore = "requires physical KLF hardware and deliberately validates monitor shutdown plus reconnect"]
async fn monitor_shutdown_allows_tls_reconnect() {
    let host = std::env::var("V2M_KLF_HOST").expect("V2M_KLF_HOST is required");
    let password = std::env::var("V2M_KLF_PASSWORD").expect("V2M_KLF_PASSWORD is required");
    let mut config = Klf200Config::new(host);
    if let Some(certificate) = std::env::var_os("V2M_KLF_CERTIFICATE") {
        config.certificate = Some(PathBuf::from(certificate));
    }

    let client = Klf200Client::connect(config.clone())
        .await
        .expect("first connect to KLF 200");
    client.login(password.clone()).await.expect("first authentication");
    assert_eq!(
        client
            .send(Request::HouseStatusMonitorEnable)
            .await
            .expect("enable monitor"),
        Response::Acknowledgement {
            command: CommandId::GW_HOUSE_STATUS_MONITOR_ENABLE_CFM,
        }
    );
    client
        .shutdown()
        .await
        .expect("disable monitor and close first connection");

    sleep(Duration::from_secs(1)).await;
    let client = Klf200Client::connect(config).await.expect("reconnect to KLF 200");
    client.login(password).await.expect("second authentication");
    client.version().await.expect("read version after reconnect");
    client.shutdown().await.expect("close second connection");
}
