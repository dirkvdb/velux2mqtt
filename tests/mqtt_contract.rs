use serde_json::{Value, json};
use velux2mqtt::hassdiscovery::HomeAssistantDiscovery;
use velux2mqtt::klf200::{
    ActuatorSet, Alias, GroupId, GroupInformation, NodeId, NodeInformation, OperatingState, Percentage,
    ProtocolTimestamp, StandardParameter,
};
use velux2mqtt::mqtt::{GroupAction, IncomingMessage, MqttCommand, MqttConfig, MqttError, TopicLayout};
use velux2mqtt::veluxnode::{CoverAction, VeluxNode, VeluxNodeCache};

#[test]
fn parses_only_exact_non_retained_cover_commands() {
    let topics = TopicLayout::new("velux/home").expect("valid topic");
    let command = topics
        .parse_command(&IncomingMessage {
            topic: "velux/home/velux_node_7/cmnd/control".to_owned(),
            payload: "  tOgGlE\n".to_owned(),
            retained: false,
        })
        .expect("valid command");
    let MqttCommand::Node(command) = command else {
        panic!("expected node command");
    };
    assert_eq!(command.node_id, NodeId::new(7));
    assert_eq!(command.action, CoverAction::Toggle);

    assert_eq!(
        topics.parse_command(&IncomingMessage {
            topic: "velux/home/velux_node_7/cmnd/control".to_owned(),
            payload: "OPEN".to_owned(),
            retained: true,
        }),
        Err(MqttError::RetainedCommand)
    );
    assert!(matches!(
        topics.parse_command(&IncomingMessage {
            topic: "velux/home/velux_node_999/cmnd/control".to_owned(),
            payload: "OPEN".to_owned(),
            retained: false,
        }),
        Err(MqttError::InvalidNodeId { .. })
    ));
    assert!(matches!(
        topics.parse_command(&IncomingMessage {
            topic: "velux/home/velux_node_7/cmnd/position".to_owned(),
            payload: "20".to_owned(),
            retained: false,
        }),
        Err(MqttError::UnexpectedCommandTopic { .. })
    ));
    assert!(matches!(
        topics.parse_command(&IncomingMessage {
            topic: "velux/home/velux_node_7/cmnd/control".to_owned(),
            payload: "STOP".to_owned(),
            retained: false,
        }),
        Err(MqttError::InvalidCommandPayload { .. })
    ));
}

#[test]
fn parses_only_explicit_non_retained_gateway_reboots() {
    let topics = TopicLayout::new("velux/home").expect("valid topic");
    assert_eq!(topics.gateway_reboot_command(), "velux/home/cmnd/reboot");
    assert_eq!(
        topics
            .parse_command(&IncomingMessage {
                topic: "velux/home/cmnd/reboot".to_owned(),
                payload: "  rEbOoT\n".to_owned(),
                retained: false,
            })
            .expect("valid reboot command"),
        MqttCommand::GatewayReboot
    );
    assert_eq!(
        topics.parse_command(&IncomingMessage {
            topic: "velux/home/cmnd/reboot".to_owned(),
            payload: "REBOOT".to_owned(),
            retained: true,
        }),
        Err(MqttError::RetainedCommand)
    );
    assert!(matches!(
        topics.parse_command(&IncomingMessage {
            topic: "velux/home/cmnd/reboot".to_owned(),
            payload: "ON".to_owned(),
            retained: false,
        }),
        Err(MqttError::InvalidCommandPayload { .. })
    ));
}

