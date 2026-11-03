use std::collections::{BTreeMap, BTreeSet};

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
    known_components: BTreeSet<String>,
}

impl HomeAssistantDiscovery {
    #[must_use]
    pub fn new(client_id: impl Into<String>) -> Self {
        Self {
            prefix: DEFAULT_DISCOVERY_PREFIX.to_owned(),
            client_id: client_id.into(),
            known_components: BTreeSet::new(),
        }
    }

    #[must_use]
    pub fn with_prefix(client_id: impl Into<String>, prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into().trim_matches('/').to_owned(),
            client_id: client_id.into(),
            known_components: BTreeSet::new(),
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

    #[must_use]
    pub fn group_unique_id(&self, group: &GroupInformation) -> String {
        format!("{}_group_{}", topic_identifier(&self.client_id), group.group_id.get())
    }

    #[must_use]
    pub fn group_topic(&self, group: &GroupInformation) -> String {
        format!("{}/cover/{}/config", self.prefix, self.group_unique_id(group))
    }

    fn device_identifier(&self) -> String {
        format!("{}_gateway", topic_identifier(&self.client_id))
    }

    #[must_use]
    pub fn device_topic(&self) -> String {
        format!("{}/device/{}/config", self.prefix, self.device_identifier())
    }

    /// Creates device discovery for all controllable nodes and product groups, preceded
    /// by legacy migration messages and followed by retained legacy-topic cleanup.
    ///
    /// # Errors
    ///
    /// Returns an error if JSON serialization fails.
    pub fn publications<'a>(
        &mut self,
        topics: &TopicLayout,
        nodes: impl IntoIterator<Item = &'a VeluxNode>,
        groups: impl IntoIterator<Item = &'a GroupInformation>,
    ) -> Result<Vec<MqttPublication>, MqttError> {
        let mut components = BTreeMap::new();
        for node in nodes.into_iter().filter(|node| node.is_controllable()) {
            let unique_id = self.unique_id(node);
            components.insert(
                unique_id.clone(),
                CoverDiscovery {
                    state_topic: Some(topics.node_state(node.id)),
                    position_topic: Some(topics.node_position(node.id)),
                    state_open: Some("open"),
                    state_opening: Some("opening"),
                    state_closed: Some("closed"),
                    state_closing: Some("closing"),
                    position_open: Some(100),
                    position_closed: Some(0),
                    ..CoverDiscovery::new(
                        node.name.clone(),
                        unique_id,
                        topics.node_command(node.id),
                        device_class(node.actuator_type),
                        false,
                    )
                },
            );
        }
        for group in groups {
            let unique_id = self.group_unique_id(group);
            let name = if group.name.trim().is_empty() {
                format!("VELUX group {}", group.group_id.get())
            } else {
                group.name.clone()
            };
            components.insert(
                unique_id.clone(),
                CoverDiscovery::new(name, unique_id, topics.group_command(group.group_id), "blind", true),
            );
        }

        let legacy_topics = components
            .keys()
            .map(|unique_id| format!("{}/cover/{unique_id}/config", self.prefix))
            .collect::<Vec<_>>();
        let discovery = DeviceDiscovery {
            device: Device {
                identifiers: [self.device_identifier()],
                name: format!("VELUX KLF 200 {}", self.client_id),
                manufacturer: "VELUX",
                model: "KLF 200",
            },
            origin: Origin {
                name: env!("CARGO_PKG_NAME"),
                sw_version: env!("CARGO_PKG_VERSION"),
                support_url: env!("CARGO_PKG_REPOSITORY"),
            },
            availability_topic: topics.state(),
            payload_available: "online",
            payload_not_available: "offline",
            cmps: components,
        };
        let current_components = discovery.cmps.keys().cloned().collect::<BTreeSet<_>>();
        let removed = self
            .known_components
            .difference(&current_components)
            .collect::<Vec<_>>();
        let payload = if discovery.cmps.is_empty() {
            Bytes::new()
        } else {
            serde_json::to_vec(&discovery)
                .map(Bytes::from)
                .map_err(|error| MqttError::Serialization(error.to_string()))?
        };

        // Preserve entity registry settings while moving legacy entities to device discovery.
        let mut publications = legacy_topics
            .iter()
            .map(|topic| MqttPublication::retained(topic, r#"{"migrate_discovery":true}"#))
            .collect::<Vec<_>>();
        if !removed.is_empty() && !discovery.cmps.is_empty() {
            let mut removal =
                serde_json::to_value(&discovery).map_err(|error| MqttError::Serialization(error.to_string()))?;
            for id in removed {
                removal["cmps"][id] = serde_json::json!({"p": "cover"});
            }
            publications.push(MqttPublication::retained(self.device_topic(), removal.to_string()));
        }
        publications.push(MqttPublication::retained(self.device_topic(), payload));
        publications.extend(legacy_topics.into_iter().map(MqttPublication::clear));
        self.known_components = current_components;
        Ok(publications)
    }

    #[must_use]
    pub fn clear_publication(&self, node: &VeluxNode) -> Option<MqttPublication> {
        node.is_controllable().then(|| MqttPublication::clear(self.topic(node)))
    }

    #[must_use]
    pub fn clear_group_publication(&self, group: &GroupInformation) -> MqttPublication {
        MqttPublication::clear(self.group_topic(group))
    }
}

#[derive(Serialize)]
struct DeviceDiscovery {
    device: Device,
    origin: Origin,
    availability_topic: String,
    payload_available: &'static str,
    payload_not_available: &'static str,
    cmps: BTreeMap<String, CoverDiscovery>,
}

#[derive(Serialize)]
struct CoverDiscovery {
    p: &'static str,
    name: String,
    unique_id: String,
    object_id: String,
    command_topic: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    state_topic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    position_topic: Option<String>,
    payload_open: &'static str,
    payload_close: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    state_open: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    state_opening: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    state_closed: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    state_closing: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    position_open: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    position_closed: Option<u8>,
    optimistic: bool,
    qos: u8,
    retain: bool,
    device_class: &'static str,
}

impl CoverDiscovery {
    fn new(
        name: String,
        unique_id: String,
        command_topic: String,
        device_class: &'static str,
        optimistic: bool,
    ) -> Self {
        Self {
            p: "cover",
            name,
            object_id: unique_id.clone(),
            unique_id,
            command_topic,
            state_topic: None,
            position_topic: None,
            payload_open: "OPEN",
            payload_close: "CLOSE",
            state_open: None,
            state_opening: None,
            state_closed: None,
            state_closing: None,
            position_open: None,
            position_closed: None,
            optimistic,
            qos: 1,
            retain: false,
            device_class,
        }
    }
}

#[derive(Serialize)]
struct Device {
    identifiers: [String; 1],
    name: String,
    manufacturer: &'static str,
    model: &'static str,
}

#[derive(Serialize)]
struct Origin {
    name: &'static str,
    sw_version: &'static str,
    support_url: &'static str,
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
