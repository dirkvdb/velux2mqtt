use std::collections::{BTreeMap, BTreeSet};
use std::future::{Future, pending};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use async_trait::async_trait;
use tokio::sync::{broadcast, oneshot};
use tokio::task::JoinHandle;
use tokio::time::{Instant, Interval, MissedTickBehavior, interval_at, sleep_until, timeout};

use crate::hassdiscovery::HomeAssistantDiscovery;
use crate::klf200::{
    CommandTarget, ConnectionEvent, GatewayState, GroupId, GroupInformation, Klf200Client, Klf200Config, KlfError,
    NodeId, NodeInformation, Percentage, ProductGroupActivationRequest, ProtocolVersion, Request, Response,
    SceneContactRequest, SessionIdAllocator, StandardParameter, StatusRequestType, Version,
};
use crate::mqtt::{
    GroupAction, IncomingMessage, MqttCommand, MqttConfig, MqttConnection, MqttError, MqttEvent, MqttPublication,
    TopicLayout,
};
use crate::veluxnode::{CoverAction, CoverState, NodeError, VeluxNode, VeluxNodeCache};

const DEFAULT_STATUS_REFRESH: Duration = Duration::from_mins(5);

pub struct VeluxMqttBridgeConfig {
    pub klf: Klf200Config,
    pub klf_password: String,
    pub mqtt: MqttConfig,
    pub hass_discovery: bool,
    pub reboot_on_shutdown: bool,
    pub reboot_reconnect_delay: Duration,
    pub heartbeat_interval: Duration,
    pub status_refresh_interval: Duration,
    pub status_response_timeout: Duration,
    pub reconnect_min_delay: Duration,
    pub reconnect_max_delay: Duration,
}

impl VeluxMqttBridgeConfig {
    #[must_use]
    pub fn new(klf: Klf200Config, klf_password: impl Into<String>, mqtt: MqttConfig) -> Self {
        let status_response_timeout = klf.connection.request_timeout;
        Self {
            klf,
            klf_password: klf_password.into(),
            mqtt,
            hass_discovery: false,
            reboot_on_shutdown: false,
            reboot_reconnect_delay: Duration::from_secs(30),
            heartbeat_interval: Duration::from_secs(30),
            status_refresh_interval: DEFAULT_STATUS_REFRESH,
            status_response_timeout,
            reconnect_min_delay: Duration::from_secs(1),
            reconnect_max_delay: Duration::from_mins(1),
        }
    }

    /// Validates bridge intervals and credentials.
    ///
    /// # Errors
    ///
    /// Returns an error for empty KLF values or zero/inverted timing settings.
    pub fn validate(&mut self) -> Result<()> {
        self.klf.host = self.klf.host.trim().to_owned();
        if self.klf.host.is_empty() {
            bail!("KLF host cannot be empty");
        }
        if self.klf_password.is_empty() {
            bail!("KLF password cannot be empty");
        }
        self.mqtt.validate()?;
        if self.reboot_reconnect_delay.is_zero()
            || self.heartbeat_interval.is_zero()
            || self.status_refresh_interval.is_zero()
            || self.status_response_timeout.is_zero()
            || self.reconnect_min_delay.is_zero()
            || self.reconnect_max_delay < self.reconnect_min_delay
        {
            bail!("bridge intervals must be non-zero and reconnect maximum must not be below its minimum");
        }
        Ok(())
    }
}

pub struct VeluxMqttBridge {
    config: VeluxMqttBridgeConfig,
    mqtt: Box<dyn Broker>,
    topics: TopicLayout,
    discovery: Option<HomeAssistantDiscovery>,
    connector: Arc<dyn GatewayConnector>,
    gateway: Option<Arc<dyn Gateway>>,
    gateway_events: Option<broadcast::Receiver<ConnectionEvent>>,
    setup: Option<GatewaySetup>,
    cache: VeluxNodeCache,
    groups: BTreeMap<GroupId, GroupInformation>,
    session_ids: SessionIdAllocator,
    mqtt_connected: bool,
    next_reconnect: Instant,
    reconnect: ReconnectBackoff,
}

impl VeluxMqttBridge {
    /// Creates a production bridge backed by rumqttc and the TLS KLF client.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid bridge or MQTT configuration.
    pub fn new(mut config: VeluxMqttBridgeConfig) -> Result<Self> {
        config.validate()?;
        let topics = TopicLayout::new(&config.mqtt.base_topic)?;
        let discovery = config
            .hass_discovery
            .then(|| HomeAssistantDiscovery::new(config.mqtt.client_id.clone()));
        let mqtt = MqttConnection::new(config.mqtt.clone())?;
        let reconnect = ReconnectBackoff::new(config.reconnect_min_delay, config.reconnect_max_delay);
        Ok(Self {
            config,
            mqtt: Box::new(mqtt),
            topics,
            discovery,
            connector: Arc::new(TlsGatewayConnector),
            gateway: None,
            gateway_events: None,
            setup: None,
            cache: VeluxNodeCache::new(),
            groups: BTreeMap::new(),
            session_ids: SessionIdAllocator::new(),
            mqtt_connected: false,
            next_reconnect: Instant::now(),
            reconnect,
        })
    }

    /// Runs until an operating-system shutdown signal is received.
    ///
    /// # Errors
    ///
    /// Returns an error if signal registration or graceful shutdown fails.
    pub async fn run(self) -> Result<()> {
        self.run_until(shutdown_signal()).await
    }

    async fn run_until<F>(mut self, shutdown: F) -> Result<()>
    where
        F: Future<Output = Result<()>> + Send,
    {
        tokio::pin!(shutdown);
        let mut heartbeat = configured_interval(self.config.heartbeat_interval);
        let mut refresh = configured_interval(self.config.status_refresh_interval);

        loop {
            tokio::select! {
                shutdown_result = &mut shutdown => {
                    shutdown_result?;
                    break;
                }
                mqtt_result = self.mqtt.poll() => self.handle_mqtt_result(mqtt_result).await,
                setup_result = wait_for_setup(&mut self.setup) => self.handle_setup_result(setup_result).await,
                gateway_event = receive_gateway_event(&mut self.gateway_events) => {
                    self.handle_gateway_event(gateway_event).await;
                }
                () = wait_for_reconnect(self.next_reconnect), if self.gateway.is_none() && self.setup.is_none() => {
                    self.start_gateway_setup();
                }
                _ = heartbeat.tick(), if self.gateway.is_some() => self.heartbeat().await,
                _ = refresh.tick(), if self.gateway.is_some() => self.refresh_stationary_nodes().await,
            }
        }

        self.shutdown().await
    }

    fn start_gateway_setup(&mut self) {
        let connector = Arc::clone(&self.connector);
        let klf_config = self.config.klf.clone();
        let password = self.config.klf_password.clone();
        let status_timeout = self.config.status_response_timeout;
        let (cancel_tx, cancel_rx) = oneshot::channel();
        self.setup = Some(GatewaySetup {
            task: tokio::spawn(async move {
                prepare_gateway(connector, klf_config, password, status_timeout, cancel_rx).await
            }),
            cancel: Some(cancel_tx),
        });
    }

    async fn handle_setup_result(&mut self, result: Result<PreparedGateway>) {
        self.setup = None;
        match result {
            Ok(prepared) => {
                let removed = self
                    .cache
                    .iter()
                    .filter(|(node_id, _)| prepared.cache.get(**node_id).is_none())
                    .map(|(_, node)| node.clone())
                    .collect::<Vec<_>>();
                let removed_groups = self
                    .groups
                    .iter()
                    .filter(|(group_id, _)| !prepared.groups.contains_key(group_id))
                    .map(|(_, group)| group.clone())
                    .collect::<Vec<_>>();
                self.gateway = Some(prepared.gateway);
                self.gateway_events = Some(prepared.events);
                self.cache = prepared.cache;
                self.groups = prepared.groups;
                self.session_ids = prepared.session_ids;
                self.reconnect.reset();
                log::info!(
                    "KLF connection initialized with {} nodes and {} groups",
                    self.cache.iter().count(),
                    self.groups.len()
                );
                if self.mqtt_connected {
                    if let Err(error) = self.clear_removed_nodes(&removed).await {
                        log::error!("failed to clear removed node topics: {error:#}");
                    }
                    if let Err(error) = self.clear_removed_groups(&removed_groups).await {
                        log::error!("failed to clear removed group topics: {error:#}");
                    }
                    if let Err(error) = self.publish_complete_snapshot().await {
                        log::error!("failed to publish KLF snapshot: {error:#}");
                    }
                }
            }
            Err(error) => {
                let delay = self.reconnect.next_delay();
                self.next_reconnect = Instant::now() + delay;
                log::error!("KLF setup failed: {error:#}; retrying in {delay:?}");
                self.publish_availability(false).await;
            }
        }
    }

    async fn handle_mqtt_result(&mut self, result: Result<MqttEvent, MqttError>) {
        match result {
            Ok(MqttEvent::Connected) => {
                self.mqtt_connected = true;
                log::info!("connected to MQTT broker");
                if self.gateway.is_some() {
                    if let Err(error) = self.publish_complete_snapshot().await {
                        log::error!("failed to publish MQTT snapshot after connect: {error:#}");
                    }
                } else {
                    self.publish_availability(false).await;
                }
            }
            Ok(MqttEvent::Message(message)) => self.handle_mqtt_message(message).await,
            Err(error) => {
                if self.mqtt_connected {
                    log::warn!("MQTT connection lost: {error}");
                } else {
                    log::debug!("MQTT connection attempt failed: {error}");
                }
                self.mqtt_connected = false;
            }
        }
    }

