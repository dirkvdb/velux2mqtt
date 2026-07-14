use serde_json::json;
use velux2mqtt::klf200::{
    Alias, CommandId, CommandRunStatus, Frame, NodeId, NodeInformation, NodeInformationChanged, NodeStatePosition,
    OperatingState, Percentage, ProtocolTimestamp, Response, RunStatus, SessionId, StandardParameter,
};
use velux2mqtt::veluxnode::{
    ActuatorType, Availability, CoverAction, CoverEndpoint, CoverState, NodeError, VeluxNode, VeluxNodeCache,
    derive_cover_state,
};

#[test]
fn builds_cover_metadata_and_mqtt_snapshot() {
    let node = VeluxNode::from_information(node_information(
        7,
        "Office",
        2,
        StandardParameter::Relative(Percentage::from_percent(25)),
        StandardParameter::Relative(Percentage::FULLY_CLOSED),
        OperatingState::Executing,
    ));

    assert_eq!(node.actuator_type, ActuatorType::RollerShutter);
    assert_eq!(node.actuator_subtype, 0);
    assert_eq!(node.serial_number_hex(), "0001020304050607");
    assert!(node.is_controllable());
    assert_eq!(node.state(), CoverState::Closing);
    assert_eq!(
        serde_json::to_value(node.snapshot()).expect("serialize snapshot"),
        json!({
            "state": "closing",
            "position": 75,
            "target": 0,
            "operating_state": "executing",
            "remaining_time": 12
        })
    );
}

#[test]
fn derives_stationary_partial_and_unknown_states() {
    assert_eq!(
        derive_cover_state(
            OperatingState::Done,
            Some(Percentage::FULLY_CLOSED),
            Some(Percentage::FULLY_CLOSED)
        ),
        CoverState::Closed
    );
    assert_eq!(
        derive_cover_state(
            OperatingState::Done,
            Some(Percentage::from_percent(60)),
            Some(Percentage::from_percent(60))
        ),
        CoverState::Open
    );
    assert_eq!(
        derive_cover_state(
            OperatingState::Executing,
            Some(Percentage::from_percent(75)),
            Some(Percentage::from_percent(25))
        ),
        CoverState::Opening
    );
    assert_eq!(
        derive_cover_state(OperatingState::ExecutionError, Some(Percentage::FULLY_OPEN), None),
        CoverState::Unknown
    );
    assert_eq!(
        derive_cover_state(OperatingState::Done, None, None),
        CoverState::Unknown
    );
}

#[test]
fn toggle_uses_direction_endpoints_and_stationary_threshold() {
    let mut node = VeluxNode::from_information(node_information(
        2,
        "Blind",
        2,
        StandardParameter::Relative(Percentage::from_percent(49)),
        StandardParameter::Relative(Percentage::from_percent(49)),
        OperatingState::Done,
    ));
    assert_eq!(node.resolve_action(CoverAction::Toggle), Ok(CoverEndpoint::Closed));

    node.current_position = Some(Percentage::from_percent(50));
    assert_eq!(node.resolve_action(CoverAction::Toggle), Ok(CoverEndpoint::Open));

    node.operating_state = OperatingState::Executing;
    node.current_position = Some(Percentage::from_percent(20));
    node.target_position = Some(Percentage::FULLY_CLOSED);
    assert_eq!(node.resolve_action(CoverAction::Toggle), Ok(CoverEndpoint::Open));

    node.current_position = Some(Percentage::from_percent(80));
    node.target_position = Some(Percentage::FULLY_OPEN);
    assert_eq!(node.resolve_action(CoverAction::Toggle), Ok(CoverEndpoint::Closed));

    node.operating_state = OperatingState::Done;
    node.current_position = None;
    node.target_position = None;
    assert_eq!(
        node.resolve_action(CoverAction::Toggle),
        Err(NodeError::PositionUnknown {
            node_id: NodeId::new(2)
        })
    );
}

#[test]
fn only_implemented_actuator_strategies_emit_commands() {
    let session_id = SessionId::new(12).expect("nonzero session");
    let roller = VeluxNode::from_information(node_information(
        1,
        "Roller",
        2,
        StandardParameter::Relative(Percentage::FULLY_OPEN),
        StandardParameter::Relative(Percentage::FULLY_OPEN),
        OperatingState::Done,
    ));
    let command = roller
        .command_request(session_id, CoverAction::Close)
        .expect("supported command");
    let frame = command.encode().expect("command frame");
    assert_eq!(frame.command, CommandId::GW_COMMAND_SEND_REQ);
    assert_eq!(&frame.payload[0..2], &12_u16.to_be_bytes());
    assert_eq!(&frame.payload[7..9], &0xC800_u16.to_be_bytes());

    let dual = VeluxNode::from_information(node_information(
        3,
        "Dual",
        13,
        StandardParameter::Relative(Percentage::FULLY_OPEN),
        StandardParameter::Relative(Percentage::FULLY_OPEN),
        OperatingState::Done,
    ));
    assert_eq!(dual.actuator_type, ActuatorType::DualShutter);
    assert!(!dual.is_controllable());
    assert_eq!(
        dual.command_request(session_id, CoverAction::Open),
        Err(NodeError::UnsupportedActuator {
            node_id: NodeId::new(3)
        })
    );

    let unknown = VeluxNode::from_information(node_information(
        4,
        "Future",
        63,
        StandardParameter::NoFeedback,
        StandardParameter::NoFeedback,
        OperatingState::Unknown(0xFE),
    ));
    assert_eq!(unknown.actuator_type, ActuatorType::Unknown(63));
    assert_eq!(unknown.snapshot().position, None);
}

