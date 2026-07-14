use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use thiserror::Error;

use crate::klf200::{
    CommandRequest, CommandRunStatus, CommandTarget, NodeId, NodeInformation, NodeInformationChanged,
    NodeStatePosition, OperatingState, Percentage, ProtocolTimestamp, Request, Response, RunStatus, SessionId,
    StandardParameter, StatusNotification, StatusNotificationDetail,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActuatorType {
    VenetianBlind,
    RollerShutter,
    Awning,
    WindowOpener,
    GarageOpener,
    Light,
    GateOpener,
    RollingDoorOpener,
    Lock,
    Blind,
    Beacon,
    DualShutter,
    HeatingTemperatureInterface,
    OnOffSwitch,
    HorizontalAwning,
    ExteriorVenetianBlind,
    LouvreBlind,
    CurtainTrack,
    VentilationPoint,
    ExteriorHeating,
    HeatPump,
    IntrusionAlarm,
    SwingingShutter,
    Unknown(u16),
}

impl ActuatorType {
    #[must_use]
    pub const fn from_code(code: u16) -> Self {
        match code {
            1 => Self::VenetianBlind,
            2 => Self::RollerShutter,
            3 => Self::Awning,
            4 => Self::WindowOpener,
            5 => Self::GarageOpener,
            6 => Self::Light,
            7 => Self::GateOpener,
            8 => Self::RollingDoorOpener,
            9 => Self::Lock,
            10 => Self::Blind,
            12 => Self::Beacon,
            13 => Self::DualShutter,
            14 => Self::HeatingTemperatureInterface,
            15 => Self::OnOffSwitch,
            16 => Self::HorizontalAwning,
            17 => Self::ExteriorVenetianBlind,
            18 => Self::LouvreBlind,
            19 => Self::CurtainTrack,
            20 => Self::VentilationPoint,
            21 => Self::ExteriorHeating,
            22 => Self::HeatPump,
            23 => Self::IntrusionAlarm,
            24 => Self::SwingingShutter,
            code => Self::Unknown(code),
        }
    }

    #[must_use]
    pub const fn code(self) -> u16 {
        match self {
            Self::VenetianBlind => 1,
            Self::RollerShutter => 2,
            Self::Awning => 3,
            Self::WindowOpener => 4,
            Self::GarageOpener => 5,
            Self::Light => 6,
            Self::GateOpener => 7,
            Self::RollingDoorOpener => 8,
            Self::Lock => 9,
            Self::Blind => 10,
            Self::Beacon => 12,
            Self::DualShutter => 13,
            Self::HeatingTemperatureInterface => 14,
            Self::OnOffSwitch => 15,
            Self::HorizontalAwning => 16,
            Self::ExteriorVenetianBlind => 17,
            Self::LouvreBlind => 18,
            Self::CurtainTrack => 19,
            Self::VentilationPoint => 20,
            Self::ExteriorHeating => 21,
            Self::HeatPump => 22,
            Self::IntrusionAlarm => 23,
            Self::SwingingShutter => 24,
            Self::Unknown(code) => code,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::VenetianBlind => "venetian_blind",
            Self::RollerShutter => "roller_shutter",
            Self::Awning => "awning",
            Self::WindowOpener => "window_opener",
            Self::GarageOpener => "garage_opener",
            Self::Light => "light",
            Self::GateOpener => "gate_opener",
            Self::RollingDoorOpener => "rolling_door_opener",
            Self::Lock => "lock",
            Self::Blind => "blind",
            Self::Beacon => "beacon",
            Self::DualShutter => "dual_shutter",
            Self::HeatingTemperatureInterface => "heating_temperature_interface",
            Self::OnOffSwitch => "on_off_switch",
            Self::HorizontalAwning => "horizontal_awning",
            Self::ExteriorVenetianBlind => "exterior_venetian_blind",
            Self::LouvreBlind => "louvre_blind",
            Self::CurtainTrack => "curtain_track",
            Self::VentilationPoint => "ventilation_point",
            Self::ExteriorHeating => "exterior_heating",
            Self::HeatPump => "heat_pump",
            Self::IntrusionAlarm => "intrusion_alarm",
            Self::SwingingShutter => "swinging_shutter",
            Self::Unknown(_) => "unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoverStrategy {
    MainParameter,
}

impl CoverStrategy {
    #[must_use]
    pub const fn for_actuator(actuator_type: ActuatorType) -> Option<Self> {
        match actuator_type {
            ActuatorType::VenetianBlind
            | ActuatorType::RollerShutter
            | ActuatorType::Awning
            | ActuatorType::WindowOpener
            | ActuatorType::GarageOpener
            | ActuatorType::GateOpener
            | ActuatorType::RollingDoorOpener
            | ActuatorType::Blind
            | ActuatorType::HorizontalAwning
            | ActuatorType::ExteriorVenetianBlind
            | ActuatorType::LouvreBlind
            | ActuatorType::CurtainTrack
            | ActuatorType::SwingingShutter => Some(Self::MainParameter),
            ActuatorType::Light
            | ActuatorType::Lock
            | ActuatorType::Beacon
            | ActuatorType::DualShutter
            | ActuatorType::HeatingTemperatureInterface
            | ActuatorType::OnOffSwitch
            | ActuatorType::VentilationPoint
            | ActuatorType::ExteriorHeating
            | ActuatorType::HeatPump
            | ActuatorType::IntrusionAlarm
            | ActuatorType::Unknown(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CoverState {
    Open,
    Closed,
    Opening,
    Closing,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Availability {
    Available,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatusSource {
    Discovery,
    Notification,
    Poll,
    Command,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StatusFreshness {
    pub source: StatusSource,
    pub timestamp: Option<ProtocolTimestamp>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoverAction {
    Open,
    Close,
    Toggle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoverEndpoint {
    Open,
    Closed,
}

impl CoverEndpoint {
    #[must_use]
    pub const fn parameter(self) -> StandardParameter {
        match self {
            Self::Open => StandardParameter::Relative(Percentage::FULLY_OPEN),
            Self::Closed => StandardParameter::Relative(Percentage::FULLY_CLOSED),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VeluxNode {
    pub id: NodeId,
    pub name: String,
    pub order: u16,
    pub placement: u8,
    pub serial_number: [u8; 8],
    pub node_type_sub_type: u16,
    pub actuator_type: ActuatorType,
    pub actuator_subtype: u8,
    pub product_group: i8,
    pub product_type: i8,
    pub velocity: u8,
    pub variation: u8,
    pub power_mode: u8,
    pub build_number: u8,
    pub cover_strategy: Option<CoverStrategy>,
    pub availability: Availability,
    pub operating_state: OperatingState,
    pub current_position: Option<Percentage>,
    pub target_position: Option<Percentage>,
    pub remaining_time: Option<u16>,
    pub freshness: Option<StatusFreshness>,
}

impl VeluxNode {
    #[must_use]
    pub fn from_information(information: NodeInformation) -> Self {
        let actuator_code = information.node_type_sub_type >> 6;
        let actuator_type = ActuatorType::from_code(actuator_code);
        Self {
            id: information.node_id,
            name: information.name,
            order: information.order,
            placement: information.placement,
            serial_number: information.serial_number,
            node_type_sub_type: information.node_type_sub_type,
            actuator_type,
            actuator_subtype: u8::try_from(information.node_type_sub_type & 0x3F).unwrap_or_default(),
            product_group: information.product_group,
            product_type: information.product_type,
            velocity: information.velocity,
            variation: information.variation,
            power_mode: information.power_mode,
            build_number: information.build_number,
            cover_strategy: CoverStrategy::for_actuator(actuator_type),
            availability: Availability::Available,
            operating_state: information.operating_state,
            current_position: relative_position(information.current_position),
            target_position: relative_position(information.target_position),
            remaining_time: known_remaining_time(information.remaining_time),
            freshness: Some(StatusFreshness {
                source: StatusSource::Discovery,
                timestamp: Some(information.timestamp),
            }),
        }
    }

    #[must_use]
    pub const fn is_controllable(&self) -> bool {
        self.cover_strategy.is_some()
    }

    #[must_use]
    pub fn state(&self) -> CoverState {
        derive_cover_state(self.operating_state, self.current_position, self.target_position)
    }

    #[must_use]
    pub fn snapshot(&self) -> CoverStatusSnapshot {
        CoverStatusSnapshot {
            state: self.state(),
            position: self.current_position.map(mqtt_open_percent),
            target: self.target_position.map(mqtt_open_percent),
            operating_state: operating_state_label(self.operating_state),
            remaining_time: self.remaining_time,
        }
    }

    #[must_use]
    pub fn serial_number_hex(&self) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut serial = String::with_capacity(16);
        for byte in self.serial_number {
            serial.push(char::from(HEX[usize::from(byte >> 4)]));
            serial.push(char::from(HEX[usize::from(byte & 0x0F)]));
        }
        serial
    }

    /// Resolves an MQTT cover action and creates the corresponding KLF command request.
    ///
    /// # Errors
    ///
    /// Returns an error for unsupported actuator types or when toggle cannot determine a safe
    /// endpoint from current movement and position data.
    pub fn command_request(&self, session_id: SessionId, action: CoverAction) -> Result<Request, NodeError> {
        if self.cover_strategy.is_none() {
            return Err(NodeError::UnsupportedActuator { node_id: self.id });
        }
        let endpoint = self.resolve_action(action)?;
        Ok(Request::CommandSend {
            command: CommandRequest::new(session_id, endpoint.parameter()),
            target: CommandTarget::new([self.id]).map_err(|error| NodeError::Protocol {
                message: error.to_string(),
            })?,
        })
    }

    /// Resolves a cover action to an endpoint without mutating state.
    ///
    /// # Errors
    ///
    /// Returns an error when toggle lacks sufficient current or target position information.
    pub fn resolve_action(&self, action: CoverAction) -> Result<CoverEndpoint, NodeError> {
        match action {
            CoverAction::Open => Ok(CoverEndpoint::Open),
            CoverAction::Close => Ok(CoverEndpoint::Closed),
            CoverAction::Toggle => self.toggle_endpoint(),
        }
    }

    pub fn mark_command_accepted(&mut self, endpoint: CoverEndpoint) -> bool {
        let previous = self.clone();
        self.target_position = match endpoint {
            CoverEndpoint::Open => Some(Percentage::FULLY_OPEN),
            CoverEndpoint::Closed => Some(Percentage::FULLY_CLOSED),
        };
        self.operating_state = OperatingState::Executing;
        self.freshness = Some(StatusFreshness {
            source: StatusSource::Command,
            timestamp: None,
        });
        *self != previous
    }

    pub fn apply_command_confirmation(
        &mut self,
        response: &Response,
        session_id: SessionId,
        endpoint: CoverEndpoint,
    ) -> bool {
        match response {
            Response::CommandAccepted {
                session_id: confirmed,
                status: 1,
            } if *confirmed == session_id => self.mark_command_accepted(endpoint),
            _ => false,
        }
    }

    pub fn apply_information_changed(&mut self, information: &NodeInformationChanged) -> bool {
        if information.node_id != self.id {
            return false;
        }
        let previous = self.clone();
        self.name.clone_from(&information.name);
        self.order = information.order;
        self.placement = information.placement;
        self.variation = information.variation;
        *self != previous
    }

    pub fn apply_state_position(&mut self, state: &NodeStatePosition, source: StatusSource) -> bool {
        if state.node_id != self.id {
            return false;
        }
        let previous = self.clone();
        self.availability = Availability::Available;
        self.operating_state = state.operating_state;
        self.current_position = relative_position(state.current_position);
        self.target_position = relative_position(state.target_position);
        self.remaining_time = known_remaining_time(state.remaining_time);
        self.freshness = Some(StatusFreshness {
            source,
            timestamp: Some(state.timestamp),
        });
        *self != previous
    }

    pub fn apply_status(&mut self, status: &StatusNotification, source: StatusSource) -> bool {
        if status.node_id != self.id {
            return false;
        }
        let StatusNotificationDetail::Main {
            target_position,
            current_position,
            remaining_time,
            ..
        } = &status.detail
        else {
            return false;
        };
        let previous = self.clone();
        self.availability = Availability::Available;
        self.operating_state = operating_state_from_run_status(status.run_status);
        self.current_position = relative_position(*current_position);
        self.target_position = relative_position(*target_position);
        self.remaining_time = known_remaining_time(*remaining_time);
        self.freshness = Some(StatusFreshness {
            source,
            timestamp: None,
        });
        *self != previous
    }

    pub fn apply_run_status(&mut self, status: &CommandRunStatus) -> bool {
        if status.node_id != self.id {
            return false;
        }
        let previous = self.clone();
        self.operating_state = operating_state_from_run_status(status.run_status);
        self.remaining_time = match status.run_status {
            RunStatus::Completed => Some(0),
            RunStatus::Failed => None,
            RunStatus::Active | RunStatus::Unknown(_) => self.remaining_time,
        };
        if status.node_parameter == 0 {
            self.current_position = relative_position(status.parameter_value);
        }
        self.freshness = Some(StatusFreshness {
            source: StatusSource::Notification,
            timestamp: None,
        });
        *self != previous
    }

    pub fn set_available(&mut self, available: bool) -> bool {
        let availability = if available {
            Availability::Available
        } else {
            Availability::Unavailable
        };
        if self.availability == availability {
            false
        } else {
            self.availability = availability;
            true
        }
    }

    pub fn mark_status_unknown(&mut self) -> bool {
        let previous = self.clone();
        self.operating_state = OperatingState::Unknown(u8::MAX);
        self.current_position = None;
        self.target_position = None;
        self.remaining_time = None;
        self.freshness = Some(StatusFreshness {
            source: StatusSource::Poll,
            timestamp: None,
        });
        *self != previous
    }

    fn toggle_endpoint(&self) -> Result<CoverEndpoint, NodeError> {
        if matches!(
            self.operating_state,
            OperatingState::Executing | OperatingState::WaitingForPower
        ) {
            if self.target_position == Some(Percentage::FULLY_CLOSED) {
                return Ok(CoverEndpoint::Open);
            }
            if self.target_position == Some(Percentage::FULLY_OPEN) {
                return Ok(CoverEndpoint::Closed);
            }
            match self.state() {
                CoverState::Closing => return Ok(CoverEndpoint::Open),
                CoverState::Opening => return Ok(CoverEndpoint::Closed),
                _ => {}
            }
        }

        self.current_position
            .map(|position| {
                if position >= Percentage::from_percent(50) {
                    CoverEndpoint::Open
                } else {
                    CoverEndpoint::Closed
                }
            })
            .ok_or(NodeError::PositionUnknown { node_id: self.id })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CoverStatusSnapshot {
    pub state: CoverState,
    pub position: Option<u8>,
    pub target: Option<u8>,
    pub operating_state: &'static str,
    pub remaining_time: Option<u16>,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum NodeError {
    #[error("node {node_id:?} is not a supported cover actuator")]
    UnsupportedActuator { node_id: NodeId },
    #[error("node {node_id:?} has no usable position for toggle")]
    PositionUnknown { node_id: NodeId },
    #[error("cannot build KLF command: {message}")]
    Protocol { message: String },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VeluxNodeCache {
    nodes: BTreeMap<NodeId, VeluxNode>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReconcileResult {
    pub added: Vec<NodeId>,
    pub updated: Vec<NodeId>,
    pub removed: Vec<NodeId>,
}

impl VeluxNodeCache {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn get(&self, node_id: NodeId) -> Option<&VeluxNode> {
        self.nodes.get(&node_id)
    }

    pub fn get_mut(&mut self, node_id: NodeId) -> Option<&mut VeluxNode> {
        self.nodes.get_mut(&node_id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&NodeId, &VeluxNode)> {
        self.nodes.iter()
    }

    pub fn reconcile(&mut self, information: Vec<NodeInformation>) -> ReconcileResult {
        let present = information.iter().map(|node| node.node_id).collect::<BTreeSet<_>>();
        let mut result = ReconcileResult::default();
        for information in information {
            let node = VeluxNode::from_information(information);
            match self.nodes.get(&node.id) {
                None => result.added.push(node.id),
                Some(existing) if existing != &node => result.updated.push(node.id),
                Some(_) => {}
            }
            self.nodes.insert(node.id, node);
        }

        let removed = self
            .nodes
            .keys()
            .filter(|node_id| !present.contains(node_id))
            .copied()
            .collect::<Vec<_>>();
        for node_id in &removed {
            self.nodes.remove(node_id);
        }
        result.removed = removed;
        result
    }

    pub fn apply_response(&mut self, response: &Response) -> Option<NodeId> {
        match response {
            Response::NodeInformation(information) => {
                let node = VeluxNode::from_information(information.clone());
                let changed = self.nodes.get(&node.id) != Some(&node);
                self.nodes.insert(node.id, node.clone());
                changed.then_some(node.id)
            }
            Response::NodeInformationChanged(information) => self
                .nodes
                .get_mut(&information.node_id)
                .and_then(|node| node.apply_information_changed(information).then_some(node.id)),
            Response::NodeStatePosition(state) => self.nodes.get_mut(&state.node_id).and_then(|node| {
                node.apply_state_position(state, StatusSource::Notification)
                    .then_some(node.id)
            }),
            Response::StatusNotification(status) => self
                .nodes
                .get_mut(&status.node_id)
                .and_then(|node| node.apply_status(status, StatusSource::Poll).then_some(node.id)),
            Response::CommandRunStatus(status) => self
                .nodes
                .get_mut(&status.node_id)
                .and_then(|node| node.apply_run_status(status).then_some(node.id)),
            Response::CommandRemainingTime { node_id, seconds, .. } => self.nodes.get_mut(node_id).and_then(|node| {
                let updated = known_remaining_time(*seconds);
                if node.remaining_time == updated {
                    None
                } else {
                    node.remaining_time = updated;
                    node.freshness = Some(StatusFreshness {
                        source: StatusSource::Notification,
                        timestamp: None,
                    });
                    Some(node.id)
                }
            }),
            _ => None,
        }
    }

    pub fn set_all_available(&mut self, available: bool) -> Vec<NodeId> {
        self.nodes
            .values_mut()
            .filter_map(|node| node.set_available(available).then_some(node.id))
            .collect()
    }
}

#[must_use]
pub fn derive_cover_state(
    operating_state: OperatingState,
    current_position: Option<Percentage>,
    target_position: Option<Percentage>,
) -> CoverState {
    match operating_state {
        OperatingState::Executing | OperatingState::WaitingForPower => match (current_position, target_position) {
            (Some(current), Some(target)) if target > current => CoverState::Closing,
            (Some(current), Some(target)) if target < current => CoverState::Opening,
            _ => CoverState::Unknown,
        },
        OperatingState::NonExecuting | OperatingState::Done => match current_position {
            Some(Percentage::FULLY_CLOSED) => CoverState::Closed,
            Some(_) => CoverState::Open,
            None => CoverState::Unknown,
        },
        OperatingState::ExecutionError | OperatingState::NotUsed | OperatingState::Unknown(_) => CoverState::Unknown,
    }
}

fn relative_position(parameter: StandardParameter) -> Option<Percentage> {
    match parameter {
        StandardParameter::Relative(position) => Some(position),
        _ => None,
    }
}

fn mqtt_open_percent(position: Percentage) -> u8 {
    100_u8.saturating_sub(position.rounded_percent())
}

fn known_remaining_time(seconds: u16) -> Option<u16> {
    (seconds != u16::MAX).then_some(seconds)
}

fn operating_state_from_run_status(run_status: RunStatus) -> OperatingState {
    match run_status {
        RunStatus::Completed => OperatingState::Done,
        RunStatus::Failed => OperatingState::ExecutionError,
        RunStatus::Active => OperatingState::Executing,
        RunStatus::Unknown(value) => OperatingState::Unknown(value),
    }
}

fn operating_state_label(operating_state: OperatingState) -> &'static str {
    match operating_state {
        OperatingState::NonExecuting => "nonexecuting",
        OperatingState::ExecutionError => "execution_error",
        OperatingState::NotUsed => "not_used",
        OperatingState::WaitingForPower => "waiting_for_power",
        OperatingState::Executing => "executing",
        OperatingState::Done => "done",
        OperatingState::Unknown(_) => "unknown",
    }
}