    async fn handle_mqtt_message(&mut self, message: IncomingMessage) {
        let command = match self.topics.parse_command(&message) {
            Ok(command) => command,
            Err(MqttError::RetainedCommand) => {
                log::warn!("ignored retained MQTT command on {}", message.topic);
                return;
            }
            Err(error) => {
                log::warn!("ignored invalid MQTT command: {error}");
                return;
            }
        };
        if self.gateway.is_none() {
            log::warn!("ignored MQTT command while KLF is offline");
            return;
        }
        match command {
            MqttCommand::Node(command) => {
                if let Err(error) = self.execute_cover_command(command.node_id, command.action).await {
                    log::error!("cover command for node {} failed: {error:#}", command.node_id.get());
                }
            }
            MqttCommand::Group(command) => {
                if let Err(error) = self.execute_group_command(command.group_id, command.action).await {
                    log::error!("group command for group {} failed: {error:#}", command.group_id.get());
                }
            }
            MqttCommand::GatewayReboot => {
                if let Err(error) = self.execute_gateway_reboot().await {
                    log::error!("gateway reboot command failed: {error:#}");
                }
            }
        }
    }

    async fn execute_gateway_reboot(&mut self) -> Result<()> {
        let gateway = Arc::clone(self.gateway()?);
        let reboot_result = gateway.shutdown(true).await;

        self.gateway = None;
        self.gateway_events = None;
        self.cache.set_all_available(false);
        self.session_ids = SessionIdAllocator::new();
        self.reconnect.reset();
        let delay = self.config.reboot_reconnect_delay;
        self.next_reconnect = Instant::now() + delay;
        self.publish_availability(false).await;

        match reboot_result {
            Ok(()) => {
                log::info!("KLF house monitoring disabled and gateway reboot accepted; reconnecting after {delay:?}");
                Ok(())
            }
            Err(error) => Err(error).context("KLF monitor-disable/reboot sequence failed"),
        }
    }

    async fn execute_cover_command(&mut self, node_id: NodeId, action: CoverAction) -> Result<()> {
        let node = self
            .cache
            .get(node_id)
            .cloned()
            .ok_or_else(|| anyhow!("node {} is not present", node_id.get()))?;
        if let Err(NodeError::PositionUnknown { .. }) = node.resolve_action(action) {
            self.request_status(node_id).await?;
            bail!("node {} position is unknown; requested a fresh status", node_id.get());
        }
        let endpoint = node.resolve_action(action)?;
        let session_id = self.session_ids.allocate()?;
        let request = match node.command_request(session_id, action) {
            Ok(request) => request,
            Err(error) => {
                self.session_ids.release(session_id);
                return Err(error.into());
            }
        };
        let gateway = match self.gateway() {
            Ok(gateway) => Arc::clone(gateway),
            Err(error) => {
                self.session_ids.release(session_id);
                return Err(error);
            }
        };
        let response = gateway.send(request).await;
        match response {
            Ok(response @ Response::CommandAccepted { status: 1, .. }) => {
                if let Some(node) = self.cache.get_mut(node_id) {
                    node.apply_command_confirmation(&response, session_id, endpoint);
                }
                self.publish_node_status(node_id).await?;
                Ok(())
            }
            Ok(Response::CommandAccepted { status, .. }) => {
                self.session_ids.release(session_id);
                bail!("KLF rejected command session {session_id} with status {status}");
            }
            Ok(response) => {
                self.session_ids.release(session_id);
                bail!("unexpected command response: {response:?}");
            }
            Err(error) => {
                self.session_ids.release(session_id);
                Err(error.into())
            }
        }
    }

    async fn execute_group_command(&mut self, group_id: GroupId, action: GroupAction) -> Result<()> {
        let group = self
            .groups
            .get(&group_id)
            .cloned()
            .ok_or_else(|| anyhow!("group {} is not present", group_id.get()))?;
        let session_id = self.session_ids.allocate()?;
        let position = match action {
            GroupAction::Open => Percentage::FULLY_OPEN,
            GroupAction::Close => Percentage::FULLY_CLOSED,
        };
        let request = Request::SceneContact(SceneContactRequest::ActivateProductGroup(
            ProductGroupActivationRequest {
                session_id,
                command_originator: 1,
                priority_level: 3,
                product_group_id: group_id.get(),
                parameter_id: 0,
                position: StandardParameter::Relative(position),
                velocity: group.velocity,
                priority_level_lock: false,
                priority_level_settings: [3; 8],
                lock_time: 0,
            },
        ));
        let gateway = match self.gateway() {
            Ok(gateway) => Arc::clone(gateway),
            Err(error) => {
                self.session_ids.release(session_id);
                return Err(error);
            }
        };
        match gateway.send(request).await {
            Ok(Response::ProductGroupResult(result)) if result.session_id == session_id && result.status == 0 => Ok(()),
            Ok(Response::ProductGroupResult(result)) => {
                self.session_ids.release(session_id);
                bail!(
                    "KLF rejected product-group session {session_id} with status {}",
                    result.status
                );
            }
            Ok(response) => {
                self.session_ids.release(session_id);
                bail!("unexpected product-group response: {response:?}");
            }
            Err(error) => {
                self.session_ids.release(session_id);
                Err(error.into())
            }
        }
    }

    async fn heartbeat(&mut self) {
        let Some(gateway) = self.gateway.clone() else {
            return;
        };
        if let Err(error) = gateway.gateway_state().await {
            self.gateway_failed(format!("heartbeat failed: {error}")).await;
        }
    }

    async fn refresh_stationary_nodes(&mut self) {
        let node_ids = self
            .cache
            .iter()
            .filter(|(_, node)| {
                node.is_controllable() && !matches!(node.state(), CoverState::Opening | CoverState::Closing)
            })
            .map(|(node_id, _)| *node_id)
            .collect::<Vec<_>>();
        for node_id in node_ids {
            if let Err(error) = self.request_status(node_id).await {
                log::warn!("status refresh for node {} failed: {error:#}", node_id.get());
            }
        }
    }

    async fn request_status(&mut self, node_id: NodeId) -> Result<()> {
        let session_id = self.session_ids.allocate()?;
        let request = Request::StatusRequest {
            session_id,
            target: CommandTarget::new([node_id])?,
            status_type: StatusRequestType::MainInformation,
        };
        let gateway = Arc::clone(self.gateway()?);
        match gateway.send(request).await {
            Ok(Response::StatusAccepted { status: 1, .. }) => Ok(()),
            Ok(Response::StatusAccepted { status, .. }) => {
                self.session_ids.release(session_id);
                bail!("KLF rejected status session {session_id} with status {status}");
            }
            Ok(response) => {
                self.session_ids.release(session_id);
                bail!("unexpected status response: {response:?}");
            }
            Err(error) => {
                self.session_ids.release(session_id);
                Err(error.into())
            }
        }
    }

    async fn handle_gateway_event(&mut self, event: Result<ConnectionEvent, broadcast::error::RecvError>) {
        match event {
            Ok(ConnectionEvent::Notification(response)) => self.handle_response(response).await,
            Ok(ConnectionEvent::UnexpectedResponse(response)) => {
                log::warn!("unexpected KLF response: {response:?}");
            }
            Ok(ConnectionEvent::SessionTimedOut(session_id)) => {
                self.session_ids.release(session_id);
                log::warn!("KLF session {session_id} timed out");
            }
            Ok(ConnectionEvent::ProtocolError(message)) => log::warn!("KLF protocol error: {message}"),
            Ok(ConnectionEvent::Disconnected(message)) => self.gateway_failed(message).await,
            Err(broadcast::error::RecvError::Lagged(count)) => {
                self.gateway_failed(format!("KLF event receiver lagged by {count} messages"))
                    .await;
            }
            Err(broadcast::error::RecvError::Closed) => self.gateway_failed("KLF event stream closed".to_owned()).await,
        }
    }

    async fn handle_response(&mut self, response: Response) {
        match &response {
            Response::SessionFinished { session_id } => {
                self.session_ids.release(*session_id);
            }
            Response::CommandRunStatus(status) if matches!(status.run_status, crate::klf200::RunStatus::Failed) => {
                log::error!(
                    "KLF command session {} failed for node {} with reply {} and information code {}",
                    status.session_id,
                    status.node_id.get(),
                    status.status_reply,
                    status.information_code
                );
            }
            _ => {}
        }

        if let Err(error) = self.apply_group_response(&response).await {
            log::error!("failed to apply KLF group update: {error:#}");
        }

        let node_added = matches!(
            &response,
            Response::NodeInformation(information) if self.cache.get(information.node_id).is_none()
        );
        let metadata_changed = matches!(
            response,
            Response::NodeInformation(_) | Response::NodeInformationChanged(_)
        );
        if let Some(node_id) = self.cache.apply_response(&response) {
            if node_added
                && self.mqtt_connected
                && let Err(error) = self.publish_node_inventory().await
            {
                log::error!("failed to publish node inventory: {error:#}");
            }
            if metadata_changed && let Err(error) = self.publish_node_info(node_id).await {
                log::error!("failed to publish metadata for node {}: {error:#}", node_id.get());
            }
            if self.cache.get(node_id).is_some_and(VeluxNode::is_controllable)
                && let Err(error) = self.publish_node_status(node_id).await
            {
                log::error!("failed to publish status for node {}: {error:#}", node_id.get());
            }
        }
    }

