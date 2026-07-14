use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use clap::Parser;
use clap_verbosity_flag::{InfoLevel, Verbosity};
use velux2mqtt::bridge::{VeluxMqttBridge, VeluxMqttBridgeConfig};
use velux2mqtt::klf200::Klf200Config;
use velux2mqtt::mqtt::MqttConfig;

#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
    /// KLF 200 hostname or IP address.
    #[arg(long, env = "V2M_KLF_HOST")]
    klf_host: String,

    /// KLF 200 API port.
    #[arg(long, env = "V2M_KLF_PORT", default_value_t = 51_200)]
    klf_port: u16,

    /// KLF WLAN password. Prefer `V2M_KLF_PASSWORD` to avoid process-list exposure.
    #[arg(long, env = "V2M_KLF_PASSWORD", hide_env_values = true)]
    klf_password: String,

    /// Heartbeat interval in seconds.
    #[arg(long, env = "V2M_KLF_HEARTBEAT_INTERVAL", default_value_t = 30)]
    klf_heartbeat_interval: u64,

    /// Stationary-node status refresh interval in seconds.
    #[arg(long, env = "V2M_STATUS_REFRESH_INTERVAL", default_value_t = 300)]
    status_refresh_interval: u64,

    /// PEM certificate used to authenticate the KLF gateway.
    #[arg(long, env = "V2M_KLF_CERTIFICATE")]
    certificate: Option<PathBuf>,

    /// Reboot the KLF after disabling house monitoring during graceful shutdown.
    #[arg(long, env = "V2M_KLF_REBOOT_ON_SHUTDOWN", default_value_t = false)]
    klf_reboot_on_shutdown: bool,

    /// Delay before reconnecting after a requested KLF reboot, in seconds.
    #[arg(long, env = "V2M_KLF_REBOOT_RECONNECT_DELAY", default_value_t = 30)]
    klf_reboot_reconnect_delay: u64,

    /// MQTT broker hostname or IP address.
    #[arg(long, env = "V2M_MQTT_ADDRESS")]
    mqtt_addr: String,

    /// MQTT broker port.
    #[arg(long, env = "V2M_MQTT_PORT", default_value_t = 1883)]
    mqtt_port: u16,

    #[arg(long, env = "V2M_MQTT_USER", default_value = "")]
    mqtt_user: String,

    #[arg(long, env = "V2M_MQTT_PASS", default_value = "", hide_env_values = true)]
    mqtt_pass: String,

    #[arg(long, env = "V2M_CLIENT_ID", default_value = "velux2mqtt")]
    mqtt_client_id: String,

    #[arg(long, env = "V2M_MQTT_BASE_TOPIC", default_value = "velux")]
    mqtt_base_topic: String,

    #[arg(long, env = "V2M_HASS_DISCOVERY", default_value_t = false)]
    hass_discovery: bool,

    #[command(flatten)]
    verbosity: Verbosity<InfoLevel>,
}

impl Cli {
    fn validate(&self) -> Result<()> {
        if self.klf_host.trim().is_empty() {
            bail!("KLF host cannot be empty");
        }
        if self.klf_password.is_empty() {
            bail!("KLF password cannot be empty");
        }
        if self.klf_heartbeat_interval == 0 {
            bail!("KLF heartbeat interval must be greater than zero");
        }
        if self.status_refresh_interval == 0 {
            bail!("status refresh interval must be greater than zero");
        }
        if self.klf_reboot_reconnect_delay == 0 {
            bail!("KLF reboot reconnect delay must be greater than zero");
        }
        if self.mqtt_addr.trim().is_empty() {
            bail!("MQTT address cannot be empty");
        }
        if self.mqtt_client_id.trim().is_empty() {
            bail!("MQTT client ID cannot be empty");
        }
        if self.mqtt_base_topic.trim_matches('/').is_empty() {
            bail!("MQTT base topic cannot be empty");
        }
        if let Some(path) = &self.certificate {
            std::fs::metadata(path).with_context(|| format!("cannot read KLF certificate at {}", path.display()))?;
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    env_logger::Builder::new()
        .filter_level(cli.verbosity.log_level_filter())
        .init();
    cli.validate()?;

    log::info!(
        "configuration validated for KLF {}:{} and MQTT {}:{}",
        cli.klf_host,
        cli.klf_port,
        cli.mqtt_addr,
        cli.mqtt_port
    );
    log::debug!(
        "client_id={}, base_topic={}, hass_discovery={}, klf_reboot_on_shutdown={}, klf_reboot_reconnect_delay={}, mqtt_user_configured={}, mqtt_password_configured={}",
        cli.mqtt_client_id,
        cli.mqtt_base_topic,
        cli.hass_discovery,
        cli.klf_reboot_on_shutdown,
        cli.klf_reboot_reconnect_delay,
        !cli.mqtt_user.is_empty(),
        !cli.mqtt_pass.is_empty()
    );
    let mut klf = Klf200Config::new(cli.klf_host);
    klf.port = cli.klf_port;
    klf.certificate = cli.certificate;

    let mut mqtt = MqttConfig::new(cli.mqtt_addr);
    mqtt.port = cli.mqtt_port;
    mqtt.user = cli.mqtt_user;
    mqtt.password = cli.mqtt_pass;
    mqtt.client_id = cli.mqtt_client_id;
    mqtt.base_topic = cli.mqtt_base_topic;

    let mut bridge_config = VeluxMqttBridgeConfig::new(klf, cli.klf_password, mqtt);
    bridge_config.hass_discovery = cli.hass_discovery;
    bridge_config.reboot_on_shutdown = cli.klf_reboot_on_shutdown;
    bridge_config.reboot_reconnect_delay = Duration::from_secs(cli.klf_reboot_reconnect_delay);
    bridge_config.heartbeat_interval = Duration::from_secs(cli.klf_heartbeat_interval);
    bridge_config.status_refresh_interval = Duration::from_secs(cli.status_refresh_interval);

    VeluxMqttBridge::new(bridge_config)?.run().await
}
