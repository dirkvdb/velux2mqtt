use std::fmt;
use std::time::Duration;

use bytes::Bytes;
use rumqttc::Outgoing;
use rumqttc::v5::{
    AsyncClient, Event, EventLoop, MqttOptions,
    mqttbytes::{
        QoS,
        v5::{ConnectReturnCode, LastWill, Packet},
    },
};
use serde::Serialize;
use thiserror::Error;
use tokio::time::timeout;

use crate::klf200::{GroupId, GroupInformation, NodeId, ProtocolTimestamp};
use crate::veluxnode::{CoverAction, CoverState, VeluxNode, VeluxNodeCache};

pub const ONLINE_PAYLOAD: &str = "online";
pub const OFFLINE_PAYLOAD: &str = "offline";

#[derive(Clone)]
pub struct MqttConfig {
    pub server: String,
    pub port: u16,
    pub client_id: String,
    pub user: String,
    pub password: String,
    pub base_topic: String,
    pub keep_alive: Duration,
    pub request_capacity: usize,
}

impl MqttConfig {
    #[must_use]
    pub fn new(server: impl Into<String>) -> Self {
        Self {
            server: server.into(),
            port: 1883,
            client_id: "velux2mqtt".to_owned(),
            user: String::new(),
            password: String::new(),
            base_topic: "velux".to_owned(),
            keep_alive: Duration::from_mins(1),
            request_capacity: 256,
        }
    }

    /// Validates and normalizes the configured base topic.
    ///
    /// # Errors
    ///
    /// Returns an error for empty client/server values or MQTT wildcard/NUL characters.
    pub fn validate(&mut self) -> Result<(), MqttError> {
        self.server = self.server.trim().to_owned();
        self.client_id = self.client_id.trim().to_owned();
        self.base_topic = self.base_topic.trim().trim_matches('/').to_owned();
        if self.server.is_empty() {
            return Err(MqttError::InvalidConfiguration("MQTT server cannot be empty"));
        }
        if self.client_id.is_empty() {
            return Err(MqttError::InvalidConfiguration("MQTT client ID cannot be empty"));
        }
        TopicLayout::new(&self.base_topic)?;
        if self.keep_alive.is_zero() {
            return Err(MqttError::InvalidConfiguration(
                "MQTT keep-alive must be greater than zero",
            ));
        }
        if self.request_capacity == 0 {
            return Err(MqttError::InvalidConfiguration(
                "MQTT request capacity must be greater than zero",
            ));
        }
        Ok(())
    }
}