    async fn apply_group_response(&mut self, response: &Response) -> Result<()> {
        let update = match response {
            Response::GroupInformation(notification) => Some(Some(notification.information.clone())),
            Response::GroupChange(change) if change.change_type == 1 => change.information.clone().map(Some),
            Response::GroupChange(change) if change.change_type == 0 => {
                self.groups.get(&change.group_id).cloned().map(|_| None)
            }
            Response::GroupDeleted(group_id) => self.groups.get(group_id).cloned().map(|_| None),
            _ => return Ok(()),
        };

        match update {
            Some(Some(group)) => {
                self.groups.insert(group.group_id, group.clone());
                if self.mqtt_connected {
                    self.publish_group_inventory().await?;
                    self.publish_group_info(&group).await?;
                }
            }
            Some(None) => {
                let group_id = match response {
                    Response::GroupChange(change) => change.group_id,
                    Response::GroupDeleted(group_id) => *group_id,
                    _ => return Ok(()),
                };
                if let Some(group) = self.groups.remove(&group_id)
                    && self.mqtt_connected
                {
                    self.publish_group_inventory().await?;
                    self.clear_removed_groups(&[group]).await?;
                }
            }
            None => {}
        }
        Ok(())
    }

    async fn gateway_failed(&mut self, reason: String) {
        if self.gateway.take().is_none() {
            return;
        }
        self.gateway_events = None;
        self.cache.set_all_available(false);
        self.session_ids = SessionIdAllocator::new();
        let delay = self.reconnect.next_delay();
        self.next_reconnect = Instant::now() + delay;
        log::error!("KLF connection lost: {reason}; reconnecting in {delay:?}");
        self.publish_availability(false).await;
    }

    async fn publish_complete_snapshot(&mut self) -> Result<()> {
        self.mqtt.publish(self.topics.availability_publication(false)).await?;
        self.publish_node_inventory().await?;
        self.publish_group_inventory().await?;
        let nodes = self.cache.iter().map(|(_, node)| node.clone()).collect::<Vec<_>>();
        for node in nodes {
            self.mqtt.publish(self.topics.info_publication(&node)?).await?;
            if node.is_controllable() {
                self.mqtt.publish_all(self.topics.status_publications(&node)?).await?;
                if let Some(discovery) = &self.discovery
                    && let Some(publication) = discovery.publication(&self.topics, &node)?
                {
                    self.mqtt.publish(publication).await?;
                }
            }
        }
        let groups = self.groups.values().cloned().collect::<Vec<_>>();
        for group in groups {
            self.publish_group_info(&group).await?;
        }
        self.mqtt.publish(self.topics.availability_publication(true)).await?;
        Ok(())
    }

    async fn publish_group_inventory(&mut self) -> Result<()> {
        self.mqtt
            .publish(self.topics.groups_publication(self.groups.values())?)
            .await?;
        Ok(())
    }

    async fn publish_node_inventory(&mut self) -> Result<()> {
        self.mqtt
            .publish(self.topics.inventory_publication(&self.cache)?)
            .await?;
        Ok(())
    }

    async fn publish_group_info(&mut self, group: &GroupInformation) -> Result<()> {
        self.mqtt.publish(self.topics.group_info_publication(group)?).await?;
        if let Some(discovery) = &self.discovery {
            self.mqtt
                .publish(discovery.group_publication(&self.topics, group)?)
                .await?;
        }
        Ok(())
    }

    async fn publish_node_info(&mut self, node_id: NodeId) -> Result<()> {
        if !self.mqtt_connected {
            return Ok(());
        }
        let node = self
            .cache
            .get(node_id)
            .ok_or_else(|| anyhow!("node {} is not present", node_id.get()))?;
        self.mqtt.publish(self.topics.info_publication(node)?).await?;
        Ok(())
    }

    async fn publish_node_status(&mut self, node_id: NodeId) -> Result<()> {
        if !self.mqtt_connected {
            return Ok(());
        }
        let node = self
            .cache
            .get(node_id)
            .ok_or_else(|| anyhow!("node {} is not present", node_id.get()))?;
        self.mqtt.publish_all(self.topics.status_publications(node)?).await?;
        Ok(())
    }

    async fn clear_removed_nodes(&mut self, removed: &[VeluxNode]) -> Result<()> {
        for node in removed {
            self.mqtt
                .publish_all(self.topics.clear_node_publications(node.id))
                .await?;
            if let Some(discovery) = &self.discovery
                && let Some(publication) = discovery.clear_publication(node)
            {
                self.mqtt.publish(publication).await?;
            }
        }
        Ok(())
    }

    async fn clear_removed_groups(&mut self, removed: &[GroupInformation]) -> Result<()> {
        for group in removed {
            self.mqtt
                .publish_all(self.topics.clear_group_publications(group.group_id))
                .await?;
            if let Some(discovery) = &self.discovery {
                self.mqtt.publish(discovery.clear_group_publication(group)).await?;
            }
        }
        Ok(())
    }

    async fn publish_availability(&mut self, online: bool) {
        if self.mqtt_connected
            && let Err(error) = self.mqtt.publish(self.topics.availability_publication(online)).await
        {
            log::error!("failed to publish MQTT availability: {error}");
        }
    }

    fn gateway(&self) -> Result<&Arc<dyn Gateway>> {
        self.gateway.as_ref().ok_or_else(|| anyhow!("KLF gateway is offline"))
    }

    async fn shutdown(&mut self) -> Result<()> {
        log::info!("shutting down");
        if let Some(setup) = self.setup.take()
            && let Some(prepared) = setup.cancel().await
            && let Err(error) = prepared.gateway.shutdown(self.config.reboot_on_shutdown).await
        {
            log::warn!("KLF shutdown after completed setup failed: {error}");
        }
        self.publish_availability(false).await;
        if let Some(gateway) = self.gateway.take() {
            match gateway.shutdown(self.config.reboot_on_shutdown).await {
                Ok(()) if self.config.reboot_on_shutdown => {
                    log::info!("KLF house monitoring disabled, gateway reboot requested, and connection closed");
                }
                Ok(()) => log::info!("KLF house monitoring disabled and connection closed"),
                Err(error) => {
                    log::warn!("KLF shutdown was not confirmed: {error}; the gateway may require a power cycle");
                }
            }
        }
        if self.mqtt_connected {
            self.mqtt.disconnect().await?;
        }
        log::info!("shutdown complete");
        Ok(())
    }
}

#[cfg(unix)]
async fn shutdown_signal() -> Result<()> {
    use tokio::signal::unix::{SignalKind, signal};

    let mut terminate = signal(SignalKind::terminate()).context("failed to listen for SIGTERM")?;
    tokio::select! {
        result = tokio::signal::ctrl_c() => result.context("failed to listen for Ctrl+C"),
        _ = terminate.recv() => Ok(()),
    }
}

#[cfg(not(unix))]
async fn shutdown_signal() -> Result<()> {
    tokio::signal::ctrl_c().await.context("failed to listen for Ctrl+C")
}

fn configured_interval(duration: Duration) -> Interval {
    let mut timer = interval_at(Instant::now() + duration, duration);
    timer.set_missed_tick_behavior(MissedTickBehavior::Skip);
    timer
}

async fn wait_for_reconnect(deadline: Instant) {
    sleep_until(deadline).await;
}

async fn wait_for_setup(setup: &mut Option<GatewaySetup>) -> Result<PreparedGateway> {
    match setup {
        Some(setup) => (&mut setup.task).await.context("KLF setup task failed")?,
        None => pending().await,
    }
}

async fn receive_gateway_event(
    receiver: &mut Option<broadcast::Receiver<ConnectionEvent>>,
) -> Result<ConnectionEvent, broadcast::error::RecvError> {
    match receiver {
        Some(receiver) => receiver.recv().await,
        None => pending().await,
    }
}

struct PreparedGateway {
    gateway: Arc<dyn Gateway>,
    events: broadcast::Receiver<ConnectionEvent>,
    cache: VeluxNodeCache,
    groups: BTreeMap<GroupId, GroupInformation>,
    session_ids: SessionIdAllocator,
}

struct GatewaySetup {
    task: JoinHandle<Result<PreparedGateway>>,
    cancel: Option<oneshot::Sender<()>>,
}

impl GatewaySetup {
    async fn cancel(mut self) -> Option<PreparedGateway> {
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(());
        }
        match self.task.await {
            Ok(Ok(prepared)) => Some(prepared),
            Ok(Err(error)) => {
                log::debug!("KLF setup stopped during shutdown: {error:#}");
                None
            }
            Err(error) => {
                log::warn!("KLF setup task failed during shutdown: {error}");
                None
            }
        }
    }
}