#[test]
fn parses_only_open_and_close_for_non_retained_groups() {
    let topics = TopicLayout::new("velux/home").expect("valid topic");
    assert_eq!(
        topics.group_command(GroupId::new(7)),
        "velux/home/velux_group_7/cmnd/control"
    );
    let command = topics
        .parse_command(&IncomingMessage {
            topic: "velux/home/velux_group_7/cmnd/control".to_owned(),
            payload: " close ".to_owned(),
            retained: false,
        })
        .expect("valid group command");
    assert!(matches!(
        command,
        MqttCommand::Group(command)
            if command.group_id == GroupId::new(7) && command.action == GroupAction::Close
    ));

    assert!(matches!(
        topics.parse_command(&IncomingMessage {
            topic: "velux/home/velux_group_7/cmnd/control".to_owned(),
            payload: "TOGGLE".to_owned(),
            retained: false,
        }),
        Err(MqttError::InvalidCommandPayload { .. })
    ));
    assert_eq!(
        topics.parse_command(&IncomingMessage {
            topic: "velux/home/velux_group_7/cmnd/control".to_owned(),
            payload: "OPEN".to_owned(),
            retained: true,
        }),
        Err(MqttError::RetainedCommand)
    );
    assert!(matches!(
        topics.parse_command(&IncomingMessage {
            topic: "velux/home/velux_group_999/cmnd/control".to_owned(),
            payload: "OPEN".to_owned(),
            retained: false,
        }),
        Err(MqttError::InvalidGroupId { .. })
    ));
}

#[test]
fn validates_topics_and_redacts_mqtt_passwords() {
    assert!(TopicLayout::new("/velux/").is_ok());
    assert!(TopicLayout::new("velux/+/bridge").is_err());
    assert!(TopicLayout::new("velux//bridge").is_err());

    let mut config = MqttConfig::new(" broker.local ");
    config.password = "mqtt-secret".to_owned();
    config.base_topic = "/velux/home/".to_owned();
    config.validate().expect("valid config");
    assert_eq!(config.server, "broker.local");
    assert_eq!(config.base_topic, "velux/home");
    let debug = format!("{config:?}");
    assert!(!debug.contains("mqtt-secret"));
    assert!(debug.contains("[REDACTED]"));
}

#[test]
fn creates_retained_inventory_metadata_and_status_topics() {
    let topics = TopicLayout::new("velux").expect("valid topic");
    let node = VeluxNode::from_information(node_information(7, "Office", 2));
    let mut cache = VeluxNodeCache::new();
    cache.reconcile(vec![node_information(7, "Office", 2), node_information(8, "Light", 6)]);

    let inventory = topics.inventory_publication(&cache).expect("inventory");
    assert_eq!(inventory.topic, "velux/nodes");
    assert!(inventory.retained);
    assert_eq!(json_payload(&inventory.payload), json!([7, 8]));

    let metadata = topics.info_publication(&node).expect("metadata");
    assert_eq!(metadata.topic, "velux/velux_node_7/info");
    assert_eq!(
        json_payload(&metadata.payload),
        json!({
            "id": 7,
            "name": "Office",
            "serial_number": "0001020304050607",
            "actuator_type": "roller_shutter",
            "actuator_subtype": 0,
            "product_group": 1,
            "product_type": 2,
            "controllable": true,
            "order": 7,
            "placement": 1,
            "build_number": 1,
            "status_timestamp": 100
        })
    );

    let publications = topics.status_publications(&node).expect("status");
    assert_eq!(publications.len(), 4);
    assert_eq!(json_payload(&publications[0].payload), node.snapshot_json());
    assert_eq!(publications[1].payload.as_ref(), b"closing");
    assert_eq!(publications[2].payload.as_ref(), b"75");
    assert_eq!(publications[3].payload.as_ref(), b"0");
    assert!(publications.iter().all(|publication| publication.retained));

    let cleared = topics.clear_node_publications(NodeId::new(7));
    assert_eq!(cleared.len(), 5);
    assert!(cleared.iter().all(|publication| publication.payload.is_empty()));
}

#[test]
fn creates_retained_group_inventory_and_metadata() {
    let topics = TopicLayout::new("velux").expect("valid topic");
    let group = group_information(7, "Awning blind", &[3, 8]);
    let inventory = topics.groups_publication([&group]).expect("group inventory");
    assert_eq!(inventory.topic, "velux/groups");
    assert!(inventory.retained);
    assert_eq!(json_payload(&inventory.payload), json!([7]));

    let metadata = topics.group_info_publication(&group).expect("group metadata");
    assert_eq!(metadata.topic, "velux/velux_group_7/info");
    assert_eq!(
        json_payload(&metadata.payload),
        json!({
            "id": 7,
            "name": "Awning blind",
            "group_type": 0,
            "object_count": 2,
            "node_ids": [3, 8],
            "order": 7,
            "placement": 1,
            "velocity": 0,
            "node_variation": 0,
            "revision": 1
        })
    );
    assert_eq!(
        topics.clear_group_publications(GroupId::new(7)),
        [velux2mqtt::mqtt::MqttPublication::clear("velux/velux_group_7/info")]
    );
}