impl fmt::Debug for MqttConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MqttConfig")
            .field("server", &self.server)
            .field("port", &self.port)
            .field("client_id", &self.client_id)
            .field("user_configured", &(!self.user.is_empty()))
            .field("password", &"[REDACTED]")
            .field("base_topic", &self.base_topic)
            .field("keep_alive", &self.keep_alive)
            .field("request_capacity", &self.request_capacity)
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IncomingMessage {
    pub topic: String,
    pub payload: String,
    pub retained: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MqttEvent {
    Connected,
    Message(IncomingMessage),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MqttPublication {
    pub topic: String,
    pub payload: Bytes,
    pub retained: bool,
}

impl MqttPublication {
    #[must_use]
    pub fn retained(topic: impl Into<String>, payload: impl Into<Bytes>) -> Self {
        Self {
            topic: topic.into(),
            payload: payload.into(),
            retained: true,
        }
    }

    #[must_use]
    pub fn clear(topic: impl Into<String>) -> Self {
        Self::retained(topic, Bytes::new())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NodeCommand {
    pub node_id: NodeId,
    pub action: CoverAction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GroupAction {
    Open,
    Close,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupCommand {
    pub group_id: GroupId,
    pub action: GroupAction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MqttCommand {
    Node(NodeCommand),
    Group(GroupCommand),
    GatewayReboot,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TopicLayout {
    base: String,
}

impl TopicLayout {
    /// Creates the canonical topic layout.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty topic, empty path segments, wildcards, or NUL bytes.
    pub fn new(base: impl AsRef<str>) -> Result<Self, MqttError> {
        let base = base.as_ref().trim().trim_matches('/');
        if base.is_empty()
            || base.split('/').any(str::is_empty)
            || base.bytes().any(|byte| matches!(byte, b'+' | b'#' | 0))
        {
            return Err(MqttError::InvalidBaseTopic { topic: base.to_owned() });
        }
        Ok(Self { base: base.to_owned() })
    }

    #[must_use]
    pub fn base(&self) -> &str {
        &self.base
    }

    #[must_use]
    pub fn state(&self) -> String {
        format!("{}/state", self.base)
    }

    #[must_use]
    pub fn nodes(&self) -> String {
        format!("{}/nodes", self.base)
    }

    #[must_use]
    pub fn groups(&self) -> String {
        format!("{}/groups", self.base)
    }

    #[must_use]
    pub fn command_filter(&self) -> String {
        format!("{}/+/cmnd/control", self.base)
    }

    #[must_use]
    pub fn gateway_reboot_command(&self) -> String {
        format!("{}/cmnd/reboot", self.base)
    }

    #[must_use]
    pub fn node_root(&self, node_id: NodeId) -> String {
        format!("{}/velux_node_{}", self.base, node_id.get())
    }

    #[must_use]
    pub fn node_info(&self, node_id: NodeId) -> String {
        format!("{}/info", self.node_root(node_id))
    }

    #[must_use]
    pub fn node_status(&self, node_id: NodeId) -> String {
        format!("{}/status", self.node_root(node_id))
    }

    #[must_use]
    pub fn node_state(&self, node_id: NodeId) -> String {
        format!("{}/state", self.node_root(node_id))
    }

    #[must_use]
    pub fn node_position(&self, node_id: NodeId) -> String {
        format!("{}/position", self.node_root(node_id))
    }

    #[must_use]
    pub fn node_target(&self, node_id: NodeId) -> String {
        format!("{}/target", self.node_root(node_id))
    }

    #[must_use]
    pub fn node_command(&self, node_id: NodeId) -> String {
        format!("{}/cmnd/control", self.node_root(node_id))
    }

    #[must_use]
    pub fn group_root(&self, group_id: GroupId) -> String {
        format!("{}/velux_group_{}", self.base, group_id.get())
    }

    #[must_use]
    pub fn group_info(&self, group_id: GroupId) -> String {
        format!("{}/info", self.group_root(group_id))
    }

    #[must_use]
    pub fn group_command(&self, group_id: GroupId) -> String {
        format!("{}/cmnd/control", self.group_root(group_id))
    }

    /// Parses a non-retained node, group, or gateway command under this base topic.
    ///
    /// # Errors
    ///
    /// Returns an error for retained commands, unexpected paths, invalid IDs, or unsupported
    /// payloads. Node commands accept `OPEN`, `CLOSE`, and `TOGGLE`; group commands accept only
    /// `OPEN` and `CLOSE`; the gateway reboot command accepts only `REBOOT`, after trimming
    /// surrounding whitespace.
    pub fn parse_command(&self, message: &IncomingMessage) -> Result<MqttCommand, MqttError> {
        if message.retained {
            return Err(MqttError::RetainedCommand);
        }
        let payload = message.payload.trim();
        if message.topic == self.gateway_reboot_command() {
            return if payload.eq_ignore_ascii_case("REBOOT") {
                Ok(MqttCommand::GatewayReboot)
            } else {
                Err(MqttError::InvalidCommandPayload {
                    payload: payload.to_owned(),
                })
            };
        }
        let relative = message
            .topic
            .strip_prefix(&self.base)
            .and_then(|suffix| suffix.strip_prefix('/'))
            .ok_or_else(|| MqttError::UnexpectedCommandTopic {
                topic: message.topic.clone(),
            })?;
        let mut segments = relative.split('/');
        let target_segment = segments.next();
        if segments.next() != Some("cmnd") || segments.next() != Some("control") || segments.next().is_some() {
            return Err(MqttError::UnexpectedCommandTopic {
                topic: message.topic.clone(),
            });
        }
        let target_segment = target_segment.ok_or_else(|| MqttError::UnexpectedCommandTopic {
            topic: message.topic.clone(),
        })?;
        if let Some(node_id) = target_segment.strip_prefix("velux_node_") {
            let node_id = node_id
                .parse::<u8>()
                .map(NodeId::new)
                .map_err(|_| MqttError::InvalidNodeId {
                    topic: message.topic.clone(),
                })?;
            let action = if payload.eq_ignore_ascii_case("OPEN") {
                CoverAction::Open
            } else if payload.eq_ignore_ascii_case("CLOSE") {
                CoverAction::Close
            } else if payload.eq_ignore_ascii_case("TOGGLE") {
                CoverAction::Toggle
            } else {
                return Err(MqttError::InvalidCommandPayload {
                    payload: payload.to_owned(),
                });
            };
            return Ok(MqttCommand::Node(NodeCommand { node_id, action }));
        }
        if let Some(group_id) = target_segment.strip_prefix("velux_group_") {
            let group_id = group_id
                .parse::<u8>()
                .map(GroupId::new)
                .map_err(|_| MqttError::InvalidGroupId {
                    topic: message.topic.clone(),
                })?;
            let action = if payload.eq_ignore_ascii_case("OPEN") {
                GroupAction::Open
            } else if payload.eq_ignore_ascii_case("CLOSE") {
                GroupAction::Close
            } else {
                return Err(MqttError::InvalidCommandPayload {
                    payload: payload.to_owned(),
                });
            };
            return Ok(MqttCommand::Group(GroupCommand { group_id, action }));
        }
        Err(MqttError::UnexpectedCommandTopic {
            topic: message.topic.clone(),
        })
    }

    /// Builds the retained inventory payload.
    ///
    /// # Errors
    ///
    /// Returns an error if JSON serialization fails.
    pub fn inventory_publication(&self, cache: &VeluxNodeCache) -> Result<MqttPublication, MqttError> {
        let node_ids = cache.iter().map(|(node_id, _)| node_id.get()).collect::<Vec<_>>();
        Ok(MqttPublication::retained(self.nodes(), serialize_json(&node_ids)?))
    }

    /// Builds retained metadata for one discovered node.
    ///
    /// # Errors
    ///
    /// Returns an error if JSON serialization fails.
    pub fn info_publication(&self, node: &VeluxNode) -> Result<MqttPublication, MqttError> {
        let metadata = NodeMetadata::from(node);
        Ok(MqttPublication::retained(
            self.node_info(node.id),
            serialize_json(&metadata)?,
        ))
    }

    /// Builds the retained group inventory payload.
    ///
    /// # Errors
    ///
    /// Returns an error if JSON serialization fails.
    pub fn groups_publication<'a>(
        &self,
        groups: impl IntoIterator<Item = &'a GroupInformation>,
    ) -> Result<MqttPublication, MqttError> {
        let group_ids = groups.into_iter().map(|group| group.group_id.get()).collect::<Vec<_>>();
        Ok(MqttPublication::retained(self.groups(), serialize_json(&group_ids)?))
    }

    /// Builds retained metadata for one discovered group.
    ///
    /// # Errors
    ///
    /// Returns an error if JSON serialization fails.
    pub fn group_info_publication(&self, group: &GroupInformation) -> Result<MqttPublication, MqttError> {
        let metadata = GroupMetadata::from(group);
        Ok(MqttPublication::retained(
            self.group_info(group.group_id),
            serialize_json(&metadata)?,
        ))
    }

    /// Builds the atomic snapshot plus state, position, and target leaf topics.
    ///
    /// # Errors
    ///
    /// Returns an error if JSON serialization fails.
    pub fn status_publications(&self, node: &VeluxNode) -> Result<Vec<MqttPublication>, MqttError> {
        let snapshot = node.snapshot();
        let position = snapshot
            .position
            .map_or_else(|| "unknown".to_owned(), |value| value.to_string());
        let target = snapshot
            .target
            .map_or_else(|| "unknown".to_owned(), |value| value.to_string());
        Ok(vec![
            MqttPublication::retained(self.node_status(node.id), serialize_json(&snapshot)?),
            MqttPublication::retained(self.node_state(node.id), cover_state_label(snapshot.state)),
            MqttPublication::retained(self.node_position(node.id), position),
            MqttPublication::retained(self.node_target(node.id), target),
        ])
    }

    #[must_use]
    pub fn availability_publication(&self, online: bool) -> MqttPublication {
        MqttPublication::retained(self.state(), if online { ONLINE_PAYLOAD } else { OFFLINE_PAYLOAD })
    }

    #[must_use]
    pub fn clear_node_publications(&self, node_id: NodeId) -> Vec<MqttPublication> {
        [
            self.node_info(node_id),
            self.node_status(node_id),
            self.node_state(node_id),
            self.node_position(node_id),
            self.node_target(node_id),
        ]
        .into_iter()
        .map(MqttPublication::clear)
        .collect()
    }

    #[must_use]
    pub fn clear_group_publications(&self, group_id: GroupId) -> Vec<MqttPublication> {
        vec![MqttPublication::clear(self.group_info(group_id))]
    }
}

#[derive(Serialize)]
struct NodeMetadata<'a> {
    id: u8,
    name: &'a str,
    serial_number: String,
    actuator_type: &'static str,
    actuator_subtype: u8,
    product_group: i8,
    product_type: i8,
    controllable: bool,
    order: u16,
    placement: u8,
    build_number: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    status_timestamp: Option<u32>,
}

impl<'a> From<&'a VeluxNode> for NodeMetadata<'a> {
    fn from(node: &'a VeluxNode) -> Self {
        Self {
            id: node.id.get(),
            name: &node.name,
            serial_number: node.serial_number_hex(),
            actuator_type: node.actuator_type.label(),
            actuator_subtype: node.actuator_subtype,
            product_group: node.product_group,
            product_type: node.product_type,
            controllable: node.is_controllable(),
            order: node.order,
            placement: node.placement,
            build_number: node.build_number,
            status_timestamp: node
                .freshness
                .and_then(|freshness| freshness.timestamp)
                .map(ProtocolTimestamp::unix_seconds),
        }
    }
}

#[derive(Serialize)]
struct GroupMetadata<'a> {
    id: u8,
    name: &'a str,
    group_type: u8,
    object_count: u8,
    node_ids: Vec<u8>,
    order: u16,
    placement: u8,
    velocity: u8,
    node_variation: u8,
    revision: u16,
}

impl<'a> From<&'a GroupInformation> for GroupMetadata<'a> {
    fn from(group: &'a GroupInformation) -> Self {
        Self {
            id: group.group_id.get(),
            name: &group.name,
            group_type: group.group_type,
            object_count: group.object_count,
            node_ids: group
                .actuators
                .iter()
                .filter_map(|node_id| u8::try_from(node_id).ok())
                .collect(),
            order: group.order,
            placement: group.placement,
            velocity: group.velocity,
            node_variation: group.node_variation,
            revision: group.revision,
        }
    }
}

pub struct MqttConnection {
    client: AsyncClient,
    event_loop: EventLoop,
    topics: TopicLayout,
}

impl MqttConnection {
    /// Creates the MQTT v5 client and event loop.
    ///
    /// Network activity starts when [`MqttConnection::poll`] is first called.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid configuration or base topics.
    pub fn new(mut config: MqttConfig) -> Result<Self, MqttError> {
        config.validate()?;
        let topics = TopicLayout::new(&config.base_topic)?;
        let mut options = MqttOptions::new(config.client_id, config.server, config.port);
        options.set_clean_start(true);
        options.set_keep_alive(config.keep_alive);
        options.set_last_will(LastWill::new(
            topics.state(),
            OFFLINE_PAYLOAD,
            QoS::AtLeastOnce,
            true,
            None,
        ));
        if !config.user.is_empty() {
            options.set_credentials(config.user, config.password);
        }
        let (client, event_loop) = AsyncClient::new(options, config.request_capacity);
        Ok(Self {
            client,
            event_loop,
            topics,
        })
    }

    #[must_use]
    pub fn topics(&self) -> &TopicLayout {
        &self.topics
    }

    /// Advances the rumqttc event loop until a connection or incoming publication is available.
    ///
    /// # Errors
    ///
    /// Returns broker, transport, subscription, or UTF-8 errors.
    pub async fn poll(&mut self) -> Result<MqttEvent, MqttError> {
        loop {
            match self
                .event_loop
                .poll()
                .await
                .map_err(|error| MqttError::Connection(error.to_string()))?
            {
                Event::Incoming(Packet::ConnAck(acknowledgement)) => {
                    if acknowledgement.code != ConnectReturnCode::Success {
                        return Err(MqttError::Connection(format!(
                            "broker rejected connection: {:?}",
                            acknowledgement.code
                        )));
                    }
                    self.client
                        .subscribe(self.topics.command_filter(), QoS::AtLeastOnce)
                        .await
                        .map_err(|error| MqttError::Connection(error.to_string()))?;
                    self.client
                        .subscribe(self.topics.gateway_reboot_command(), QoS::AtLeastOnce)
                        .await
                        .map_err(|error| MqttError::Connection(error.to_string()))?;
                    return Ok(MqttEvent::Connected);
                }
                Event::Incoming(Packet::Publish(publication)) => {
                    let topic = String::from_utf8(publication.topic.to_vec())
                        .map_err(|_| MqttError::InvalidIncomingUtf8 { field: "topic" })?;
                    let payload = String::from_utf8(publication.payload.to_vec())
                        .map_err(|_| MqttError::InvalidIncomingUtf8 { field: "payload" })?;
                    return Ok(MqttEvent::Message(IncomingMessage {
                        topic,
                        payload,
                        retained: publication.retain,
                    }));
                }
                Event::Incoming(_) | Event::Outgoing(_) => {}
            }
        }
    }

    /// Queues one `QoS` 1 publication.
    ///
    /// # Errors
    ///
    /// Returns an error when the event-loop request channel is closed.
    pub async fn publish(&mut self, publication: MqttPublication) -> Result<(), MqttError> {
        self.client
            .publish_bytes(
                publication.topic,
                QoS::AtLeastOnce,
                publication.retained,
                publication.payload,
            )
            .await
            .map_err(|error| MqttError::Connection(error.to_string()))
    }

    /// Queues multiple publications in the supplied order.
    ///
    /// # Errors
    ///
    /// Returns the first client-channel error.
    pub async fn publish_all(
        &mut self,
        publications: impl IntoIterator<Item = MqttPublication>,
    ) -> Result<(), MqttError> {
        for publication in publications {
            self.publish(publication).await?;
        }
        Ok(())
    }

    /// Flushes queued publications and requests a clean MQTT disconnect.
    ///
    /// # Errors
    ///
    /// Returns an error when the event-loop request channel is closed.
    pub async fn disconnect(&mut self) -> Result<(), MqttError> {
        self.client
            .disconnect()
            .await
            .map_err(|error| MqttError::Connection(error.to_string()))?;
        timeout(Duration::from_secs(2), async {
            loop {
                let event = self
                    .event_loop
                    .poll()
                    .await
                    .map_err(|error| MqttError::Connection(error.to_string()))?;
                if event == Event::Outgoing(Outgoing::Disconnect) {
                    return Ok(());
                }
            }
        })
        .await
        .map_err(|_| MqttError::ShutdownTimeout)?
    }
}

fn serialize_json(value: &impl Serialize) -> Result<Bytes, MqttError> {
    serde_json::to_vec(value)
        .map(Bytes::from)
        .map_err(|error| MqttError::Serialization(error.to_string()))
}

const fn cover_state_label(state: CoverState) -> &'static str {
    match state {
        CoverState::Open => "open",
        CoverState::Closed => "closed",
        CoverState::Opening => "opening",
        CoverState::Closing => "closing",
        CoverState::Unknown => "unknown",
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum MqttError {
    #[error("invalid MQTT configuration: {0}")]
    InvalidConfiguration(&'static str),
    #[error("invalid MQTT base topic '{topic}'")]
    InvalidBaseTopic { topic: String },
    #[error("retained MQTT commands are ignored")]
    RetainedCommand,
    #[error("unexpected MQTT command topic '{topic}'")]
    UnexpectedCommandTopic { topic: String },
    #[error("invalid node ID in MQTT topic '{topic}'")]
    InvalidNodeId { topic: String },
    #[error("invalid group ID in MQTT topic '{topic}'")]
    InvalidGroupId { topic: String },
    #[error("invalid MQTT command payload '{payload}'")]
    InvalidCommandPayload { payload: String },
    #[error("incoming MQTT {field} is not UTF-8")]
    InvalidIncomingUtf8 { field: &'static str },
    #[error("MQTT connection error: {0}")]
    Connection(String),
    #[error("MQTT JSON serialization error: {0}")]
    Serialization(String),
    #[error("timed out while flushing the MQTT disconnect")]
    ShutdownTimeout,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configures_v5_last_will_and_clean_session() {
        let config = MqttConfig::new("localhost");
        let connection = MqttConnection::new(config).expect("valid MQTT configuration");
        let options = &connection.event_loop.options;
        assert!(options.clean_start());
        assert_eq!(options.keep_alive(), Duration::from_mins(1));
        let will = options.last_will().expect("offline last will");
        assert_eq!(will.topic.as_ref(), b"velux/state");
        assert_eq!(will.message.as_ref(), b"offline");
        assert_eq!(will.qos, QoS::AtLeastOnce);
        assert!(will.retain);
    }
}