async fn prepare_gateway(
    connector: Arc<dyn GatewayConnector>,
    config: Klf200Config,
    password: String,
    status_timeout: Duration,
    mut cancel: oneshot::Receiver<()>,
) -> Result<PreparedGateway> {
    let gateway = tokio::select! {
        result = connector.connect(config) => result.context("failed to connect to KLF")?,
        _ = &mut cancel => bail!("KLF setup cancelled before connecting"),
    };
    let setup_result = tokio::select! {
        result = initialize_gateway(Arc::clone(&gateway), password, status_timeout) => result,
        _ = &mut cancel => Err(anyhow!("KLF setup cancelled")),
    };

    match setup_result {
        Ok(initialized) => Ok(PreparedGateway {
            gateway,
            events: initialized.events,
            cache: initialized.cache,
            groups: initialized.groups,
            session_ids: initialized.session_ids,
        }),
        Err(error) => {
            if let Err(shutdown_error) = gateway.shutdown(false).await {
                log::warn!("failed to close KLF after incomplete setup: {shutdown_error}");
            }
            Err(error)
        }
    }
}

struct InitializedGateway {
    events: broadcast::Receiver<ConnectionEvent>,
    cache: VeluxNodeCache,
    groups: BTreeMap<GroupId, GroupInformation>,
    session_ids: SessionIdAllocator,
}

async fn initialize_gateway(
    gateway: Arc<dyn Gateway>,
    password: String,
    status_timeout: Duration,
) -> Result<InitializedGateway> {
    gateway.login(password).await.context("KLF login failed")?;
    let version = gateway.version().await.context("failed to read KLF version")?;
    let protocol = gateway
        .protocol_version()
        .await
        .context("failed to read KLF protocol version")?;
    log::info!(
        "KLF connected: hardware {}, product {}:{}, protocol {}.{}",
        version.hardware,
        version.product_group,
        version.product_type,
        protocol.major,
        protocol.minor
    );
    gateway
        .send(Request::HouseStatusMonitorDisable)
        .await
        .context("failed to disable stale house monitoring")?;
    let information = gateway.discover_nodes().await.context("KLF node discovery failed")?;
    let mut cache = VeluxNodeCache::new();
    cache.reconcile(information);
    let groups = gateway
        .discover_groups()
        .await
        .context("KLF group discovery failed")?
        .into_iter()
        .map(|group| (group.group_id, group))
        .collect::<BTreeMap<_, _>>();
    let group_members = groups
        .values()
        .flat_map(|group| group.actuators.iter())
        .filter_map(|node_id| u8::try_from(node_id).ok())
        .map(NodeId::new)
        .collect::<BTreeSet<_>>();
    for node_id in group_members {
        if cache.get(node_id).is_some() {
            continue;
        }
        log::warn!(
            "KLF group metadata references node {} missing from all-node discovery; requesting it directly",
            node_id.get()
        );
        match gateway.node_information(node_id).await {
            Ok(information) => {
                cache.apply_response(&Response::NodeInformation(information));
            }
            Err(error) => {
                log::warn!("failed to recover group member node {}: {error}", node_id.get());
            }
        }
    }
    let mut events = gateway.subscribe();
    let mut session_ids = SessionIdAllocator::new();
    let node_ids = cache
        .iter()
        .filter(|(_, node)| node.is_controllable())
        .map(|(node_id, _)| *node_id)
        .collect::<Vec<_>>();
    for node_id in node_ids {
        if let Some(node) = cache.get_mut(node_id) {
            node.mark_status_unknown();
        }
        if let Err(error) = query_initial_status(
            gateway.as_ref(),
            &mut events,
            &mut cache,
            &mut session_ids,
            node_id,
            status_timeout,
        )
        .await
        {
            log::warn!("initial status for node {} is unknown: {error:#}", node_id.get());
        }
    }
    gateway
        .send(Request::HouseStatusMonitorEnable)
        .await
        .context("failed to enable house monitoring")?;
    Ok(InitializedGateway {
        events,
        cache,
        groups,
        session_ids,
    })
}

async fn query_initial_status(
    gateway: &dyn Gateway,
    events: &mut broadcast::Receiver<ConnectionEvent>,
    cache: &mut VeluxNodeCache,
    session_ids: &mut SessionIdAllocator,
    node_id: NodeId,
    response_timeout: Duration,
) -> Result<()> {
    let session_id = session_ids.allocate()?;
    let request = Request::StatusRequest {
        session_id,
        target: CommandTarget::new([node_id])?,
        status_type: StatusRequestType::MainInformation,
    };
    match gateway.send(request).await {
        Ok(Response::StatusAccepted { status: 1, .. }) => {}
        Ok(Response::StatusAccepted { status, .. }) => {
            session_ids.release(session_id);
            bail!("KLF rejected status session {session_id} with status {status}");
        }
        Ok(response) => {
            session_ids.release(session_id);
            bail!("unexpected initial status response: {response:?}");
        }
        Err(error) => {
            session_ids.release(session_id);
            return Err(error.into());
        }
    }

    timeout(response_timeout, async {
        loop {
            match events.recv().await {
                Ok(ConnectionEvent::Notification(response)) => {
                    let complete = matches!(
                        &response,
                        Response::StatusNotification(status)
                            if status.session_id == session_id && status.node_id == node_id
                    );
                    if let Response::SessionFinished { session_id } = &response {
                        session_ids.release(*session_id);
                    }
                    cache.apply_response(&response);
                    if complete {
                        return Ok(());
                    }
                }
                Ok(ConnectionEvent::SessionTimedOut(expired)) => {
                    session_ids.release(expired);
                    if expired == session_id {
                        bail!("status session {session_id} timed out");
                    }
                }
                Ok(ConnectionEvent::Disconnected(message)) => bail!("KLF disconnected: {message}"),
                Ok(_) => {}
                Err(error) => return Err(error.into()),
            }
        }
    })
    .await
    .with_context(|| format!("status notification for node {} timed out", node_id.get()))?
}

struct ReconnectBackoff {
    minimum: Duration,
    maximum: Duration,
    current: Duration,
}

impl ReconnectBackoff {
    fn new(minimum: Duration, maximum: Duration) -> Self {
        Self {
            minimum,
            maximum,
            current: minimum,
        }
    }

    fn next_delay(&mut self) -> Duration {
        let base = self.current;
        self.current = self.current.saturating_mul(2).min(self.maximum);
        let jitter_limit = u64::try_from(base.as_millis() / 4).unwrap_or(u64::MAX);
        base.saturating_add(Duration::from_millis(fastrand::u64(0..=jitter_limit)))
    }

    fn reset(&mut self) {
        self.current = self.minimum;
    }
}

#[async_trait]
trait Broker: Send {
    async fn poll(&mut self) -> Result<MqttEvent, MqttError>;
    async fn publish(&mut self, publication: MqttPublication) -> Result<(), MqttError>;
    async fn publish_all(&mut self, publications: Vec<MqttPublication>) -> Result<(), MqttError>;
    async fn disconnect(&mut self) -> Result<(), MqttError>;
}

#[async_trait]
impl Broker for MqttConnection {
    async fn poll(&mut self) -> Result<MqttEvent, MqttError> {
        MqttConnection::poll(self).await
    }

    async fn publish(&mut self, publication: MqttPublication) -> Result<(), MqttError> {
        MqttConnection::publish(self, publication).await
    }

    async fn publish_all(&mut self, publications: Vec<MqttPublication>) -> Result<(), MqttError> {
        MqttConnection::publish_all(self, publications).await
    }

    async fn disconnect(&mut self) -> Result<(), MqttError> {
        MqttConnection::disconnect(self).await
    }
}

#[async_trait]
trait GatewayConnector: Send + Sync {
    async fn connect(&self, config: Klf200Config) -> std::result::Result<Arc<dyn Gateway>, KlfError>;
}

struct TlsGatewayConnector;

#[async_trait]
impl GatewayConnector for TlsGatewayConnector {
    async fn connect(&self, config: Klf200Config) -> std::result::Result<Arc<dyn Gateway>, KlfError> {
        Klf200Client::connect(config)
            .await
            .map(|client| Arc::new(client) as Arc<dyn Gateway>)
    }
}

#[async_trait]
trait Gateway: Send + Sync {
    fn subscribe(&self) -> broadcast::Receiver<ConnectionEvent>;
    async fn login(&self, password: String) -> std::result::Result<(), KlfError>;
    async fn version(&self) -> std::result::Result<Version, KlfError>;
    async fn protocol_version(&self) -> std::result::Result<ProtocolVersion, KlfError>;
    async fn gateway_state(&self) -> std::result::Result<GatewayState, KlfError>;
    async fn discover_nodes(&self) -> std::result::Result<Vec<NodeInformation>, KlfError>;
    async fn node_information(&self, node_id: NodeId) -> std::result::Result<NodeInformation, KlfError>;
    async fn discover_groups(&self) -> std::result::Result<Vec<GroupInformation>, KlfError>;
    async fn send(&self, request: Request) -> std::result::Result<Response, KlfError>;
    async fn shutdown(&self, reboot: bool) -> std::result::Result<(), KlfError>;
}

#[async_trait]
impl Gateway for Klf200Client {
    fn subscribe(&self) -> broadcast::Receiver<ConnectionEvent> {
        Klf200Client::subscribe(self)
    }

    async fn login(&self, password: String) -> std::result::Result<(), KlfError> {
        Klf200Client::login(self, password).await
    }

    async fn version(&self) -> std::result::Result<Version, KlfError> {
        Klf200Client::version(self).await
    }