#[test]
fn publishes_explicit_unknown_leaf_values() {
    let topics = TopicLayout::new("velux").expect("valid topic");
    let mut node = VeluxNode::from_information(node_information(4, "Blind", 2));
    assert!(node.mark_status_unknown());
    let publications = topics.status_publications(&node).expect("status");
    assert_eq!(
        json_payload(&publications[0].payload),
        json!({
            "state": "unknown",
            "position": null,
            "target": null,
            "operating_state": "unknown",
            "remaining_time": null
        })
    );
    assert_eq!(publications[1].payload.as_ref(), b"unknown");
    assert_eq!(publications[2].payload.as_ref(), b"unknown");
    assert_eq!(publications[3].payload.as_ref(), b"unknown");
}

#[test]
fn creates_home_assistant_cover_without_unsupported_controls() {
    let topics = TopicLayout::new("velux").expect("valid topic");
    let mut discovery = HomeAssistantDiscovery::new("Living Room Gateway");
    let node = VeluxNode::from_information(node_information(7, "Office", 2));
    let unsupported = VeluxNode::from_information(node_information(8, "Light", 6));
    let publications = discovery
        .publications(&topics, [&node, &unsupported], [])
        .expect("serialize discovery");
    let publication = publications
        .iter()
        .find(|publication| publication.topic == discovery.device_topic())
        .expect("device discovery");
    assert_eq!(
        publication.topic,
        "homeassistant/device/living_room_gateway_gateway/config"
    );
    assert!(publication.retained);
    let device = json_payload(&publication.payload);
    assert_eq!(device["availability_topic"], "velux/state");
    assert_eq!(device["device"]["manufacturer"], "VELUX");
    assert_eq!(device["device"]["model"], "KLF 200");
    assert_eq!(device["origin"]["name"], "velux2mqtt");
    assert_eq!(device["cmps"].as_object().expect("components").len(), 1);
    let payload = &device["cmps"]["velux_0001020304050607"];
    assert_eq!(payload["p"], "cover");
    assert_eq!(payload["unique_id"], "velux_0001020304050607");
    assert_eq!(payload["object_id"], "velux_0001020304050607");
    assert_eq!(payload["command_topic"], "velux/velux_node_7/cmnd/control");
    assert_eq!(payload["state_topic"], "velux/velux_node_7/state");
    assert_eq!(payload["position_topic"], "velux/velux_node_7/position");
    assert_eq!(payload["payload_open"], "OPEN");
    assert_eq!(payload["payload_close"], "CLOSE");
    assert_eq!(payload["position_closed"], 0);
    assert_eq!(payload["position_open"], 100);
    assert_eq!(payload["device_class"], "shutter");
    assert!(payload.get("device").is_none());
    assert!(payload.get("payload_stop").is_none());
    assert!(payload.get("set_position_topic").is_none());
}

#[test]
fn creates_optimistic_home_assistant_group_with_open_and_close_only() {
    let topics = TopicLayout::new("velux").expect("valid topic");
    let mut discovery = HomeAssistantDiscovery::new("Living Room Gateway");
    let group = group_information(7, "Awning blind", &[3, 8]);
    let node = VeluxNode::from_information(node_information(3, "Office", 2));
    let publications = discovery
        .publications(&topics, [&node], [&group])
        .expect("serialize group discovery");
    let publication = publications
        .iter()
        .find(|publication| publication.topic == discovery.device_topic())
        .expect("device discovery");
    assert_eq!(
        publication.topic,
        "homeassistant/device/living_room_gateway_gateway/config"
    );
    assert!(publication.retained);
    let device = json_payload(&publication.payload);
    assert_eq!(device["cmps"].as_object().expect("components").len(), 2);
    assert_eq!(device["availability_topic"], "velux/state");
    let payload = &device["cmps"]["living_room_gateway_group_7"];
    assert_eq!(payload["name"], "Awning blind");
    assert_eq!(payload["command_topic"], "velux/velux_group_7/cmnd/control");
    assert_eq!(payload["payload_open"], "OPEN");
    assert_eq!(payload["payload_close"], "CLOSE");
    assert_eq!(payload["optimistic"], true);
    assert_eq!(payload["p"], "cover");
    assert!(payload.get("device").is_none());
    assert!(payload.get("state_topic").is_none());
    assert!(payload.get("position_topic").is_none());
    assert!(payload.get("payload_stop").is_none());
    assert!(payload.get("payload_toggle").is_none());
}

