use bytes::Bytes;
use serde::Serialize;

use crate::klf200::GroupInformation;
use crate::mqtt::{MqttError, MqttPublication, TopicLayout};
use crate::veluxnode::{ActuatorType, VeluxNode};

pub const DEFAULT_DISCOVERY_PREFIX: &str = "homeassistant";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HomeAssistantDiscovery {
    prefix: String,
    client_id: String,
}

impl HomeAssistantDiscovery {
    #[must_use]
    pub fn new(client_id: impl Into<String>) -> Self {
        Self {
            prefix: DEFAULT_DISCOVERY_PREFIX.to_owned(),
            client_id: client_id.into(),
        }
    }

    #[must_use]
    pub fn with_prefix(client_id: impl Into<String>, prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into().trim_matches('/').to_owned(),
            client_id: client_id.into(),
        }
    }

    #[must_use]
    pub fn unique_id(&self, node: &VeluxNode) -> String {
        if node.serial_number.iter().any(|byte| *byte != 0) {
            format!("velux_{}", node.serial_number_hex())
        } else {
            format!("{}_node_{}", topic_identifier(&self.client_id), node.id.get())
        }
    }

    #[must_use]
    pub fn topic(&self, node: &VeluxNode) -> String {
        format!("{}/cover/{}/config", self.prefix, self.unique_id(node))
    }

    /// Creates a retained Home Assistant MQTT cover discovery message.
    ///
    /// # Errors
    ///
    /// Returns an error if JSON serialization fails.
    pub fn publication(&self, topics: &TopicLayout, node: &VeluxNode) -> Result<Option<MqttPublication>, MqttError> {
        if !node.is_controllable() {
            return Ok(None);
        }
        let unique_id = self.unique_id(node);
        let serial_number = node.serial_number_hex();
        let payload = CoverDiscovery {
            name: &node.name,
            unique_id: &unique_id,
            object_id: &unique_id,
            availability_topic: topics.state(),
            command_topic: topics.node_command(node.id),
            state_topic: topics.node_state(node.id),
            position_topic: topics.node_position(node.id),
            payload_available: "online",
            payload_not_available: "offline",
            payload_open: "OPEN",
            payload_close: "CLOSE",
            state_open: "open",
            state_opening: "opening",
            state_closed: "closed",
            state_closing: "closing",
            position_open: 100,
            position_closed: 0,
            optimistic: false,
            qos: 1,
            retain: false,
            device_class: device_class(node.actuator_type),
            device: Device {
                identifiers: [&unique_id],
                name: &node.name,
                manufacturer: "VELUX",
                model: node.actuator_type.label(),
                serial_number: Some(&serial_number),
            },
            origin: Origin {
                name: env!("CARGO_PKG_NAME"),
                sw_version: env!("CARGO_PKG_VERSION"),
                support_url: env!("CARGO_PKG_REPOSITORY"),
            },
        };
        let bytes = serde_json::to_vec(&payload)
            .map(Bytes::from)
            .map_err(|error| MqttError::Serialization(error.to_string()))?;
        Ok(Some(MqttPublication::retained(self.topic(node), bytes)))
    }

    #[must_use]
    pub fn clear_publication(&self, node: &VeluxNode) -> Option<MqttPublication> {
        node.is_controllable().then(|| MqttPublication::clear(self.topic(node)))
    }

    #[must_use]
    pub fn group_unique_id(&self, group: &GroupInformation) -> String {
        format!("{}_group_{}", topic_identifier(&self.client_id), group.group_id.get())
    }

    #[must_use]
    pub fn group_topic(&self, group: &GroupInformation) -> String {
        format!("{}/cover/{}/config", self.prefix, self.group_unique_id(group))
    }

    /// Creates an optimistic Home Assistant cover for a KLF product group.
    ///
    /// Groups expose only `OPEN` and `CLOSE`, without an aggregate state topic.
    ///
    /// # Errors
    ///
    /// Returns an error if JSON serialization fails.
    pub fn group_publication(
        &self,
        topics: &TopicLayout,
        group: &GroupInformation,
    ) -> Result<MqttPublication, MqttError> {
        let unique_id = self.group_unique_id(group);
        let name = if group.name.trim().is_empty() {
            format!("VELUX group {}", group.group_id.get())
        } else {
            group.name.clone()
        };
        let payload = GroupCoverDiscovery {
            name: &name,
            unique_id: &unique_id,
            object_id: &unique_id,
            availability_topic: topics.state(),
            command_topic: topics.group_command(group.group_id),
            payload_available: "online",
            payload_not_available: "offline",
            payload_open: "OPEN",
            payload_close: "CLOSE",
            optimistic: true,
            qos: 1,
            retain: false,
            device_class: "blind",
            device: Device {
                identifiers: [&unique_id],
                name: &name,
                manufacturer: "VELUX",
                model: "group",
                serial_number: None,
            },
            origin: Origin {
                name: env!("CARGO_PKG_NAME"),
                sw_version: env!("CARGO_PKG_VERSION"),
                support_url: env!("CARGO_PKG_REPOSITORY"),
            },
        };
        let bytes = serde_json::to_vec(&payload)
            .map(Bytes::from)
            .map_err(|error| MqttError::Serialization(error.to_string()))?;
        Ok(MqttPublication::retained(self.group_topic(group), bytes))
    }

    #[must_use]
    pub fn clear_group_publication(&self, group: &GroupInformation) -> MqttPublication {
        MqttPublication::clear(self.group_topic(group))
    }
}