    async fn protocol_version(&self) -> std::result::Result<ProtocolVersion, KlfError> {
        Klf200Client::protocol_version(self).await
    }

    async fn gateway_state(&self) -> std::result::Result<GatewayState, KlfError> {
        Klf200Client::gateway_state(self).await
    }

    async fn discover_nodes(&self) -> std::result::Result<Vec<NodeInformation>, KlfError> {
        Klf200Client::discover_nodes(self).await
    }

    async fn node_information(&self, node_id: NodeId) -> std::result::Result<NodeInformation, KlfError> {
        Klf200Client::node_information(self, node_id).await
    }

    async fn discover_groups(&self) -> std::result::Result<Vec<GroupInformation>, KlfError> {
        Klf200Client::discover_groups(self).await
    }

    async fn send(&self, request: Request) -> std::result::Result<Response, KlfError> {
        Klf200Client::send(self, request).await
    }

    async fn shutdown(&self, reboot: bool) -> std::result::Result<(), KlfError> {
        if reboot {
            Klf200Client::shutdown_and_reboot(self).await
        } else {
            Klf200Client::shutdown(self).await
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Mutex as StdMutex, MutexGuard};

    use rumqttc::Outgoing;
    use rumqttc::v5::{
        AsyncClient as ObserverClient, Event as ObserverEvent, EventLoop as ObserverEventLoop,
        MqttOptions as ObserverOptions,
        mqttbytes::{QoS, v5::Packet as ObserverPacket},
    };
    use serde_json::Value;
    use tokio::sync::{mpsc, oneshot};
    use tokio::time::sleep;

    use super::*;
    use crate::klf200::{
        ActuatorSet, Alias, CommandId, OperatingState, Percentage, ProtocolTimestamp, RunStatus, StandardParameter,
        StatusNotification, StatusNotificationDetail,
    };

    #[test]
    fn rejects_zero_reboot_reconnect_delay() {
        let mut config = VeluxMqttBridgeConfig::new(Klf200Config::new("unused"), "password", MqttConfig::new("unused"));
        config.reboot_reconnect_delay = Duration::ZERO;
        assert!(config.validate().is_err());
    }

    #[tokio::test]
    async fn publishes_initial_state_handles_commands_and_reconnects() {
        let publications = Arc::new(StdMutex::new(Vec::new()));
        let disconnected = Arc::new(AtomicBool::new(false));
        let (mqtt_tx, mqtt_rx) = mpsc::unbounded_channel();
        let mqtt = FakeBroker {
            incoming: mqtt_rx,
            publications: Arc::clone(&publications),
            disconnected: Arc::clone(&disconnected),
        };
        mqtt_tx
            .send(Ok(MqttEvent::Connected))
            .expect("bridge receives MQTT event");

        let first = Arc::new(FakeGateway::with_empty_node_discovery(
            vec![node_information(7, "Office", 2), node_information(8, "Lamp", 6)],
            vec![group_information(4, "Upstairs", &[7, 8])],
        ));
        let second = Arc::new(FakeGateway::new(vec![node_information(9, "Bedroom", 2)]));
        let connector = Arc::new(FakeConnector::new([
            Arc::clone(&first) as Arc<dyn Gateway>,
            Arc::clone(&second) as Arc<dyn Gateway>,
        ]));
        let bridge = test_bridge(mqtt, Arc::clone(&connector) as Arc<dyn GatewayConnector>, true);
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let task = tokio::spawn(bridge.run_until(async { shutdown_rx.await.context("shutdown sender dropped") }));

        wait_until(|| payload_count(&publications, "velux/state", b"online") >= 1).await;
        assert!(has_topic(&publications, "velux/nodes"));
        assert_eq!(latest_json(&publications, "velux/nodes"), serde_json::json!([7, 8]));
        assert!(has_topic(&publications, "velux/velux_node_7/info"));
        assert!(has_topic(&publications, "velux/velux_node_7/status"));
        assert!(has_topic(
            &publications,
            "homeassistant/cover/velux_0001020304050607/config"
        ));
        exercise_group_support(&mqtt_tx, &first, &publications).await;
        assert!(!has_topic(
            &publications,
            "homeassistant/cover/velux_0001020304050608/config"
        ));

        mqtt_tx
            .send(Ok(command_message(7, "CLOSE", true)))
            .expect("bridge receives retained command");
        mqtt_tx
            .send(Ok(command_message(7, "OPEN", false)))
            .expect("bridge receives open command");
        mqtt_tx
            .send(Ok(command_message(7, "CLOSE", false)))
            .expect("bridge receives close command");
        mqtt_tx
            .send(Ok(command_message(7, "TOGGLE", false)))
            .expect("bridge receives toggle command");
        wait_until(|| first.command_parameters().len() == 3).await;
        assert_eq!(
            first.command_parameters(),
            [
                Percentage::FULLY_OPEN.raw(),
                Percentage::FULLY_CLOSED.raw(),
                Percentage::FULLY_OPEN.raw()
            ]
        );

        first.notify(Response::NodeStatePosition(crate::klf200::NodeStatePosition {
            node_id: NodeId::new(7),
            operating_state: OperatingState::Done,
            current_position: StandardParameter::Relative(Percentage::FULLY_CLOSED),
            target_position: StandardParameter::Relative(Percentage::FULLY_CLOSED),
            functional_positions: [StandardParameter::NoFeedback; 4],
            remaining_time: 0,
            timestamp: ProtocolTimestamp::from_unix_seconds(200),
        }));
        wait_until(|| payload_count(&publications, "velux/velux_node_7/state", b"closed") >= 1).await;

        mqtt_tx
            .send(Err(MqttError::Connection("test broker outage".to_owned())))
            .expect("bridge receives MQTT failure");
        mqtt_tx
            .send(Ok(MqttEvent::Connected))
            .expect("bridge reconnects to MQTT");
        wait_until(|| payload_count(&publications, "velux/state", b"online") >= 2).await;

        first.disconnect("test KLF outage");
        wait_until(|| connector.connect_count.load(Ordering::SeqCst) >= 2).await;
        wait_until(|| payload_count(&publications, "velux/state", b"online") >= 3).await;
        assert!(has_empty_topic(&publications, "velux/velux_node_7/status"));
        assert!(has_empty_topic(
            &publications,
            "homeassistant/cover/velux_0001020304050607/config"
        ));
        assert!(has_empty_topic(&publications, "velux/velux_group_4/info"));
        assert!(has_empty_topic(
            &publications,
            "homeassistant/cover/test_bridge_group_4/config"
        ));
        assert_eq!(latest_json(&publications, "velux/nodes"), serde_json::json!([9]));
        assert_eq!(latest_json(&publications, "velux/groups"), serde_json::json!([]));

        shutdown_tx.send(()).expect("bridge still running");
        task.await.expect("bridge task joins").expect("clean shutdown");
        assert!(disconnected.load(Ordering::SeqCst));
        assert!(second.shutdown.load(Ordering::SeqCst));
        assert_eq!(latest_payload(&publications, "velux/state"), b"offline");
    }

    #[tokio::test]
    async fn live_node_addition_republishes_inventory() {
        let publications = Arc::new(StdMutex::new(Vec::new()));
        let (mqtt_tx, mqtt_rx) = mpsc::unbounded_channel();
        let mqtt = FakeBroker {
            incoming: mqtt_rx,
            publications: Arc::clone(&publications),
            disconnected: Arc::new(AtomicBool::new(false)),
        };
        drop(mqtt_tx);
        let connector = Arc::new(FakeConnector::new([]));
        let mut bridge = test_bridge(mqtt, connector, false);
        bridge.mqtt_connected = true;

        bridge
            .handle_response(Response::NodeInformation(node_information(4, "New node", 2)))
            .await;

        assert_eq!(latest_json(&publications, "velux/nodes"), serde_json::json!([4]));
    }

    async fn exercise_group_support(
        mqtt_tx: &mpsc::UnboundedSender<Result<MqttEvent, MqttError>>,
        gateway: &FakeGateway,
        publications: &Arc<StdMutex<Vec<MqttPublication>>>,
    ) {
        assert_eq!(latest_json(publications, "velux/groups"), serde_json::json!([4]));
        assert!(has_topic(publications, "velux/velux_group_4/info"));
        assert!(has_topic(
            publications,
            "homeassistant/cover/test_bridge_group_4/config"
        ));

        for (payload, retained) in [("OPEN", true), ("TOGGLE", false), ("OPEN", false), ("CLOSE", false)] {
            mqtt_tx
                .send(Ok(group_command_message(4, payload, retained)))
                .expect("bridge receives group command");
        }
        wait_until(|| gateway.group_commands().len() == 2).await;
        assert_eq!(
            gateway.group_commands(),
            [
                (GroupId::new(4), Percentage::FULLY_OPEN.raw()),
                (GroupId::new(4), Percentage::FULLY_CLOSED.raw())
            ]
        );

        let temporary_group = group_information(5, "Temporary", &[7]);
        gateway.notify(Response::GroupChange(crate::klf200::GroupChange {
            change_type: 1,
            group_id: temporary_group.group_id,
            information: Some(temporary_group),
        }));
        wait_until(|| has_topic(publications, "velux/velux_group_5/info")).await;
        assert_eq!(latest_json(publications, "velux/groups"), serde_json::json!([4, 5]));
        gateway.notify(Response::GroupDeleted(GroupId::new(5)));
        wait_until(|| has_empty_topic(publications, "velux/velux_group_5/info")).await;
        assert_eq!(latest_json(publications, "velux/groups"), serde_json::json!([4]));
    }

    #[tokio::test]
    async fn publishes_unknown_before_online_when_initial_status_times_out() {
        let publications = Arc::new(StdMutex::new(Vec::new()));
        let disconnected = Arc::new(AtomicBool::new(false));
        let (mqtt_tx, mqtt_rx) = mpsc::unbounded_channel();
        let mqtt = FakeBroker {
            incoming: mqtt_rx,
            publications: Arc::clone(&publications),
            disconnected,
        };
        mqtt_tx
            .send(Ok(MqttEvent::Connected))
            .expect("bridge receives MQTT event");
        let gateway = Arc::new(FakeGateway::without_status(vec![node_information(2, "Unknown", 2)]));
        let connector = Arc::new(FakeConnector::new([gateway as Arc<dyn Gateway>]));
        let bridge = test_bridge(mqtt, connector, false);
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let task = tokio::spawn(bridge.run_until(async { shutdown_rx.await.context("shutdown sender dropped") }));

        wait_until(|| payload_count(&publications, "velux/state", b"online") >= 1).await;
        assert_eq!(latest_payload(&publications, "velux/velux_node_2/state"), b"unknown");
        assert_eq!(latest_payload(&publications, "velux/velux_node_2/position"), b"unknown");
        let status = latest_json(&publications, "velux/velux_node_2/status");
        assert_eq!(status["position"], Value::Null);
        assert_eq!(status["target"], Value::Null);

        shutdown_tx.send(()).expect("bridge still running");
        task.await.expect("bridge task joins").expect("clean shutdown");
    }

    #[tokio::test]
    async fn shutdown_cancels_incomplete_setup_through_gateway_shutdown() {
        let (mqtt_tx, mqtt_rx) = mpsc::unbounded_channel();
        let mqtt = FakeBroker {
            incoming: mqtt_rx,
            publications: Arc::new(StdMutex::new(Vec::new())),
            disconnected: Arc::new(AtomicBool::new(false)),
        };
        let gateway = Arc::new(FakeGateway::blocking_discovery());
        let connector = Arc::new(FakeConnector::new([Arc::clone(&gateway) as Arc<dyn Gateway>]));
        let bridge = test_bridge(mqtt, connector, false);
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let task = tokio::spawn(bridge.run_until(async { shutdown_rx.await.context("shutdown sender dropped") }));

        wait_until(|| gateway.discovery_started.load(Ordering::SeqCst)).await;
        shutdown_tx.send(()).expect("bridge still running");
        timeout(Duration::from_secs(1), task)
            .await
            .expect("bridge shutdown completes")
            .expect("bridge task joins")
            .expect("clean shutdown");
        assert!(gateway.shutdown.load(Ordering::SeqCst));
        assert!(!gateway.rebooted.load(Ordering::SeqCst));
        drop(mqtt_tx);
    }

    #[tokio::test]
    async fn configured_reboot_is_only_used_for_completed_service_shutdown() {
        let (mqtt_tx, mqtt_rx) = mpsc::unbounded_channel();
        let mqtt = FakeBroker {
            incoming: mqtt_rx,
            publications: Arc::new(StdMutex::new(Vec::new())),
            disconnected: Arc::new(AtomicBool::new(false)),
        };
        mqtt_tx
            .send(Ok(MqttEvent::Connected))
            .expect("bridge receives MQTT event");
        let gateway = Arc::new(FakeGateway::new(Vec::new()));
        let connector = Arc::new(FakeConnector::new([Arc::clone(&gateway) as Arc<dyn Gateway>]));
        let mut config = test_config(MqttConfig::new("unused"), false);
        config.reboot_on_shutdown = true;
        let bridge = test_bridge_with_config(mqtt, connector, config);
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let task = tokio::spawn(bridge.run_until(async { shutdown_rx.await.context("shutdown sender dropped") }));

        wait_until(|| gateway.status_nodes().is_empty() && !gateway.shutdown.load(Ordering::SeqCst)).await;
        wait_until(|| gateway.events.receiver_count() > 0).await;
        shutdown_tx.send(()).expect("bridge still running");
        task.await.expect("bridge task joins").expect("clean shutdown");
        assert!(gateway.shutdown.load(Ordering::SeqCst));
        assert!(gateway.rebooted.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn mqtt_reboot_rejects_retained_messages_and_reconnects_after_confirmation() {
        let publications = Arc::new(StdMutex::new(Vec::new()));
        let (_mqtt_tx, mqtt_rx) = mpsc::unbounded_channel();
        let mqtt = FakeBroker {
            incoming: mqtt_rx,
            publications: Arc::clone(&publications),
            disconnected: Arc::new(AtomicBool::new(false)),
        };
        let gateway = Arc::new(FakeGateway::new(Vec::new()));
        let connector = Arc::new(FakeConnector::new([]));
        let mut bridge = test_bridge(mqtt, connector, false);
        bridge.gateway = Some(Arc::clone(&gateway) as Arc<dyn Gateway>);
        bridge.mqtt_connected = true;

        bridge.handle_mqtt_message(reboot_message(true)).await;
        assert_eq!(gateway.reboot_requests.load(Ordering::SeqCst), 0);
        assert!(bridge.gateway.is_some());

        let reconnect_delay = bridge.config.reboot_reconnect_delay;
        let reboot_started = Instant::now();
        bridge.handle_mqtt_message(reboot_message(false)).await;
        assert_eq!(gateway.reboot_requests.load(Ordering::SeqCst), 1);
        assert!(gateway.shutdown.load(Ordering::SeqCst));
        assert!(gateway.rebooted.load(Ordering::SeqCst));
        assert!(bridge.gateway.is_none());
        assert_eq!(latest_payload(&publications, "velux/state"), b"offline");
        assert!(bridge.next_reconnect >= reboot_started + reconnect_delay);
    }

    #[tokio::test]
    async fn refreshes_stationary_nodes_without_polling_moving_covers() {
        let (mqtt_tx, mqtt_rx) = mpsc::unbounded_channel();
        let mqtt = FakeBroker {
            incoming: mqtt_rx,
            publications: Arc::new(StdMutex::new(Vec::new())),
            disconnected: Arc::new(AtomicBool::new(false)),
        };
        drop(mqtt_tx);
        let stationary = node_information(1, "Stationary", 2);
        let mut moving = node_information(2, "Moving", 2);
        moving.operating_state = OperatingState::Executing;
        moving.target_position = StandardParameter::Relative(Percentage::FULLY_CLOSED);
        let gateway = Arc::new(FakeGateway::new(vec![stationary.clone(), moving.clone()]));
        let connector = Arc::new(FakeConnector::new([]));
        let mut bridge = test_bridge(mqtt, connector, false);
        bridge.gateway = Some(Arc::clone(&gateway) as Arc<dyn Gateway>);
        bridge.cache.reconcile(vec![stationary, moving]);

        bridge.refresh_stationary_nodes().await;

        assert_eq!(gateway.status_nodes(), [NodeId::new(1)]);
    }

    #[tokio::test]
    #[ignore = "requires V2M_TEST_MQTT_PORT and a disposable local MQTT v5 broker"]
    async fn full_bridge_smoke_with_local_mqtt_broker() {
        let host = std::env::var("V2M_TEST_MQTT_HOST").unwrap_or_else(|_| "127.0.0.1".to_owned());
        let port = std::env::var("V2M_TEST_MQTT_PORT")
            .expect("V2M_TEST_MQTT_PORT must identify a disposable broker")
            .parse::<u16>()
            .expect("valid MQTT test port");
        let base_topic = format!("velux2mqtt_test_{}", std::process::id());
        let mut mqtt_config = MqttConfig::new(&host);
        mqtt_config.port = port;
        mqtt_config.client_id = format!("velux2mqtt-test-{}", std::process::id());
        mqtt_config.base_topic.clone_from(&base_topic);
        let mqtt = MqttConnection::new(mqtt_config.clone()).expect("valid MQTT test connection");

        let first = Arc::new(FakeGateway::with_groups(
            vec![node_information(3, "Broker test", 2)],
            vec![group_information(6, "Broker group", &[3])],
        ));
        let second = Arc::new(FakeGateway::with_groups(
            vec![node_information(3, "Broker test", 2)],
            vec![group_information(6, "Broker group", &[3])],
        ));
        let connector = Arc::new(FakeConnector::new([
            Arc::clone(&first) as Arc<dyn Gateway>,
            Arc::clone(&second) as Arc<dyn Gateway>,
        ]));
        let config = test_config(mqtt_config, false);
        let bridge = test_bridge_with_config(mqtt, connector, config);
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let bridge_task =
            tokio::spawn(bridge.run_until(async { shutdown_rx.await.context("shutdown sender dropped") }));

        let (observer, mut observer_loop) = observer(&host, port, &base_topic).await;
        wait_for_observer_payload(&mut observer_loop, &format!("{base_topic}/state"), b"online").await;
        observer
            .publish(
                format!("{base_topic}/velux_node_3/cmnd/control"),
                QoS::AtLeastOnce,
                false,
                "OPEN",
            )
            .await
            .expect("queue test command");
        wait_for_observer_outgoing_publish(&mut observer_loop).await;
        wait_until(|| first.command_parameters().len() == 1).await;
        assert_eq!(first.command_parameters(), [Percentage::FULLY_OPEN.raw()]);

        observer
            .publish(
                format!("{base_topic}/velux_group_6/cmnd/control"),
                QoS::AtLeastOnce,
                false,
                "CLOSE",
            )
            .await
            .expect("queue test group command");
        wait_for_observer_outgoing_publish(&mut observer_loop).await;
        wait_until(|| first.group_commands().len() == 1).await;
        assert_eq!(
            first.group_commands(),
            [(GroupId::new(6), Percentage::FULLY_CLOSED.raw())]
        );

        observer
            .publish(format!("{base_topic}/cmnd/reboot"), QoS::AtLeastOnce, false, "REBOOT")
            .await
            .expect("queue gateway reboot");
        wait_for_observer_outgoing_publish(&mut observer_loop).await;
        wait_until(|| first.reboot_requests.load(Ordering::SeqCst) == 1).await;
        wait_for_observer_payload(&mut observer_loop, &format!("{base_topic}/state"), b"offline").await;
        wait_for_observer_payload(&mut observer_loop, &format!("{base_topic}/state"), b"online").await;

        shutdown_tx.send(()).expect("bridge still running");
        let state_topic = format!("{base_topic}/state");
        let ((), bridge_result) = tokio::join!(
            wait_for_observer_payload(&mut observer_loop, &state_topic, b"offline"),
            bridge_task
        );
        bridge_result.expect("bridge task joins").expect("clean shutdown");
        assert!(second.shutdown.load(Ordering::SeqCst));
    }

    fn test_bridge(
        mqtt: impl Broker + 'static,
        connector: Arc<dyn GatewayConnector>,
        hass_discovery: bool,
    ) -> VeluxMqttBridge {
        let mut mqtt_config = MqttConfig::new("unused");
        mqtt_config.client_id = "test-bridge".to_owned();
        let config = test_config(mqtt_config, hass_discovery);
        test_bridge_with_config(mqtt, connector, config)
    }

    fn test_config(mqtt_config: MqttConfig, hass_discovery: bool) -> VeluxMqttBridgeConfig {
        let mut config = VeluxMqttBridgeConfig::new(Klf200Config::new("unused"), "password", mqtt_config);
        config.hass_discovery = hass_discovery;
        config.status_response_timeout = Duration::from_millis(10);
        config.reboot_reconnect_delay = Duration::from_millis(5);
        config.heartbeat_interval = Duration::from_mins(1);
        config.status_refresh_interval = Duration::from_mins(1);
        config.reconnect_min_delay = Duration::from_millis(1);
        config.reconnect_max_delay = Duration::from_millis(2);
        config.validate().expect("valid test configuration");
        config
    }

    fn test_bridge_with_config(
        mqtt: impl Broker + 'static,
        connector: Arc<dyn GatewayConnector>,
        config: VeluxMqttBridgeConfig,
    ) -> VeluxMqttBridge {
        VeluxMqttBridge {
            topics: TopicLayout::new(&config.mqtt.base_topic).expect("valid topics"),
            discovery: config
                .hass_discovery
                .then(|| HomeAssistantDiscovery::new(config.mqtt.client_id.clone())),
            reconnect: ReconnectBackoff::new(config.reconnect_min_delay, config.reconnect_max_delay),
            config,
            mqtt: Box::new(mqtt),
            connector,
            gateway: None,
            gateway_events: None,
            setup: None,
            cache: VeluxNodeCache::new(),
            groups: BTreeMap::new(),
            session_ids: SessionIdAllocator::new(),
            mqtt_connected: false,
            next_reconnect: Instant::now(),
        }
    }

    async fn observer(host: &str, port: u16, base_topic: &str) -> (ObserverClient, ObserverEventLoop) {
        let mut options = ObserverOptions::new(format!("observer-{}", std::process::id()), host, port);
        options.set_clean_start(true);
        options.set_keep_alive(Duration::from_secs(10));
        let (client, mut event_loop) = ObserverClient::new(options, 32);
        client
            .subscribe(format!("{base_topic}/#"), QoS::AtLeastOnce)
            .await
            .expect("queue observer subscription");
        timeout(Duration::from_secs(2), async {
            loop {
                if matches!(
                    event_loop.poll().await.expect("observer MQTT event"),
                    ObserverEvent::Incoming(ObserverPacket::SubAck(_))
                ) {
                    return;
                }
            }
        })
        .await
        .expect("observer subscribes");
        (client, event_loop)
    }

    async fn wait_for_observer_payload(event_loop: &mut ObserverEventLoop, topic: &str, payload: &[u8]) {
        timeout(Duration::from_secs(2), async {
            loop {
                if let ObserverEvent::Incoming(ObserverPacket::Publish(publication)) =
                    event_loop.poll().await.expect("observer MQTT event")
                    && publication.topic.as_ref() == topic.as_bytes()
                    && publication.payload.as_ref() == payload
                {
                    return;
                }
            }
        })
        .await
        .expect("observer receives expected publication");
    }

    async fn wait_for_observer_outgoing_publish(event_loop: &mut ObserverEventLoop) {
        timeout(Duration::from_secs(2), async {
            loop {
                if matches!(
                    event_loop.poll().await.expect("observer MQTT event"),
                    ObserverEvent::Outgoing(Outgoing::Publish(_))
                ) {
                    return;
                }
            }
        })
        .await
        .expect("observer publishes command");
    }

    fn command_message(node_id: u8, payload: &str, retained: bool) -> MqttEvent {
        MqttEvent::Message(IncomingMessage {
            topic: format!("velux/velux_node_{node_id}/cmnd/control"),
            payload: payload.to_owned(),
            retained,
        })
    }

    fn group_command_message(group_id: u8, payload: &str, retained: bool) -> MqttEvent {
        MqttEvent::Message(IncomingMessage {
            topic: format!("velux/velux_group_{group_id}/cmnd/control"),
            payload: payload.to_owned(),
            retained,
        })
    }

    fn reboot_message(retained: bool) -> IncomingMessage {
        IncomingMessage {
            topic: "velux/cmnd/reboot".to_owned(),
            payload: "REBOOT".to_owned(),
            retained,
        }
    }

    async fn wait_until(predicate: impl Fn() -> bool) {
        timeout(Duration::from_secs(2), async {
            while !predicate() {
                sleep(Duration::from_millis(2)).await;
            }
        })
        .await
        .expect("condition becomes true");
    }

    fn lock_publications(publications: &Arc<StdMutex<Vec<MqttPublication>>>) -> MutexGuard<'_, Vec<MqttPublication>> {
        publications.lock().expect("publication mutex")
    }

    fn has_topic(publications: &Arc<StdMutex<Vec<MqttPublication>>>, topic: &str) -> bool {
        lock_publications(publications)
            .iter()
            .any(|publication| publication.topic == topic && !publication.payload.is_empty())
    }

    fn has_empty_topic(publications: &Arc<StdMutex<Vec<MqttPublication>>>, topic: &str) -> bool {
        lock_publications(publications)
            .iter()
            .any(|publication| publication.topic == topic && publication.payload.is_empty())
    }

    fn payload_count(publications: &Arc<StdMutex<Vec<MqttPublication>>>, topic: &str, payload: &[u8]) -> usize {
        lock_publications(publications)
            .iter()
            .filter(|publication| publication.topic == topic && publication.payload.as_ref() == payload)
            .count()
    }

    fn latest_payload(publications: &Arc<StdMutex<Vec<MqttPublication>>>, topic: &str) -> Vec<u8> {
        lock_publications(publications)
            .iter()
            .rev()
            .find(|publication| publication.topic == topic)
            .expect("topic was published")
            .payload
            .to_vec()
    }

    fn latest_json(publications: &Arc<StdMutex<Vec<MqttPublication>>>, topic: &str) -> Value {
        serde_json::from_slice(&latest_payload(publications, topic)).expect("valid JSON publication")
    }

    struct FakeBroker {
        incoming: mpsc::UnboundedReceiver<Result<MqttEvent, MqttError>>,
        publications: Arc<StdMutex<Vec<MqttPublication>>>,
        disconnected: Arc<AtomicBool>,
    }

    #[async_trait]
    impl Broker for FakeBroker {
        async fn poll(&mut self) -> Result<MqttEvent, MqttError> {
            self.incoming
                .recv()
                .await
                .unwrap_or_else(|| Err(MqttError::Connection("test broker closed".to_owned())))
        }

        async fn publish(&mut self, publication: MqttPublication) -> Result<(), MqttError> {
            lock_publications(&self.publications).push(publication);
            Ok(())
        }

        async fn publish_all(&mut self, publications: Vec<MqttPublication>) -> Result<(), MqttError> {
            lock_publications(&self.publications).extend(publications);
            Ok(())
        }

        async fn disconnect(&mut self) -> Result<(), MqttError> {
            self.disconnected.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    struct FakeConnector {
        gateways: StdMutex<VecDeque<Arc<dyn Gateway>>>,
        connect_count: AtomicUsize,
    }

    impl FakeConnector {
        fn new(gateways: impl IntoIterator<Item = Arc<dyn Gateway>>) -> Self {
            Self {
                gateways: StdMutex::new(gateways.into_iter().collect()),
                connect_count: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl GatewayConnector for FakeConnector {
        async fn connect(&self, _config: Klf200Config) -> std::result::Result<Arc<dyn Gateway>, KlfError> {
            self.connect_count.fetch_add(1, Ordering::SeqCst);
            self.gateways
                .lock()
                .expect("gateway queue")
                .pop_front()
                .ok_or_else(|| KlfError::Io {
                    message: "no scripted gateway".to_owned(),
                })
        }
    }

    struct FakeGateway {
        events: broadcast::Sender<ConnectionEvent>,
        nodes: Vec<NodeInformation>,
        groups: Vec<GroupInformation>,
        command_parameters: StdMutex<Vec<u16>>,
        group_commands: StdMutex<Vec<(GroupId, u16)>>,
        status_nodes: StdMutex<Vec<NodeId>>,
        publish_status: bool,
        block_discovery: bool,
        empty_node_discovery: bool,
        discovery_started: AtomicBool,
        shutdown: AtomicBool,
        rebooted: AtomicBool,
        reboot_requests: AtomicUsize,
    }

    impl FakeGateway {
        fn new(nodes: Vec<NodeInformation>) -> Self {
            Self::with_groups(nodes, Vec::new())
        }

        fn with_groups(nodes: Vec<NodeInformation>, groups: Vec<GroupInformation>) -> Self {
            Self::with_status_and_groups(nodes, groups, true)
        }

        fn with_empty_node_discovery(nodes: Vec<NodeInformation>, groups: Vec<GroupInformation>) -> Self {
            let mut gateway = Self::with_status_and_groups(nodes, groups, true);
            gateway.empty_node_discovery = true;
            gateway
        }

        fn without_status(nodes: Vec<NodeInformation>) -> Self {
            Self::with_status_and_groups(nodes, Vec::new(), false)
        }

        fn blocking_discovery() -> Self {
            let mut gateway = Self::with_status_and_groups(Vec::new(), Vec::new(), true);
            gateway.block_discovery = true;
            gateway
        }

        fn with_status_and_groups(
            nodes: Vec<NodeInformation>,
            groups: Vec<GroupInformation>,
            publish_status: bool,
        ) -> Self {
            let (events, _) = broadcast::channel(64);
            Self {
                events,
                nodes,
                groups,
                command_parameters: StdMutex::new(Vec::new()),
                group_commands: StdMutex::new(Vec::new()),
                status_nodes: StdMutex::new(Vec::new()),
                publish_status,
                block_discovery: false,
                empty_node_discovery: false,
                discovery_started: AtomicBool::new(false),
                shutdown: AtomicBool::new(false),
                rebooted: AtomicBool::new(false),
                reboot_requests: AtomicUsize::new(0),
            }
        }

        fn command_parameters(&self) -> Vec<u16> {
            self.command_parameters.lock().expect("command parameters").clone()
        }

        fn status_nodes(&self) -> Vec<NodeId> {
            self.status_nodes.lock().expect("status nodes").clone()
        }

        fn group_commands(&self) -> Vec<(GroupId, u16)> {
            self.group_commands.lock().expect("group commands").clone()
        }

        fn notify(&self, response: Response) {
            let _ = self.events.send(ConnectionEvent::Notification(response));
        }

        fn disconnect(&self, message: &str) {
            let _ = self.events.send(ConnectionEvent::Disconnected(message.to_owned()));
        }
    }

    #[async_trait]
    impl Gateway for FakeGateway {
        fn subscribe(&self) -> broadcast::Receiver<ConnectionEvent> {
            self.events.subscribe()
        }

        async fn login(&self, _password: String) -> std::result::Result<(), KlfError> {
            Ok(())
        }

        async fn version(&self) -> std::result::Result<Version, KlfError> {
            Ok(Version {
                software: [1, 2, 3, 4, 5, 6],
                hardware: 1,
                product_group: 2,
                product_type: 3,
            })
        }

        async fn protocol_version(&self) -> std::result::Result<ProtocolVersion, KlfError> {
            Ok(ProtocolVersion { major: 3, minor: 18 })
        }

        async fn gateway_state(&self) -> std::result::Result<GatewayState, KlfError> {
            Ok(GatewayState {
                state: 0,
                sub_state: 0,
                data: [0; 4],
            })
        }

        async fn discover_nodes(&self) -> std::result::Result<Vec<NodeInformation>, KlfError> {
            if self.block_discovery {
                self.discovery_started.store(true, Ordering::SeqCst);
                return std::future::pending().await;
            }
            if self.empty_node_discovery {
                Ok(Vec::new())
            } else {
                Ok(self.nodes.clone())
            }
        }

        async fn node_information(&self, node_id: NodeId) -> std::result::Result<NodeInformation, KlfError> {
            self.nodes
                .iter()
                .find(|information| information.node_id == node_id)
                .cloned()
                .ok_or(KlfError::InvalidRequest {
                    message: "test node is not present",
                })
        }

        async fn discover_groups(&self) -> std::result::Result<Vec<GroupInformation>, KlfError> {
            Ok(self.groups.clone())
        }

        async fn send(&self, request: Request) -> std::result::Result<Response, KlfError> {
            match request {
                Request::HouseStatusMonitorEnable | Request::HouseStatusMonitorDisable => {
                    Ok(Response::Acknowledgement {
                        command: CommandId::GW_HOUSE_STATUS_MONITOR_ENABLE_CFM,
                    })
                }
                Request::StatusRequest { session_id, target, .. } => {
                    self.status_nodes
                        .lock()
                        .expect("status nodes")
                        .extend_from_slice(target.nodes());
                    if self.publish_status {
                        let node_id = target.nodes()[0];
                        let node = self
                            .nodes
                            .iter()
                            .find(|node| node.node_id == node_id)
                            .expect("status node exists");
                        self.notify(Response::StatusNotification(StatusNotification {
                            session_id,
                            status_id: 1,
                            node_id,
                            run_status: RunStatus::Completed,
                            status_reply: 0,
                            detail: StatusNotificationDetail::Main {
                                target_position: node.target_position,
                                current_position: node.current_position,
                                remaining_time: node.remaining_time,
                                last_master_execution_address: 0,
                                last_command_originator: 1,
                            },
                        }));
                        self.notify(Response::SessionFinished { session_id });
                    }
                    Ok(Response::StatusAccepted { session_id, status: 1 })
                }
                Request::CommandSend { command, .. } => {
                    self.command_parameters
                        .lock()
                        .expect("command parameters")
                        .push(command.main_parameter.to_raw().get());
                    self.notify(Response::SessionFinished {
                        session_id: command.session_id,
                    });
                    Ok(Response::CommandAccepted {
                        session_id: command.session_id,
                        status: 1,
                    })
                }
                Request::SceneContact(SceneContactRequest::ActivateProductGroup(request)) => {
                    self.group_commands
                        .lock()
                        .expect("group commands")
                        .push((GroupId::new(request.product_group_id), request.position.to_raw().get()));
                    self.notify(Response::SessionFinished {
                        session_id: request.session_id,
                    });
                    Ok(Response::ProductGroupResult(crate::klf200::ProductGroupResult {
                        session_id: request.session_id,
                        status: 0,
                    }))
                }
                Request::Reboot => {
                    self.reboot_requests.fetch_add(1, Ordering::SeqCst);
                    Ok(Response::Acknowledgement {
                        command: CommandId::GW_REBOOT_CFM,
                    })
                }
                request => Err(KlfError::UnsupportedRequest {
                    command: request.command_id(),
                }),
            }
        }

        async fn shutdown(&self, reboot: bool) -> std::result::Result<(), KlfError> {
            self.shutdown.store(true, Ordering::SeqCst);
            self.rebooted.store(reboot, Ordering::SeqCst);
            if reboot {
                self.reboot_requests.fetch_add(1, Ordering::SeqCst);
            }
            Ok(())
        }
    }

    fn node_information(node_id: u8, name: &str, actuator_code: u16) -> NodeInformation {
        NodeInformation {
            node_id: NodeId::new(node_id),
            order: u16::from(node_id),
            placement: 1,
            name: name.to_owned(),
            velocity: 0,
            node_type_sub_type: actuator_code << 6,
            product_group: 1,
            product_type: 2,
            variation: 0,
            power_mode: 0,
            build_number: 1,
            serial_number: [0, 1, 2, 3, 4, 5, 6, node_id],
            operating_state: OperatingState::Done,
            current_position: StandardParameter::Relative(Percentage::from_percent(25)),
            target_position: StandardParameter::Relative(Percentage::from_percent(25)),
            functional_positions: [StandardParameter::NoFeedback; 4],
            remaining_time: 0,
            timestamp: ProtocolTimestamp::from_unix_seconds(100),
            aliases: vec![Alias { kind: 1, value: 2 }],
        }
    }

    fn group_information(group_id: u8, name: &str, node_ids: &[usize]) -> GroupInformation {
        let mut actuators = ActuatorSet::new();
        for node_id in node_ids {
            assert!(actuators.insert(*node_id));
        }
        GroupInformation {
            group_id: GroupId::new(group_id),
            order: u16::from(group_id),
            placement: 1,
            name: name.to_owned(),
            velocity: 0,
            node_variation: 0,
            group_type: 0,
            object_count: u8::try_from(node_ids.len()).expect("fixture fits in u8"),
            actuators,
            revision: 1,
        }
    }
}