#[test]
fn rejected_commands_do_not_mutate_state_and_failed_runs_are_visible() {
    let mut node = VeluxNode::from_information(node_information(
        5,
        "Bedroom",
        2,
        StandardParameter::Relative(Percentage::from_percent(30)),
        StandardParameter::Relative(Percentage::from_percent(30)),
        OperatingState::Done,
    ));
    let before = node.clone();
    let session_id = SessionId::new(20).expect("nonzero session");
    let rejected = Response::CommandAccepted { session_id, status: 0 };
    assert!(!node.apply_command_confirmation(&rejected, session_id, CoverEndpoint::Closed));
    assert_eq!(node, before);

    let accepted = Response::CommandAccepted { session_id, status: 1 };
    assert!(node.apply_command_confirmation(&accepted, session_id, CoverEndpoint::Closed));
    assert_eq!(node.state(), CoverState::Closing);

    let failed = CommandRunStatus {
        session_id,
        status_id: 1,
        node_id: node.id,
        node_parameter: 0,
        parameter_value: StandardParameter::Relative(Percentage::from_percent(35)),
        run_status: RunStatus::Failed,
        status_reply: 8,
        information_code: 0,
    };
    assert!(node.apply_run_status(&failed));
    assert_eq!(node.operating_state, OperatingState::ExecutionError);
    assert_eq!(node.state(), CoverState::Unknown);
}

#[test]
fn state_updates_are_change_detected_and_cache_reconciles_removed_nodes() {
    let first = node_information(
        1,
        "One",
        2,
        StandardParameter::Relative(Percentage::FULLY_OPEN),
        StandardParameter::Relative(Percentage::FULLY_OPEN),
        OperatingState::Done,
    );
    let second = node_information(
        2,
        "Two",
        6,
        StandardParameter::NoFeedback,
        StandardParameter::NoFeedback,
        OperatingState::Done,
    );
    let mut cache = VeluxNodeCache::new();
    let initial = cache.reconcile(vec![first.clone(), second]);
    assert_eq!(initial.added, [NodeId::new(1), NodeId::new(2)]);
    assert!(initial.removed.is_empty());

    let reconciled = cache.reconcile(vec![first]);
    assert_eq!(reconciled.removed, [NodeId::new(2)]);
    assert_eq!(cache.iter().count(), 1);

    let state = NodeStatePosition {
        node_id: NodeId::new(1),
        operating_state: OperatingState::Executing,
        current_position: StandardParameter::Relative(Percentage::from_percent(10)),
        target_position: StandardParameter::Relative(Percentage::FULLY_CLOSED),
        functional_positions: [StandardParameter::NoFeedback; 4],
        remaining_time: 9,
        timestamp: ProtocolTimestamp::from_unix_seconds(101),
    };
    assert_eq!(
        cache.apply_response(&Response::NodeStatePosition(state)),
        Some(NodeId::new(1))
    );
    assert_eq!(
        cache.get(NodeId::new(1)).expect("cached node").availability,
        Availability::Available
    );
    assert_eq!(cache.apply_response(&Response::NodeStatePosition(state)), None);
    assert_eq!(cache.set_all_available(false), [NodeId::new(1)]);
    assert!(cache.set_all_available(false).is_empty());

    let changed = NodeInformationChanged {
        node_id: NodeId::new(1),
        name: "Renamed".to_owned(),
        order: 4,
        placement: 3,
        variation: 2,
    };
    assert_eq!(
        cache.apply_response(&Response::NodeInformationChanged(changed)),
        Some(NodeId::new(1))
    );
    assert_eq!(cache.get(NodeId::new(1)).expect("renamed node").name, "Renamed");
}

fn node_information(
    node_id: u8,
    name: &str,
    actuator_code: u16,
    current_position: StandardParameter,
    target_position: StandardParameter,
    operating_state: OperatingState,
) -> NodeInformation {
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
        operating_state,
        current_position,
        target_position,
        functional_positions: [StandardParameter::NoFeedback; 4],
        remaining_time: 12,
        timestamp: ProtocolTimestamp::from_unix_seconds(100),
        aliases: vec![Alias { kind: 1, value: 2 }],
    }
}

#[test]
fn unknown_run_status_and_response_types_remain_safe() {
    let frame = Frame::new(CommandId::new(0xDEAD), [1, 2].as_slice());
    let response = Response::decode(frame).expect("unknown response");
    assert!(matches!(response, Response::Unknown { .. }));
    assert_eq!(
        derive_cover_state(OperatingState::Unknown(0xA5), None, None),
        CoverState::Unknown
    );
}