#[test]
fn migrates_legacy_discovery_before_publishing_the_shared_device() {
    let topics = TopicLayout::new("home/velux").expect("valid topic");
    let mut discovery = HomeAssistantDiscovery::with_prefix("Gateway A", "/custom/");
    let node = VeluxNode::from_information(node_information(7, "Office", 2));
    let group = group_information(3, "Upstairs", &[7]);
    let publications = discovery.publications(&topics, [&node], [&group]).expect("discovery");
    assert_eq!(publications.len(), 5);
    assert_eq!(publications[2].topic, "custom/device/gateway_a_gateway/config");
    let payload = json_payload(&publications[2].payload);
    assert_eq!(payload["device"]["identifiers"], json!(["gateway_a_gateway"]));
    assert_eq!(payload["availability_topic"], "home/velux/state");
    for (index, id) in ["gateway_a_group_3", "velux_0001020304050607"].iter().enumerate() {
        assert_eq!(publications[index].topic, format!("custom/cover/{id}/config"));
        assert_eq!(
            json_payload(&publications[index].payload),
            json!({"migrate_discovery": true})
        );
        assert_eq!(publications[index + 3].topic, publications[index].topic);
        assert!(publications[index + 3].payload.is_empty());
    }
    assert!(publications.iter().all(|publication| publication.retained));
    let other = HomeAssistantDiscovery::new("Gateway B");
    assert_ne!(discovery.device_topic(), other.device_topic());
}

#[test]
fn removes_missing_components_before_publishing_the_current_inventory() {
    let topics = TopicLayout::new("velux").expect("valid topic");
    let mut discovery = HomeAssistantDiscovery::new("gateway");
    let node = VeluxNode::from_information(node_information(7, "Office", 2));
    let group = group_information(3, "Upstairs", &[7]);
    discovery
        .publications(&topics, [&node], [&group])
        .expect("initial discovery");
    let publications = discovery.publications(&topics, [&node], []).expect("updated discovery");
    let updates = publications
        .iter()
        .filter(|publication| publication.topic == discovery.device_topic())
        .collect::<Vec<_>>();
    assert_eq!(updates.len(), 2);
    assert_eq!(
        json_payload(&updates[0].payload)["cmps"]["gateway_group_3"],
        json!({"p": "cover"})
    );
    let current = json_payload(&updates[1].payload);
    assert!(current["cmps"].get("gateway_group_3").is_none());
    assert_eq!(
        current["cmps"]["velux_0001020304050607"]["unique_id"],
        "velux_0001020304050607"
    );
    let cleared = discovery.publications(&topics, [], []).expect("empty inventory");
    assert_eq!(cleared.len(), 1);
    assert_eq!(cleared[0].topic, discovery.device_topic());
    assert!(cleared[0].retained);
    assert!(cleared[0].payload.is_empty());
}

fn json_payload(payload: &[u8]) -> Value {
    serde_json::from_slice(payload).expect("valid JSON payload")
}

trait SnapshotJson {
    fn snapshot_json(&self) -> Value;
}

impl SnapshotJson for VeluxNode {
    fn snapshot_json(&self) -> Value {
        serde_json::to_value(self.snapshot()).expect("serializable snapshot")
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
        operating_state: OperatingState::Executing,
        current_position: StandardParameter::Relative(Percentage::from_percent(25)),
        target_position: StandardParameter::Relative(Percentage::FULLY_CLOSED),
        functional_positions: [StandardParameter::NoFeedback; 4],
        remaining_time: 12,
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
        object_count: u8::try_from(node_ids.len()).expect("test group size"),
        actuators,
        revision: 1,
    }
}