#[derive(Serialize)]
struct CoverDiscovery<'a> {
    name: &'a str,
    unique_id: &'a str,
    object_id: &'a str,
    availability_topic: String,
    command_topic: String,
    state_topic: String,
    position_topic: String,
    payload_available: &'static str,
    payload_not_available: &'static str,
    payload_open: &'static str,
    payload_close: &'static str,
    state_open: &'static str,
    state_opening: &'static str,
    state_closed: &'static str,
    state_closing: &'static str,
    position_open: u8,
    position_closed: u8,
    optimistic: bool,
    qos: u8,
    retain: bool,
    device_class: &'static str,
    device: Device<'a>,
    origin: Origin<'static>,
}

#[derive(Serialize)]
struct GroupCoverDiscovery<'a> {
    name: &'a str,
    unique_id: &'a str,
    object_id: &'a str,
    availability_topic: String,
    command_topic: String,
    payload_available: &'static str,
    payload_not_available: &'static str,
    payload_open: &'static str,
    payload_close: &'static str,
    optimistic: bool,
    qos: u8,
    retain: bool,
    device_class: &'static str,
    device: Device<'a>,
    origin: Origin<'static>,
}

#[derive(Serialize)]
struct Device<'a> {
    identifiers: [&'a str; 1],
    name: &'a str,
    manufacturer: &'static str,
    model: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    serial_number: Option<&'a str>,
}

#[derive(Serialize)]
struct Origin<'a> {
    name: &'a str,
    sw_version: &'a str,
    support_url: &'a str,
}

const fn device_class(actuator_type: ActuatorType) -> &'static str {
    match actuator_type {
        ActuatorType::Awning | ActuatorType::HorizontalAwning => "awning",
        ActuatorType::CurtainTrack => "curtain",
        ActuatorType::GarageOpener => "garage",
        ActuatorType::GateOpener => "gate",
        ActuatorType::WindowOpener => "window",
        ActuatorType::RollerShutter
        | ActuatorType::RollingDoorOpener
        | ActuatorType::Blind
        | ActuatorType::SwingingShutter => "shutter",
        _ => "blind",
    }
}

fn topic_identifier(value: &str) -> String {
    let mut identifier = String::with_capacity(value.len());
    let mut last_was_separator = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            identifier.push(character.to_ascii_lowercase());
            last_was_separator = false;
        } else if !last_was_separator && !identifier.is_empty() {
            identifier.push('_');
            last_was_separator = true;
        }
    }
    while identifier.ends_with('_') {
        identifier.pop();
    }
    if identifier.is_empty() {
        "velux2mqtt".to_owned()
    } else {
        identifier
    }
}
