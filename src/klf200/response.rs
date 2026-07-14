use bytes::Bytes;

use super::{
    ActuatorSet, Alias, BeaconSet, CommandId, ContactInputLink, Frame, GroupId, GroupInformation, KlfError,
    NetworkSetup, NodeId, NodeSet, ProtocolTimestamp, Result, SessionId, StandardParameter, decode_fixed_string,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Version {
    pub software: [u8; 6],
    pub hardware: u8,
    pub product_group: u8,
    pub product_type: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtocolVersion {
    pub major: u16,
    pub minor: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GatewayState {
    pub state: u8,
    pub sub_state: u8,
    pub data: [u8; 4],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalTime {
    pub utc_time: ProtocolTimestamp,
    pub second: u8,
    pub minute: u8,
    pub hour: u8,
    pub day_of_month: u8,
    pub month_since_january: u8,
    pub years_since_1900: i16,
    pub week_day: u8,
    pub day_of_year: u16,
    pub daylight_saving_flag: i8,
}

#[derive(Clone, Eq, PartialEq)]
pub struct PasswordChangedNotification {
    pub new_password: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SystemTableObject {
    pub index: u8,
    pub actuator_address: [u8; 3],
    pub actuator_type_sub_type: u16,
    pub power_mode_and_flags: u8,
    pub manufacturer_id: u8,
    pub backbone_reference: [u8; 3],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemTableData {
    pub entries: Vec<SystemTableObject>,
    pub remaining_entries: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NodeDiscoveryResult {
    pub added: NodeSet,
    pub rf_connection_error: NodeSet,
    pub key_error_existing_node: NodeSet,
    pub removed: NodeSet,
    pub open: NodeSet,
    pub status: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControllerCopyResult {
    pub mode: u8,
    pub status: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyChangeResult {
    pub command: CommandId,
    pub status: i8,
    pub changed: NodeSet,
    pub unchanged: NodeSet,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PgcJob {
    pub state: i8,
    pub status: i8,
    pub job_type: i8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemTableUpdate {
    pub added: NodeSet,
    pub removed: NodeSet,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigurationActivationResult {
    pub activated: NodeSet,
    pub no_contact: NodeSet,
    pub other_error: NodeSet,
    pub status: i8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NodeMutationResult {
    pub command: CommandId,
    pub status: u8,
    pub node_id: NodeId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GroupOperationResult {
    pub command: CommandId,
    pub status: u8,
    pub group_id: GroupId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupInformationNotification {
    pub command: CommandId,
    pub information: GroupInformation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupChange {
    pub change_type: u8,
    pub group_id: GroupId,
    pub information: Option<GroupInformation>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AllGroupsAccepted {
    pub status: i8,
    pub total_groups: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionCommandResult {
    pub command: CommandId,
    pub session_id: SessionId,
    pub status: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LimitationStatus {
    pub session_id: SessionId,
    pub node_id: NodeId,
    pub parameter_id: u8,
    pub minimum: StandardParameter,
    pub maximum: StandardParameter,
    pub originator: u8,
    pub limitation_time: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SceneStatusResult {
    pub command: CommandId,
    pub status: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SceneInitializationResult {
    pub status: u8,
    pub node_states: [bool; 25],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SceneObjectResult {
    pub command: CommandId,
    pub status: u8,
    pub scene_id: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SceneSummary {
    pub scene_id: u8,
    pub name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SceneList {
    pub scenes: Vec<SceneSummary>,
    pub remaining_scenes: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SceneNode {
    pub node_id: NodeId,
    pub parameter_id: u8,
    pub parameter: StandardParameter,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SceneInformation {
    pub scene_id: u8,
    pub name: String,
    pub nodes: Vec<SceneNode>,
    pub remaining_nodes: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SceneSessionResult {
    pub command: CommandId,
    pub status: u8,
    pub session_id: SessionId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SceneChange {
    pub change_type: u8,
    pub scene_id: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductGroupResult {
    pub session_id: SessionId,
    pub status: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContactInputOperationResult {
    pub command: CommandId,
    pub contact_input_id: u8,
    pub status: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivationLogHeader {
    pub maximum_lines: u16,
    pub line_count: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivationLogEntry {
    pub command: CommandId,
    pub timestamp: ProtocolTimestamp,
    pub session_id: SessionId,
    pub status_id: u8,
    pub node_id: NodeId,
    pub node_parameter: u8,
    pub parameter_value: StandardParameter,
    pub run_status: RunStatus,
    pub status_reply: u8,
    pub information_code: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MultipleActivationLogResult {
    pub line_count: u16,
    pub status: u8,
}

impl std::fmt::Debug for PasswordChangedNotification {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("PasswordChangedNotification([REDACTED])")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperatingState {
    NonExecuting,
    ExecutionError,
    NotUsed,
    WaitingForPower,
    Executing,
    Done,
    Unknown(u8),
}

impl From<u8> for OperatingState {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::NonExecuting,
            1 => Self::ExecutionError,
            2 => Self::NotUsed,
            3 => Self::WaitingForPower,
            4 => Self::Executing,
            5 => Self::Done,
            value => Self::Unknown(value),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NodeInformation {
    pub node_id: NodeId,
    pub order: u16,
    pub placement: u8,
    pub name: String,
    pub velocity: u8,
    pub node_type_sub_type: u16,
    pub product_group: i8,
    pub product_type: i8,
    pub variation: u8,
    pub power_mode: u8,
    pub build_number: u8,
    pub serial_number: [u8; 8],
    pub operating_state: OperatingState,
    pub current_position: StandardParameter,
    pub target_position: StandardParameter,
    pub functional_positions: [StandardParameter; 4],
    pub remaining_time: u16,
    pub timestamp: ProtocolTimestamp,
    pub aliases: Vec<Alias>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NodeStatePosition {
    pub node_id: NodeId,
    pub operating_state: OperatingState,
    pub current_position: StandardParameter,
    pub target_position: StandardParameter,
    pub functional_positions: [StandardParameter; 4],
    pub remaining_time: u16,
    pub timestamp: ProtocolTimestamp,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NodeInformationChanged {
    pub node_id: NodeId,
    pub name: String,
    pub order: u16,
    pub placement: u8,
    pub variation: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunStatus {
    Completed,
    Failed,
    Active,
    Unknown(u8),
}

impl From<u8> for RunStatus {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Completed,
            1 => Self::Failed,
            2 => Self::Active,
            value => Self::Unknown(value),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommandRunStatus {
    pub session_id: SessionId,
    pub status_id: u8,
    pub node_id: NodeId,
    pub node_parameter: u8,
    pub parameter_value: StandardParameter,
    pub run_status: RunStatus,
    pub status_reply: u8,
    pub information_code: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParameterStatus {
    pub node_parameter: u8,
    pub value: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StatusNotificationDetail {
    Parameters(Vec<ParameterStatus>),
    Main {
        target_position: StandardParameter,
        current_position: StandardParameter,
        remaining_time: u16,
        last_master_execution_address: u32,
        last_command_originator: u8,
    },
    Unknown {
        status_type: u8,
        payload: Bytes,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatusNotification {
    pub session_id: SessionId,
    pub status_id: u8,
    pub node_id: NodeId,
    pub run_status: RunStatus,
    pub status_reply: u8,
    pub detail: StatusNotificationDetail,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Response {
    ErrorNotification {
        error_number: u8,
    },
    Acknowledgement {
        command: CommandId,
    },
    PasswordEntered {
        accepted: bool,
    },
    PasswordChanged {
        accepted: bool,
    },
    PasswordChangedNotification(PasswordChangedNotification),
    OperationStatus {
        command: CommandId,
        status: u8,
    },
    Version(Version),
    ProtocolVersion(ProtocolVersion),
    GatewayState(GatewayState),
    NetworkSetup(NetworkSetup),
    LocalTime(LocalTime),
    SystemTableData(SystemTableData),
    NodeDiscoveryResult(NodeDiscoveryResult),
    NodesRemoved {
        scene_deleted: bool,
    },
    ControllerCopyResult(ControllerCopyResult),
    KeyChangeResult(KeyChangeResult),
    PgcJob(PgcJob),
    SystemTableUpdate(SystemTableUpdate),
    ConfigurationActivationResult(ConfigurationActivationResult),
    NodeMutationResult(NodeMutationResult),
    GroupOperationResult(GroupOperationResult),
    GroupInformation(GroupInformationNotification),
    GroupChange(GroupChange),
    AllGroupsAccepted(AllGroupsAccepted),
    GroupDeleted(GroupId),
    SessionCommandResult(SessionCommandResult),
    WinkFinished {
        session_id: SessionId,
    },
    LimitationStatus(LimitationStatus),
    ModeNotification(Bytes),
    SceneStatusResult(SceneStatusResult),
    SceneInitializationResult(SceneInitializationResult),
    SceneObjectResult(SceneObjectResult),
    SceneListAccepted {
        total_scenes: u8,
    },
    SceneList(SceneList),
    SceneInformation(SceneInformation),
    SceneSessionResult(SceneSessionResult),
    SceneChange(SceneChange),
    ProductGroupResult(ProductGroupResult),
    ProductGroupNotification(Bytes),
    ContactInputLinks(Vec<ContactInputLink>),
    ContactInputOperationResult(ContactInputOperationResult),
    ActivationLogHeader(ActivationLogHeader),
    ActivationLogEntry(ActivationLogEntry),
    MultipleActivationLogResult(MultipleActivationLogResult),
    NodeInformationAccepted {
        status: u8,
        node_id: NodeId,
    },
    AllNodesInformationAccepted {
        status: u8,
        total_nodes: u8,
    },
    NodeInformation(NodeInformation),
    NodeInformationChanged(NodeInformationChanged),
    NodeStatePosition(NodeStatePosition),
    CommandAccepted {
        session_id: SessionId,
        status: u8,
    },
    CommandRunStatus(CommandRunStatus),
    CommandRemainingTime {
        session_id: SessionId,
        node_id: NodeId,
        node_parameter: u8,
        seconds: u16,
    },
    SessionFinished {
        session_id: SessionId,
    },
    StatusAccepted {
        session_id: SessionId,
        status: u8,
    },
    StatusNotification(StatusNotification),
    Unknown {
        command: CommandId,
        payload: Bytes,
    },
}

impl Response {
    /// Decodes the typed subset of a validated KLF frame.
    ///
    /// Commands outside the implemented subset are preserved in [`Response::Unknown`].
    ///
    /// # Errors
    ///
    /// Returns an error when a supported response is truncated or contains an invalid field such
    /// as session ID zero, invalid UTF-8, or too many node aliases.
    pub fn decode(frame: Frame) -> Result<Self> {
        let command = frame.command;
        let payload = frame.payload;
        match command.raw() {
            0x0100..=0x011A => decode_configuration(command, &payload),
            0x0206..=0x0209 | 0x020D..=0x020E | 0x0220..=0x0230 => decode_node_group(command, &payload),
            0x0308..=0x0322 => decode_command_extension(command, &payload),
            0x0400..=0x0465 => decode_scene_contact(command, &payload),
            0x0500..=0x0509 => decode_activation_log(command, &payload),
            _ => decode_core(command, payload),
        }
    }
}

fn decode_core(command: CommandId, payload: Bytes) -> Result<Response> {
    match command {
        CommandId::GW_REBOOT_CFM
        | CommandId::GW_SET_FACTORY_DEFAULT_CFM
        | CommandId::GW_HOUSE_STATUS_MONITOR_ENABLE_CFM
        | CommandId::GW_HOUSE_STATUS_MONITOR_DISABLE_CFM
        | CommandId::GW_GET_ALL_NODES_INFORMATION_FINISHED_NTF => {
            require_exact(&payload, 0, command)?;
            Ok(Response::Acknowledgement { command })
        }
        CommandId::GW_ERROR_NTF
        | CommandId::GW_SET_NETWORK_SETUP_CFM
        | CommandId::GW_SET_UTC_CFM
        | CommandId::GW_PASSWORD_ENTER_CFM
        | CommandId::GW_PASSWORD_CHANGE_CFM
        | CommandId::GW_PASSWORD_CHANGE_NTF
        | CommandId::GW_GET_VERSION_CFM
        | CommandId::GW_GET_PROTOCOL_VERSION_CFM
        | CommandId::GW_GET_STATE_CFM
        | CommandId::GW_LEAVE_LEARN_STATE_CFM
        | CommandId::GW_RTC_SET_TIME_ZONE_CFM
        | CommandId::GW_GET_NETWORK_SETUP_CFM
        | CommandId::GW_GET_LOCAL_TIME_CFM => decode_gateway(command, &payload),
        CommandId::GW_GET_NODE_INFORMATION_CFM => {
            require_exact(&payload, 2, command)?;
            Ok(Response::NodeInformationAccepted {
                status: payload[0],
                node_id: NodeId::new(payload[1]),
            })
        }
        CommandId::GW_GET_ALL_NODES_INFORMATION_CFM => {
            require_exact(&payload, 2, command)?;
            Ok(Response::AllNodesInformationAccepted {
                status: payload[0],
                total_nodes: payload[1],
            })
        }
        CommandId::GW_GET_NODE_INFORMATION_NTF | CommandId::GW_GET_ALL_NODES_INFORMATION_NTF => {
            decode_node_information(&payload, command).map(Response::NodeInformation)
        }
        CommandId::GW_NODE_INFORMATION_CHANGED_NTF => {
            require_exact(&payload, 69, command)?;
            Ok(Response::NodeInformationChanged(NodeInformationChanged {
                node_id: NodeId::new(payload[0]),
                name: decode_fixed_string(&payload[1..65])?,
                order: read_u16(&payload, 65),
                placement: payload[67],
                variation: payload[68],
            }))
        }
        CommandId::GW_NODE_STATE_POSITION_CHANGED_NTF => {
            decode_node_state(&payload, command).map(Response::NodeStatePosition)
        }
        CommandId::GW_COMMAND_SEND_CFM => decode_session_status(&payload, command, true),
        CommandId::GW_COMMAND_RUN_STATUS_NTF => {
            decode_command_run_status(&payload, command).map(Response::CommandRunStatus)
        }
        CommandId::GW_COMMAND_REMAINING_TIME_NTF => {
            require_exact(&payload, 6, command)?;
            Ok(Response::CommandRemainingTime {
                session_id: session_id(&payload, command)?,
                node_id: NodeId::new(payload[2]),
                node_parameter: payload[3],
                seconds: read_u16(&payload, 4),
            })
        }
        CommandId::GW_SESSION_FINISHED_NTF => {
            require_exact(&payload, 2, command)?;
            Ok(Response::SessionFinished {
                session_id: session_id(&payload, command)?,
            })
        }
        CommandId::GW_STATUS_REQUEST_CFM => decode_session_status(&payload, command, false),
        CommandId::GW_STATUS_REQUEST_NTF => {
            decode_status_notification(&payload, command).map(Response::StatusNotification)
        }
        _ => Ok(Response::Unknown { command, payload }),
    }
}

fn decode_session_status(payload: &[u8], command: CommandId, is_command: bool) -> Result<Response> {
    require_exact(payload, 3, command)?;
    let session_id = session_id(payload, command)?;
    if is_command {
        Ok(Response::CommandAccepted {
            session_id,
            status: payload[2],
        })
    } else {
        Ok(Response::StatusAccepted {
            session_id,
            status: payload[2],
        })
    }
}

fn decode_activation_log(command: CommandId, payload: &[u8]) -> Result<Response> {
    match command {
        CommandId::GW_GET_ACTIVATION_LOG_HEADER_CFM => {
            require_exact(payload, 4, command)?;
            Ok(Response::ActivationLogHeader(ActivationLogHeader {
                maximum_lines: read_u16(payload, 0),
                line_count: read_u16(payload, 2),
            }))
        }
        CommandId::GW_CLEAR_ACTIVATION_LOG_CFM | CommandId::GW_ACTIVATION_LOG_UPDATED_NTF => {
            require_exact(payload, 0, command)?;
            Ok(Response::Acknowledgement { command })
        }
        CommandId::GW_GET_ACTIVATION_LOG_LINE_CFM | CommandId::GW_GET_MULTIPLE_ACTIVATION_LOG_LINES_NTF => {
            require_exact(payload, 17, command)?;
            Ok(Response::ActivationLogEntry(ActivationLogEntry {
                command,
                timestamp: ProtocolTimestamp::from_unix_seconds(read_u32(payload, 0)),
                session_id: session_id_at(payload, 4, command)?,
                status_id: payload[6],
                node_id: NodeId::new(payload[7]),
                node_parameter: payload[8],
                parameter_value: StandardParameter::from_raw(read_u16(payload, 9)),
                run_status: payload[11].into(),
                status_reply: payload[12],
                information_code: read_u32(payload, 13),
            }))
        }
        CommandId::GW_GET_MULTIPLE_ACTIVATION_LOG_LINES_CFM => {
            require_exact(payload, 3, command)?;
            Ok(Response::MultipleActivationLogResult(MultipleActivationLogResult {
                line_count: read_u16(payload, 0),
                status: payload[2],
            }))
        }
        _ => Ok(Response::Unknown {
            command,
            payload: Bytes::copy_from_slice(payload),
        }),
    }
}

fn decode_scene_contact(command: CommandId, payload: &[u8]) -> Result<Response> {
    match command {
        CommandId::GW_INITIALIZE_SCENE_CFM
        | CommandId::GW_INITIALIZE_SCENE_CANCEL_CFM
        | CommandId::GW_RECORD_SCENE_CFM => {
            require_exact(payload, 1, command)?;
            Ok(Response::SceneStatusResult(SceneStatusResult {
                command,
                status: payload[0],
            }))
        }
        CommandId::GW_INITIALIZE_SCENE_NTF => {
            require_exact(payload, 26, command)?;
            let mut node_states = [false; 25];
            for (state, value) in node_states.iter_mut().zip(&payload[1..]) {
                *state = *value == 0;
            }
            Ok(Response::SceneInitializationResult(SceneInitializationResult {
                status: payload[0],
                node_states,
            }))
        }
        CommandId::GW_RECORD_SCENE_NTF
        | CommandId::GW_DELETE_SCENE_CFM
        | CommandId::GW_RENAME_SCENE_CFM
        | CommandId::GW_GET_SCENE_INFOAMATION_CFM => {
            require_exact(payload, 2, command)?;
            Ok(Response::SceneObjectResult(SceneObjectResult {
                command,
                status: payload[0],
                scene_id: payload[1],
            }))
        }
        CommandId::GW_GET_SCENE_LIST_CFM => {
            require_exact(payload, 1, command)?;
            Ok(Response::SceneListAccepted {
                total_scenes: payload[0],
            })
        }
        CommandId::GW_GET_SCENE_LIST_NTF => decode_scene_list(payload, command),
        CommandId::GW_GET_SCENE_INFOAMATION_NTF => decode_scene_information(payload, command),
        CommandId::GW_ACTIVATE_SCENE_CFM | CommandId::GW_STOP_SCENE_CFM => {
            require_exact(payload, 3, command)?;
            Ok(Response::SceneSessionResult(SceneSessionResult {
                command,
                status: payload[0],
                session_id: session_id_at(payload, 1, command)?,
            }))
        }
        CommandId::GW_SCENE_INFORMATION_CHANGED_NTF => {
            require_exact(payload, 2, command)?;
            Ok(Response::SceneChange(SceneChange {
                change_type: payload[0],
                scene_id: payload[1],
            }))
        }
        CommandId::GW_ACTIVATE_PRODUCTGROUP_CFM => {
            require_exact(payload, 3, command)?;
            Ok(Response::ProductGroupResult(ProductGroupResult {
                session_id: session_id(payload, command)?,
                status: payload[2],
            }))
        }
        CommandId::GW_ACTIVATE_PRODUCTGROUP_NTF => {
            Ok(Response::ProductGroupNotification(Bytes::copy_from_slice(payload)))
        }
        CommandId::GW_GET_CONTACT_INPUT_LINK_LIST_CFM => decode_contact_input_links(payload, command),
        CommandId::GW_SET_CONTACT_INPUT_LINK_CFM | CommandId::GW_REMOVE_CONTACT_INPUT_LINK_CFM => {
            require_exact(payload, 2, command)?;
            Ok(Response::ContactInputOperationResult(ContactInputOperationResult {
                command,
                contact_input_id: payload[0],
                status: payload[1],
            }))
        }
        _ => Ok(Response::Unknown {
            command,
            payload: Bytes::copy_from_slice(payload),
        }),
    }
}

fn decode_scene_list(payload: &[u8], command: CommandId) -> Result<Response> {
    require(payload, 2, command)?;
    let count = usize::from(payload[0]);
    if count > 3 {
        return Err(KlfError::InvalidRequest {
            message: "scene-list notification contains more than three scenes",
        });
    }
    let expected = 2 + count * 65;
    require_exact(payload, expected, command)?;
    let scenes = (0..count)
        .map(|index| {
            let offset = 1 + index * 65;
            Ok(SceneSummary {
                scene_id: payload[offset],
                name: decode_fixed_string(&payload[offset + 1..offset + 65])?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Response::SceneList(SceneList {
        scenes,
        remaining_scenes: payload[expected - 1],
    }))
}

fn decode_scene_information(payload: &[u8], command: CommandId) -> Result<Response> {
    require(payload, 67, command)?;
    let count = usize::from(payload[65]);
    if count > 45 {
        return Err(KlfError::InvalidRequest {
            message: "scene-information notification contains more than 45 nodes",
        });
    }
    let expected = 67 + count * 4;
    require_exact(payload, expected, command)?;
    let nodes = (0..count)
        .map(|index| {
            let offset = 66 + index * 4;
            SceneNode {
                node_id: NodeId::new(payload[offset]),
                parameter_id: payload[offset + 1],
                parameter: StandardParameter::from_raw(read_u16(payload, offset + 2)),
            }
        })
        .collect();
    Ok(Response::SceneInformation(SceneInformation {
        scene_id: payload[0],
        name: decode_fixed_string(&payload[1..65])?,
        nodes,
        remaining_nodes: payload[expected - 1],
    }))
}

fn decode_contact_input_links(payload: &[u8], command: CommandId) -> Result<Response> {
    require(payload, 1, command)?;
    let count = usize::from(payload[0]);
    if count > 10 {
        return Err(KlfError::InvalidRequest {
            message: "contact-input list contains more than ten links",
        });
    }
    let expected = 1 + count * 17;
    require_exact(payload, expected, command)?;
    let links = (0..count)
        .map(|index| decode_contact_input_link(payload, 1 + index * 17))
        .collect();
    Ok(Response::ContactInputLinks(links))
}

fn decode_contact_input_link(payload: &[u8], offset: usize) -> ContactInputLink {
    ContactInputLink {
        contact_input_id: payload[offset],
        assignment: payload[offset + 1],
        action_id: payload[offset + 2],
        command_originator: payload[offset + 3],
        priority_level: payload[offset + 4],
        parameter_id: payload[offset + 5],
        position: StandardParameter::from_raw(read_u16(payload, offset + 6)),
        velocity: payload[offset + 8],
        lock_priority_level: payload[offset + 9],
        priority_level_settings: [
            payload[offset + 10],
            payload[offset + 11],
            payload[offset + 12],
            payload[offset + 13],
            payload[offset + 14],
        ],
        success_output_id: payload[offset + 15],
        error_output_id: payload[offset + 16],
    }
}

fn decode_command_extension(command: CommandId, payload: &[u8]) -> Result<Response> {
    match command {
        CommandId::GW_WINK_SEND_CFM
        | CommandId::GW_SET_LIMITATION_CFM
        | CommandId::GW_GET_LIMITATION_STATUS_CFM
        | CommandId::GW_MODE_SEND_CFM => {
            require_exact(payload, 3, command)?;
            Ok(Response::SessionCommandResult(SessionCommandResult {
                command,
                session_id: session_id(payload, command)?,
                status: payload[2],
            }))
        }
        CommandId::GW_WINK_SEND_NTF => {
            require_exact(payload, 2, command)?;
            Ok(Response::WinkFinished {
                session_id: session_id(payload, command)?,
            })
        }
        CommandId::GW_LIMITATION_STATUS_NTF => {
            require_exact(payload, 10, command)?;
            Ok(Response::LimitationStatus(LimitationStatus {
                session_id: session_id(payload, command)?,
                node_id: NodeId::new(payload[2]),
                parameter_id: payload[3],
                minimum: StandardParameter::from_raw(read_u16(payload, 4)),
                maximum: StandardParameter::from_raw(read_u16(payload, 6)),
                originator: payload[8],
                limitation_time: payload[9],
            }))
        }
        CommandId::GW_MODE_SEND_NTF => Ok(Response::ModeNotification(Bytes::copy_from_slice(payload))),
        _ => Ok(Response::Unknown {
            command,
            payload: Bytes::copy_from_slice(payload),
        }),
    }
}

fn decode_node_group(command: CommandId, payload: &[u8]) -> Result<Response> {
    match command {
        CommandId::GW_SET_NODE_VARIATION_CFM
        | CommandId::GW_SET_NODE_NAME_CFM
        | CommandId::GW_SET_NODE_ORDER_AND_PLACEMENT_CFM => {
            require_exact(payload, 2, command)?;
            Ok(Response::NodeMutationResult(NodeMutationResult {
                command,
                status: payload[0],
                node_id: NodeId::new(payload[1]),
            }))
        }
        CommandId::GW_GET_GROUP_INFORMATION_CFM
        | CommandId::GW_SET_GROUP_INFORMATION_CFM
        | CommandId::GW_NEW_GROUP_CFM => {
            require_exact(payload, 2, command)?;
            Ok(Response::GroupOperationResult(GroupOperationResult {
                command,
                status: payload[0],
                group_id: GroupId::new(payload[1]),
            }))
        }
        CommandId::GW_DELETE_GROUP_CFM => {
            require_exact(payload, 2, command)?;
            Ok(Response::GroupOperationResult(GroupOperationResult {
                command,
                status: payload[1],
                group_id: GroupId::new(payload[0]),
            }))
        }
        CommandId::GW_GET_GROUP_INFORMATION_NTF | CommandId::GW_GET_ALL_GROUPS_INFORMATION_NTF => {
            Ok(Response::GroupInformation(GroupInformationNotification {
                command,
                information: decode_group_information(payload, command)?,
            }))
        }
        CommandId::GW_GROUP_INFORMATION_CHANGED_NTF => {
            require(payload, 2, command)?;
            let change_type = payload[0];
            let group_id = GroupId::new(payload[1]);
            let information = if change_type == 1 {
                require_exact(payload, 100, command)?;
                Some(decode_group_information(&payload[1..], command)?)
            } else {
                require_exact(payload, 2, command)?;
                None
            };
            Ok(Response::GroupChange(GroupChange {
                change_type,
                group_id,
                information,
            }))
        }
        CommandId::GW_GET_ALL_GROUPS_INFORMATION_CFM => {
            require_exact(payload, 2, command)?;
            Ok(Response::AllGroupsAccepted(AllGroupsAccepted {
                status: i8::from_be_bytes([payload[0]]),
                total_groups: payload[1],
            }))
        }
        CommandId::GW_GET_ALL_GROUPS_INFORMATION_FINISHED_NTF => {
            require_exact(payload, 0, command)?;
            Ok(Response::Acknowledgement { command })
        }
        CommandId::GW_GROUP_DELETED_NTF => {
            require_exact(payload, 1, command)?;
            Ok(Response::GroupDeleted(GroupId::new(payload[0])))
        }
        _ => Ok(Response::Unknown {
            command,
            payload: Bytes::copy_from_slice(payload),
        }),
    }
}

fn decode_group_information(payload: &[u8], command: CommandId) -> Result<GroupInformation> {
    require_exact(payload, 99, command)?;
    let mut actuator_bytes = [0; 25];
    actuator_bytes.copy_from_slice(&payload[72..97]);
    Ok(GroupInformation {
        group_id: GroupId::new(payload[0]),
        order: read_u16(payload, 1),
        placement: payload[3],
        name: decode_fixed_string(&payload[4..68])?,
        velocity: payload[68],
        node_variation: payload[69],
        group_type: payload[70],
        object_count: payload[71],
        actuators: ActuatorSet::from_bytes(actuator_bytes),
        revision: read_u16(payload, 97),
    })
}

fn decode_configuration(command: CommandId, payload: &[u8]) -> Result<Response> {
    match command {
        CommandId::GW_CS_GET_SYSTEMTABLE_DATA_CFM
        | CommandId::GW_CS_DISCOVER_NODES_CFM
        | CommandId::GW_CS_VIRGIN_STATE_CFM
        | CommandId::GW_CS_CONTROLLER_COPY_CFM
        | CommandId::GW_CS_CONTROLLER_COPY_CANCEL_NTF
        | CommandId::GW_CS_RECEIVE_KEY_CFM
        | CommandId::GW_CS_GENERATE_NEW_KEY_CFM
        | CommandId::GW_CS_REPAIR_KEY_CFM => {
            require_exact(payload, 0, command)?;
            Ok(Response::Acknowledgement { command })
        }
        CommandId::GW_CS_GET_SYSTEMTABLE_DATA_NTF => decode_system_table_data(payload, command),
        CommandId::GW_CS_DISCOVER_NODES_NTF => {
            require_exact(payload, 131, command)?;
            Ok(Response::NodeDiscoveryResult(NodeDiscoveryResult {
                added: decode_node_set(payload, 0),
                rf_connection_error: decode_node_set(payload, 26),
                key_error_existing_node: decode_node_set(payload, 52),
                removed: decode_node_set(payload, 78),
                open: decode_node_set(payload, 104),
                status: payload[130],
            }))
        }
        CommandId::GW_CS_REMOVE_NODES_CFM => {
            require_exact(payload, 1, command)?;
            Ok(Response::NodesRemoved {
                scene_deleted: payload[0] != 0,
            })
        }
        CommandId::GW_CS_CONTROLLER_COPY_NTF => {
            require_exact(payload, 2, command)?;
            Ok(Response::ControllerCopyResult(ControllerCopyResult {
                mode: payload[0],
                status: payload[1],
            }))
        }
        CommandId::GW_CS_RECEIVE_KEY_NTF | CommandId::GW_CS_GENERATE_NEW_KEY_NTF | CommandId::GW_CS_REPAIR_KEY_NTF => {
            require_exact(payload, 53, command)?;
            Ok(Response::KeyChangeResult(KeyChangeResult {
                command,
                status: i8::from_be_bytes([payload[0]]),
                changed: decode_node_set(payload, 1),
                unchanged: decode_node_set(payload, 27),
            }))
        }
        CommandId::GW_CS_PGC_JOB_NTF => {
            require_exact(payload, 3, command)?;
            Ok(Response::PgcJob(PgcJob {
                state: i8::from_be_bytes([payload[0]]),
                status: i8::from_be_bytes([payload[1]]),
                job_type: i8::from_be_bytes([payload[2]]),
            }))
        }
        CommandId::GW_CS_SYSTEM_TABLE_UPDATE_NTF => {
            require_exact(payload, 52, command)?;
            Ok(Response::SystemTableUpdate(SystemTableUpdate {
                added: decode_node_set(payload, 0),
                removed: decode_node_set(payload, 26),
            }))
        }
        CommandId::GW_CS_ACTIVATE_CONFIGURATION_MODE_CFM => {
            require_exact(payload, 79, command)?;
            Ok(Response::ConfigurationActivationResult(ConfigurationActivationResult {
                activated: decode_node_set(payload, 0),
                no_contact: decode_node_set(payload, 26),
                other_error: decode_node_set(payload, 52),
                status: i8::from_be_bytes([payload[78]]),
            }))
        }
        _ => Ok(Response::Unknown {
            command,
            payload: Bytes::copy_from_slice(payload),
        }),
    }
}

fn decode_system_table_data(payload: &[u8], command: CommandId) -> Result<Response> {
    require(payload, 2, command)?;
    let count = usize::from(payload[0]);
    let expected = 2 + count * 11;
    require_exact(payload, expected, command)?;
    let entries = (0..count)
        .map(|index| {
            let offset = 1 + index * 11;
            SystemTableObject {
                index: payload[offset],
                actuator_address: [payload[offset + 1], payload[offset + 2], payload[offset + 3]],
                actuator_type_sub_type: read_u16(payload, offset + 4),
                power_mode_and_flags: payload[offset + 6],
                manufacturer_id: payload[offset + 7],
                backbone_reference: [payload[offset + 8], payload[offset + 9], payload[offset + 10]],
            }
        })
        .collect();
    Ok(Response::SystemTableData(SystemTableData {
        entries,
        remaining_entries: payload[expected - 1],
    }))
}

fn decode_node_set(payload: &[u8], offset: usize) -> NodeSet {
    let mut actuators = [0; 25];
    actuators.copy_from_slice(&payload[offset..offset + 25]);
    NodeSet::new(
        ActuatorSet::from_bytes(actuators),
        BeaconSet::from_byte(payload[offset + 25]),
    )
}

fn decode_gateway(command: CommandId, payload: &[u8]) -> Result<Response> {
    match command {
        CommandId::GW_ERROR_NTF => {
            require_exact(payload, 1, command)?;
            Ok(Response::ErrorNotification {
                error_number: payload[0],
            })
        }
        CommandId::GW_SET_NETWORK_SETUP_CFM | CommandId::GW_SET_UTC_CFM => {
            require_exact(payload, 0, command)?;
            Ok(Response::Acknowledgement { command })
        }
        CommandId::GW_PASSWORD_ENTER_CFM => {
            require_exact(payload, 1, command)?;
            Ok(Response::PasswordEntered {
                accepted: payload[0] == 0,
            })
        }
        CommandId::GW_PASSWORD_CHANGE_CFM => {
            require_exact(payload, 1, command)?;
            Ok(Response::PasswordChanged {
                accepted: payload[0] == 0,
            })
        }
        CommandId::GW_PASSWORD_CHANGE_NTF => {
            require_exact(payload, 32, command)?;
            Ok(Response::PasswordChangedNotification(PasswordChangedNotification {
                new_password: decode_fixed_string(payload)?,
            }))
        }
        CommandId::GW_GET_VERSION_CFM => decode_version(payload, command).map(Response::Version),
        CommandId::GW_GET_PROTOCOL_VERSION_CFM => {
            require_exact(payload, 4, command)?;
            Ok(Response::ProtocolVersion(ProtocolVersion {
                major: read_u16(payload, 0),
                minor: read_u16(payload, 2),
            }))
        }
        CommandId::GW_GET_STATE_CFM => {
            require_exact(payload, 6, command)?;
            Ok(Response::GatewayState(GatewayState {
                state: payload[0],
                sub_state: payload[1],
                data: [payload[2], payload[3], payload[4], payload[5]],
            }))
        }
        CommandId::GW_LEAVE_LEARN_STATE_CFM | CommandId::GW_RTC_SET_TIME_ZONE_CFM => {
            require_exact(payload, 1, command)?;
            Ok(Response::OperationStatus {
                command,
                status: payload[0],
            })
        }
        CommandId::GW_GET_NETWORK_SETUP_CFM => {
            require_exact(payload, 13, command)?;
            Ok(Response::NetworkSetup(NetworkSetup {
                ip_address: [payload[0], payload[1], payload[2], payload[3]],
                subnet_mask: [payload[4], payload[5], payload[6], payload[7]],
                default_gateway: [payload[8], payload[9], payload[10], payload[11]],
                dhcp: payload[12] == 1,
            }))
        }
        CommandId::GW_GET_LOCAL_TIME_CFM => {
            require_exact(payload, 15, command)?;
            Ok(Response::LocalTime(LocalTime {
                utc_time: ProtocolTimestamp::from_unix_seconds(read_u32(payload, 0)),
                second: payload[4],
                minute: payload[5],
                hour: payload[6],
                day_of_month: payload[7],
                month_since_january: payload[8],
                years_since_1900: i16::from_be_bytes([payload[9], payload[10]]),
                week_day: payload[11],
                day_of_year: read_u16(payload, 12),
                daylight_saving_flag: i8::from_be_bytes([payload[14]]),
            }))
        }
        _ => Ok(Response::Unknown {
            command,
            payload: Bytes::copy_from_slice(payload),
        }),
    }
}

fn decode_version(payload: &[u8], command: CommandId) -> Result<Version> {
    require_exact(payload, 9, command)?;
    let mut software = [0; 6];
    software.copy_from_slice(&payload[..6]);
    Ok(Version {
        software,
        hardware: payload[6],
        product_group: payload[7],
        product_type: payload[8],
    })
}

fn decode_node_information(payload: &[u8], command: CommandId) -> Result<NodeInformation> {
    require_exact(payload, 124, command)?;
    let alias_count = usize::from(payload[103]);
    if alias_count > 5 {
        return Err(KlfError::InvalidRequest {
            message: "node information contains more than five aliases",
        });
    }
    let mut serial_number = [0; 8];
    serial_number.copy_from_slice(&payload[76..84]);
    let aliases = (0..alias_count)
        .map(|index| {
            let offset = 104 + index * 4;
            Alias {
                kind: read_u16(payload, offset),
                value: read_u16(payload, offset + 2),
            }
        })
        .collect();

    Ok(NodeInformation {
        node_id: NodeId::new(payload[0]),
        order: read_u16(payload, 1),
        placement: payload[3],
        name: decode_fixed_string(&payload[4..68])?,
        velocity: payload[68],
        node_type_sub_type: read_u16(payload, 69),
        product_group: i8::from_be_bytes([payload[71]]),
        product_type: i8::from_be_bytes([payload[72]]),
        variation: payload[73],
        power_mode: payload[74],
        build_number: payload[75],
        serial_number,
        operating_state: payload[84].into(),
        current_position: StandardParameter::from_raw(read_u16(payload, 85)),
        target_position: StandardParameter::from_raw(read_u16(payload, 87)),
        functional_positions: [
            StandardParameter::from_raw(read_u16(payload, 89)),
            StandardParameter::from_raw(read_u16(payload, 91)),
            StandardParameter::from_raw(read_u16(payload, 93)),
            StandardParameter::from_raw(read_u16(payload, 95)),
        ],
        remaining_time: read_u16(payload, 97),
        timestamp: ProtocolTimestamp::from_unix_seconds(read_u32(payload, 99)),
        aliases,
    })
}

fn decode_node_state(payload: &[u8], command: CommandId) -> Result<NodeStatePosition> {
    require_exact(payload, 20, command)?;
    Ok(NodeStatePosition {
        node_id: NodeId::new(payload[0]),
        operating_state: payload[1].into(),
        current_position: StandardParameter::from_raw(read_u16(payload, 2)),
        target_position: StandardParameter::from_raw(read_u16(payload, 4)),
        functional_positions: [
            StandardParameter::from_raw(read_u16(payload, 6)),
            StandardParameter::from_raw(read_u16(payload, 8)),
            StandardParameter::from_raw(read_u16(payload, 10)),
            StandardParameter::from_raw(read_u16(payload, 12)),
        ],
        remaining_time: read_u16(payload, 14),
        timestamp: ProtocolTimestamp::from_unix_seconds(read_u32(payload, 16)),
    })
}

fn decode_command_run_status(payload: &[u8], command: CommandId) -> Result<CommandRunStatus> {
    require_exact(payload, 13, command)?;
    Ok(CommandRunStatus {
        session_id: session_id(payload, command)?,
        status_id: payload[2],
        node_id: NodeId::new(payload[3]),
        node_parameter: payload[4],
        parameter_value: StandardParameter::from_raw(read_u16(payload, 5)),
        run_status: payload[7].into(),
        status_reply: payload[8],
        information_code: read_u32(payload, 9),
    })
}

fn decode_status_notification(payload: &[u8], command: CommandId) -> Result<StatusNotification> {
    require(payload, 7, command)?;
    let status_type = payload[6];
    let detail = match status_type {
        0..=2 => {
            require(payload, 8, command)?;
            let count = usize::from(payload[7]);
            require_exact(payload, 8 + count * 3, command)?;
            let values = (0..count)
                .map(|index| {
                    let offset = 8 + index * 3;
                    ParameterStatus {
                        node_parameter: payload[offset],
                        value: read_u16(payload, offset + 1),
                    }
                })
                .collect();
            StatusNotificationDetail::Parameters(values)
        }
        3 => {
            require_exact(payload, 18, command)?;
            StatusNotificationDetail::Main {
                target_position: StandardParameter::from_raw(read_u16(payload, 7)),
                current_position: StandardParameter::from_raw(read_u16(payload, 9)),
                remaining_time: read_u16(payload, 11),
                last_master_execution_address: read_u32(payload, 13),
                last_command_originator: payload[17],
            }
        }
        _ => StatusNotificationDetail::Unknown {
            status_type,
            payload: Bytes::copy_from_slice(&payload[7..]),
        },
    };

    Ok(StatusNotification {
        session_id: session_id(payload, command)?,
        status_id: payload[2],
        node_id: NodeId::new(payload[3]),
        run_status: payload[4].into(),
        status_reply: payload[5],
        detail,
    })
}

fn session_id(payload: &[u8], command: CommandId) -> Result<SessionId> {
    require(payload, 2, command)?;
    session_id_at(payload, 0, command)
}

fn session_id_at(payload: &[u8], offset: usize, command: CommandId) -> Result<SessionId> {
    require(payload, offset + 2, command)?;
    SessionId::new(read_u16(payload, offset)).ok_or(KlfError::InvalidRequest {
        message: "session ID zero is reserved",
    })
}

fn require(payload: &[u8], expected: usize, command: CommandId) -> Result<()> {
    if payload.len() < expected {
        Err(KlfError::TruncatedPayload {
            command,
            expected,
            actual: payload.len(),
        })
    } else {
        Ok(())
    }
}

fn require_exact(payload: &[u8], expected: usize, command: CommandId) -> Result<()> {
    match payload.len().cmp(&expected) {
        std::cmp::Ordering::Less => Err(KlfError::TruncatedPayload {
            command,
            expected,
            actual: payload.len(),
        }),
        std::cmp::Ordering::Equal => Ok(()),
        std::cmp::Ordering::Greater => Err(KlfError::InvalidPayloadLength {
            command,
            expected,
            actual: payload.len(),
        }),
    }
}

fn read_u16(payload: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes([payload[offset], payload[offset + 1]])
}

fn read_u32(payload: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes([
        payload[offset],
        payload[offset + 1],
        payload[offset + 2],
        payload[offset + 3],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::klf200::Percentage;

    #[test]
    fn decodes_version_and_protocol_version_vectors() {
        let version = Response::decode(Frame::new(
            CommandId::GW_GET_VERSION_CFM,
            [0, 2, 0, 0, 71, 0, 6, 14, 3].as_slice(),
        ))
        .expect("decode version");
        assert_eq!(
            version,
            Response::Version(Version {
                software: [0, 2, 0, 0, 71, 0],
                hardware: 6,
                product_group: 14,
                product_type: 3,
            })
        );

        let protocol = Response::decode(Frame::new(
            CommandId::GW_GET_PROTOCOL_VERSION_CFM,
            [0, 3, 0, 18].as_slice(),
        ))
        .expect("decode protocol version");
        assert_eq!(
            protocol,
            Response::ProtocolVersion(ProtocolVersion { major: 3, minor: 18 })
        );
    }

    #[test]
    fn decodes_gateway_network_time_and_password_vectors() {
        assert_eq!(
            Response::decode(Frame::new(CommandId::GW_LEAVE_LEARN_STATE_CFM, [1].as_slice()))
                .expect("decode leave learn state"),
            Response::OperationStatus {
                command: CommandId::GW_LEAVE_LEARN_STATE_CFM,
                status: 1,
            }
        );
        assert_eq!(
            Response::decode(Frame::new(
                CommandId::GW_GET_NETWORK_SETUP_CFM,
                [192, 168, 1, 78, 255, 255, 255, 0, 192, 168, 1, 1, 1].as_slice(),
            ))
            .expect("decode network setup"),
            Response::NetworkSetup(NetworkSetup {
                ip_address: [192, 168, 1, 78],
                subnet_mask: [255, 255, 255, 0],
                default_gateway: [192, 168, 1, 1],
                dhcp: true,
            })
        );
        for command in [CommandId::GW_SET_NETWORK_SETUP_CFM, CommandId::GW_SET_UTC_CFM] {
            assert_eq!(
                Response::decode(Frame::new(command, Bytes::new())).expect("decode empty confirmation"),
                Response::Acknowledgement { command }
            );
        }
        assert_eq!(
            Response::decode(Frame::new(CommandId::GW_RTC_SET_TIME_ZONE_CFM, [1].as_slice()))
                .expect("decode time-zone status"),
            Response::OperationStatus {
                command: CommandId::GW_RTC_SET_TIME_ZONE_CFM,
                status: 1,
            }
        );

        let local_payload = [0x65, 0x53, 0xF1, 0x00, 59, 58, 23, 31, 11, 0, 123, 0, 1, 0x6C, 0xFF];
        assert_eq!(
            Response::decode(Frame::new(CommandId::GW_GET_LOCAL_TIME_CFM, local_payload.to_vec()))
                .expect("decode local time"),
            Response::LocalTime(LocalTime {
                utc_time: ProtocolTimestamp::from_unix_seconds(0x6553_F100),
                second: 59,
                minute: 58,
                hour: 23,
                day_of_month: 31,
                month_since_january: 11,
                years_since_1900: 123,
                week_day: 0,
                day_of_year: 364,
                daylight_saving_flag: -1,
            })
        );
        assert_eq!(
            Response::decode(Frame::new(CommandId::GW_PASSWORD_CHANGE_CFM, [0].as_slice()))
                .expect("decode password-change confirmation"),
            Response::PasswordChanged { accepted: true }
        );
        let mut changed_payload = [0; 32];
        changed_payload[..11].copy_from_slice(b"replacement");
        let changed = Response::decode(Frame::new(CommandId::GW_PASSWORD_CHANGE_NTF, changed_payload.to_vec()))
            .expect("decode password-change notification");
        assert_eq!(
            changed,
            Response::PasswordChangedNotification(PasswordChangedNotification {
                new_password: "replacement".to_owned(),
            })
        );
        assert_eq!(
            format!("{changed:?}"),
            "PasswordChangedNotification(PasswordChangedNotification([REDACTED]))"
        );
    }

    #[test]
    fn rejects_malformed_gateway_network_time_and_password_payloads() {
        let fixed_lengths = [
            (CommandId::GW_LEAVE_LEARN_STATE_CFM, 1),
            (CommandId::GW_GET_NETWORK_SETUP_CFM, 13),
            (CommandId::GW_SET_NETWORK_SETUP_CFM, 0),
            (CommandId::GW_SET_UTC_CFM, 0),
            (CommandId::GW_RTC_SET_TIME_ZONE_CFM, 1),
            (CommandId::GW_GET_LOCAL_TIME_CFM, 15),
            (CommandId::GW_PASSWORD_CHANGE_CFM, 1),
            (CommandId::GW_PASSWORD_CHANGE_NTF, 32),
        ];
        for (command, expected) in fixed_lengths {
            if expected > 0 {
                let actual = expected - 1;
                assert_eq!(
                    Response::decode(Frame::new(command, vec![0; actual])),
                    Err(KlfError::TruncatedPayload {
                        command,
                        expected,
                        actual,
                    })
                );
            }
            assert_eq!(
                Response::decode(Frame::new(command, vec![0; expected + 1])),
                Err(KlfError::InvalidPayloadLength {
                    command,
                    expected,
                    actual: expected + 1,
                })
            );
        }
    }

    #[test]
    fn decodes_configuration_service_table_and_discovery_vectors() {
        let empty_confirmations = [
            CommandId::GW_CS_GET_SYSTEMTABLE_DATA_CFM,
            CommandId::GW_CS_DISCOVER_NODES_CFM,
            CommandId::GW_CS_VIRGIN_STATE_CFM,
            CommandId::GW_CS_CONTROLLER_COPY_CFM,
            CommandId::GW_CS_CONTROLLER_COPY_CANCEL_NTF,
            CommandId::GW_CS_RECEIVE_KEY_CFM,
            CommandId::GW_CS_GENERATE_NEW_KEY_CFM,
            CommandId::GW_CS_REPAIR_KEY_CFM,
        ];
        for command in empty_confirmations {
            assert_eq!(
                Response::decode(Frame::new(command, Bytes::new())).expect("decode configuration acknowledgement"),
                Response::Acknowledgement { command }
            );
        }

        let table = [1, 7, 1, 2, 3, 0x12, 0x34, 0xCD, 0x2A, 4, 5, 6, 9];
        assert_eq!(
            Response::decode(Frame::new(CommandId::GW_CS_GET_SYSTEMTABLE_DATA_NTF, table.to_vec()))
                .expect("decode system table"),
            Response::SystemTableData(SystemTableData {
                entries: vec![SystemTableObject {
                    index: 7,
                    actuator_address: [1, 2, 3],
                    actuator_type_sub_type: 0x1234,
                    power_mode_and_flags: 0xCD,
                    manufacturer_id: 0x2A,
                    backbone_reference: [4, 5, 6],
                }],
                remaining_entries: 9,
            })
        );

        let mut discovery = vec![0; 131];
        discovery[0] = 1;
        discovery[25] = 2;
        discovery[26] = 3;
        discovery[52] = 4;
        discovery[78] = 5;
        discovery[104] = 6;
        discovery[130] = 7;
        let Response::NodeDiscoveryResult(result) =
            Response::decode(Frame::new(CommandId::GW_CS_DISCOVER_NODES_NTF, discovery))
                .expect("decode discovery result")
        else {
            panic!("expected discovery result");
        };
        assert!(result.added.actuators.contains(0));
        assert!(result.added.beacons.contains(1));
        assert_eq!(result.rf_connection_error.actuators.as_bytes()[0], 3);
        assert_eq!(result.key_error_existing_node.actuators.as_bytes()[0], 4);
        assert_eq!(result.removed.actuators.as_bytes()[0], 5);
        assert_eq!(result.open.actuators.as_bytes()[0], 6);
        assert_eq!(result.status, 7);
    }

    #[test]
    fn decodes_configuration_service_job_and_change_vectors() {
        assert_eq!(
            Response::decode(Frame::new(CommandId::GW_CS_REMOVE_NODES_CFM, [1].as_slice()))
                .expect("decode remove result"),
            Response::NodesRemoved { scene_deleted: true }
        );
        assert_eq!(
            Response::decode(Frame::new(CommandId::GW_CS_CONTROLLER_COPY_NTF, [1, 4].as_slice()))
                .expect("decode controller copy"),
            Response::ControllerCopyResult(ControllerCopyResult { mode: 1, status: 4 })
        );
        for command in [
            CommandId::GW_CS_RECEIVE_KEY_NTF,
            CommandId::GW_CS_GENERATE_NEW_KEY_NTF,
            CommandId::GW_CS_REPAIR_KEY_NTF,
        ] {
            let mut payload = vec![0; 53];
            payload[0] = 0xFF;
            payload[1] = 1;
            payload[26] = 2;
            payload[27] = 4;
            let Response::KeyChangeResult(result) =
                Response::decode(Frame::new(command, payload)).expect("decode key change")
            else {
                panic!("expected key change result");
            };
            assert_eq!(result.command, command);
            assert_eq!(result.status, -1);
            assert_eq!(result.changed.actuators.as_bytes()[0], 1);
            assert!(result.changed.beacons.contains(1));
            assert_eq!(result.unchanged.actuators.as_bytes()[0], 4);
        }
        assert_eq!(
            Response::decode(Frame::new(CommandId::GW_CS_PGC_JOB_NTF, [0, 2, 3].as_slice())).expect("decode PGC job"),
            Response::PgcJob(PgcJob {
                state: 0,
                status: 2,
                job_type: 3,
            })
        );

        let mut update = vec![0; 52];
        update[0] = 1;
        update[26] = 2;
        let Response::SystemTableUpdate(update) =
            Response::decode(Frame::new(CommandId::GW_CS_SYSTEM_TABLE_UPDATE_NTF, update))
                .expect("decode table update")
        else {
            panic!("expected table update");
        };
        assert_eq!(update.added.actuators.as_bytes()[0], 1);
        assert_eq!(update.removed.actuators.as_bytes()[0], 2);

        let mut activation = vec![0; 79];
        activation[0] = 1;
        activation[26] = 2;
        activation[52] = 4;
        activation[78] = 0xFF;
        let Response::ConfigurationActivationResult(result) =
            Response::decode(Frame::new(CommandId::GW_CS_ACTIVATE_CONFIGURATION_MODE_CFM, activation))
                .expect("decode configuration activation")
        else {
            panic!("expected configuration activation");
        };
        assert_eq!(result.activated.actuators.as_bytes()[0], 1);
        assert_eq!(result.no_contact.actuators.as_bytes()[0], 2);
        assert_eq!(result.other_error.actuators.as_bytes()[0], 4);
        assert_eq!(result.status, -1);
    }

    #[test]
    fn rejects_malformed_configuration_service_payloads() {
        let fixed_lengths = [
            (CommandId::GW_CS_GET_SYSTEMTABLE_DATA_CFM, 0),
            (CommandId::GW_CS_DISCOVER_NODES_CFM, 0),
            (CommandId::GW_CS_DISCOVER_NODES_NTF, 131),
            (CommandId::GW_CS_REMOVE_NODES_CFM, 1),
            (CommandId::GW_CS_VIRGIN_STATE_CFM, 0),
            (CommandId::GW_CS_CONTROLLER_COPY_CFM, 0),
            (CommandId::GW_CS_CONTROLLER_COPY_NTF, 2),
            (CommandId::GW_CS_CONTROLLER_COPY_CANCEL_NTF, 0),
            (CommandId::GW_CS_RECEIVE_KEY_CFM, 0),
            (CommandId::GW_CS_RECEIVE_KEY_NTF, 53),
            (CommandId::GW_CS_PGC_JOB_NTF, 3),
            (CommandId::GW_CS_SYSTEM_TABLE_UPDATE_NTF, 52),
            (CommandId::GW_CS_GENERATE_NEW_KEY_CFM, 0),
            (CommandId::GW_CS_GENERATE_NEW_KEY_NTF, 53),
            (CommandId::GW_CS_REPAIR_KEY_CFM, 0),
            (CommandId::GW_CS_REPAIR_KEY_NTF, 53),
            (CommandId::GW_CS_ACTIVATE_CONFIGURATION_MODE_CFM, 79),
        ];
        for (command, expected) in fixed_lengths {
            let actual = if expected == 0 { 1 } else { expected - 1 };
            let result = Response::decode(Frame::new(command, vec![0; actual]));
            if expected == 0 {
                assert!(matches!(result, Err(KlfError::InvalidPayloadLength { .. })));
            } else {
                assert!(matches!(result, Err(KlfError::TruncatedPayload { .. })));
            }
        }
        assert_eq!(
            Response::decode(Frame::new(CommandId::GW_CS_GET_SYSTEMTABLE_DATA_NTF, [1, 0].as_slice())),
            Err(KlfError::TruncatedPayload {
                command: CommandId::GW_CS_GET_SYSTEMTABLE_DATA_NTF,
                expected: 13,
                actual: 2,
            })
        );
    }

    fn group_information_payload() -> ([u8; 99], GroupInformation) {
        let mut actuator_bytes = [0; 25];
        actuator_bytes[0] = 0x81;
        actuator_bytes[24] = 0x80;
        let mut payload = [0; 99];
        payload[0..4].copy_from_slice(&[4, 0x12, 0x34, 7]);
        payload[4..16].copy_from_slice(b"West windows");
        payload[68..72].copy_from_slice(&[2, 3, 1, 3]);
        payload[72..97].copy_from_slice(&actuator_bytes);
        payload[97..99].copy_from_slice(&[0x45, 0x67]);
        let information = GroupInformation {
            group_id: GroupId::new(4),
            order: 0x1234,
            placement: 7,
            name: "West windows".to_owned(),
            velocity: 2,
            node_variation: 3,
            group_type: 1,
            object_count: 3,
            actuators: ActuatorSet::from_bytes(actuator_bytes),
            revision: 0x4567,
        };
        (payload, information)
    }

    #[test]
    fn decodes_node_and_group_status_vectors() {
        for command in [
            CommandId::GW_SET_NODE_VARIATION_CFM,
            CommandId::GW_SET_NODE_NAME_CFM,
            CommandId::GW_SET_NODE_ORDER_AND_PLACEMENT_CFM,
        ] {
            assert_eq!(
                Response::decode(Frame::new(command, [2, 9].as_slice())).expect("decode node mutation"),
                Response::NodeMutationResult(NodeMutationResult {
                    command,
                    status: 2,
                    node_id: NodeId::new(9),
                })
            );
        }

        for command in [
            CommandId::GW_GET_GROUP_INFORMATION_CFM,
            CommandId::GW_SET_GROUP_INFORMATION_CFM,
            CommandId::GW_NEW_GROUP_CFM,
        ] {
            assert_eq!(
                Response::decode(Frame::new(command, [1, 4].as_slice())).expect("decode group result"),
                Response::GroupOperationResult(GroupOperationResult {
                    command,
                    status: 1,
                    group_id: GroupId::new(4),
                })
            );
        }
        assert_eq!(
            Response::decode(Frame::new(CommandId::GW_DELETE_GROUP_CFM, [4, 2].as_slice()))
                .expect("decode delete group result"),
            Response::GroupOperationResult(GroupOperationResult {
                command: CommandId::GW_DELETE_GROUP_CFM,
                status: 2,
                group_id: GroupId::new(4),
            })
        );
    }

    #[test]
    fn decodes_group_notification_vectors() {
        let (payload, information) = group_information_payload();
        for command in [
            CommandId::GW_GET_GROUP_INFORMATION_NTF,
            CommandId::GW_GET_ALL_GROUPS_INFORMATION_NTF,
        ] {
            assert_eq!(
                Response::decode(Frame::new(command, payload.to_vec())).expect("decode group information"),
                Response::GroupInformation(GroupInformationNotification {
                    command,
                    information: information.clone(),
                })
            );
        }

        let mut changed_payload = [0; 100];
        changed_payload[0] = 1;
        changed_payload[1..].copy_from_slice(&payload);
        assert_eq!(
            Response::decode(Frame::new(
                CommandId::GW_GROUP_INFORMATION_CHANGED_NTF,
                changed_payload.to_vec(),
            ))
            .expect("decode changed group"),
            Response::GroupChange(GroupChange {
                change_type: 1,
                group_id: GroupId::new(4),
                information: Some(information),
            })
        );
        assert_eq!(
            Response::decode(Frame::new(
                CommandId::GW_GROUP_INFORMATION_CHANGED_NTF,
                [0, 4].as_slice(),
            ))
            .expect("decode deleted group change"),
            Response::GroupChange(GroupChange {
                change_type: 0,
                group_id: GroupId::new(4),
                information: None,
            })
        );
        assert_eq!(
            Response::decode(Frame::new(
                CommandId::GW_GET_ALL_GROUPS_INFORMATION_CFM,
                [0xFF, 200].as_slice(),
            ))
            .expect("decode all-groups confirmation"),
            Response::AllGroupsAccepted(AllGroupsAccepted {
                status: -1,
                total_groups: 200,
            })
        );
        assert_eq!(
            Response::decode(Frame::new(
                CommandId::GW_GET_ALL_GROUPS_INFORMATION_FINISHED_NTF,
                Bytes::new(),
            ))
            .expect("decode all-groups completion"),
            Response::Acknowledgement {
                command: CommandId::GW_GET_ALL_GROUPS_INFORMATION_FINISHED_NTF,
            }
        );
        assert_eq!(
            Response::decode(Frame::new(CommandId::GW_GROUP_DELETED_NTF, [4].as_slice()))
                .expect("decode group deletion"),
            Response::GroupDeleted(GroupId::new(4))
        );
    }

    #[test]
    fn rejects_malformed_node_and_group_payloads() {
        let cases = [
            (CommandId::GW_SET_NODE_VARIATION_CFM, 2),
            (CommandId::GW_SET_NODE_NAME_CFM, 2),
            (CommandId::GW_SET_NODE_ORDER_AND_PLACEMENT_CFM, 2),
            (CommandId::GW_GET_GROUP_INFORMATION_CFM, 2),
            (CommandId::GW_GET_GROUP_INFORMATION_NTF, 99),
            (CommandId::GW_SET_GROUP_INFORMATION_CFM, 2),
            (CommandId::GW_DELETE_GROUP_CFM, 2),
            (CommandId::GW_NEW_GROUP_CFM, 2),
            (CommandId::GW_GET_ALL_GROUPS_INFORMATION_CFM, 2),
            (CommandId::GW_GET_ALL_GROUPS_INFORMATION_NTF, 99),
            (CommandId::GW_GET_ALL_GROUPS_INFORMATION_FINISHED_NTF, 0),
            (CommandId::GW_GROUP_DELETED_NTF, 1),
        ];
        for (command, expected) in cases {
            if expected > 0 {
                assert!(matches!(
                    Response::decode(Frame::new(command, vec![0; expected - 1])),
                    Err(KlfError::TruncatedPayload { .. })
                ));
            }
            assert!(matches!(
                Response::decode(Frame::new(command, vec![0; expected + 1])),
                Err(KlfError::InvalidPayloadLength { .. })
            ));
        }

        let mut modified = vec![0; 99];
        modified[0] = 1;
        assert!(matches!(
            Response::decode(Frame::new(CommandId::GW_GROUP_INFORMATION_CHANGED_NTF, modified)),
            Err(KlfError::TruncatedPayload { .. })
        ));
        assert!(matches!(
            Response::decode(Frame::new(
                CommandId::GW_GROUP_INFORMATION_CHANGED_NTF,
                [0, 4, 0].as_slice(),
            )),
            Err(KlfError::InvalidPayloadLength { .. })
        ));
    }

    #[test]
    fn decodes_wink_limitation_and_mode_vectors() {
        let session_id = SessionId::new(0x1234).expect("nonzero");
        for (command, status) in [
            (CommandId::GW_WINK_SEND_CFM, 1),
            (CommandId::GW_SET_LIMITATION_CFM, 1),
            (CommandId::GW_GET_LIMITATION_STATUS_CFM, 1),
            (CommandId::GW_MODE_SEND_CFM, 0),
        ] {
            assert_eq!(
                Response::decode(Frame::new(command, vec![0x12, 0x34, status])).expect("decode session command result"),
                Response::SessionCommandResult(SessionCommandResult {
                    command,
                    session_id,
                    status,
                })
            );
        }
        assert_eq!(
            Response::decode(Frame::new(CommandId::GW_WINK_SEND_NTF, [0x12, 0x34].as_slice()))
                .expect("decode wink completion"),
            Response::WinkFinished { session_id }
        );
        assert_eq!(
            Response::decode(Frame::new(
                CommandId::GW_LIMITATION_STATUS_NTF,
                [0x12, 0x34, 9, 4, 0x32, 0, 0x96, 0, 1, 253].as_slice(),
            ))
            .expect("decode limitation status"),
            Response::LimitationStatus(LimitationStatus {
                session_id,
                node_id: NodeId::new(9),
                parameter_id: 4,
                minimum: StandardParameter::Relative(Percentage::from_percent(25)),
                maximum: StandardParameter::Relative(Percentage::from_percent(75)),
                originator: 1,
                limitation_time: 253,
            })
        );
        assert_eq!(
            Response::decode(Frame::new(CommandId::GW_MODE_SEND_NTF, [0x12, 0x34, 7].as_slice()))
                .expect("decode opaque mode notification"),
            Response::ModeNotification(Bytes::from_static(&[0x12, 0x34, 7]))
        );
    }

    #[test]
    fn rejects_malformed_wink_and_limitation_payloads() {
        for (command, expected) in [
            (CommandId::GW_WINK_SEND_CFM, 3),
            (CommandId::GW_WINK_SEND_NTF, 2),
            (CommandId::GW_SET_LIMITATION_CFM, 3),
            (CommandId::GW_GET_LIMITATION_STATUS_CFM, 3),
            (CommandId::GW_LIMITATION_STATUS_NTF, 10),
            (CommandId::GW_MODE_SEND_CFM, 3),
        ] {
            assert!(matches!(
                Response::decode(Frame::new(command, vec![0; expected - 1])),
                Err(KlfError::TruncatedPayload { .. })
            ));
            assert!(matches!(
                Response::decode(Frame::new(command, vec![0; expected + 1])),
                Err(KlfError::InvalidPayloadLength { .. })
            ));
        }
        assert_eq!(
            Response::decode(Frame::new(CommandId::GW_WINK_SEND_CFM, [0, 0, 1].as_slice())),
            Err(KlfError::InvalidRequest {
                message: "session ID zero is reserved",
            })
        );
    }

    #[test]
    fn decodes_scene_status_and_list_vectors() {
        for command in [
            CommandId::GW_INITIALIZE_SCENE_CFM,
            CommandId::GW_INITIALIZE_SCENE_CANCEL_CFM,
            CommandId::GW_RECORD_SCENE_CFM,
        ] {
            assert_eq!(
                Response::decode(Frame::new(command, [2].as_slice())).expect("decode scene status"),
                Response::SceneStatusResult(SceneStatusResult { command, status: 2 })
            );
        }

        let mut initialization = [1; 26];
        initialization[0] = 2;
        initialization[1] = 0;
        let mut node_states = [false; 25];
        node_states[0] = true;
        assert_eq!(
            Response::decode(Frame::new(CommandId::GW_INITIALIZE_SCENE_NTF, initialization.to_vec(),))
                .expect("decode scene initialization"),
            Response::SceneInitializationResult(SceneInitializationResult { status: 2, node_states })
        );

        for command in [
            CommandId::GW_RECORD_SCENE_NTF,
            CommandId::GW_DELETE_SCENE_CFM,
            CommandId::GW_RENAME_SCENE_CFM,
            CommandId::GW_GET_SCENE_INFOAMATION_CFM,
        ] {
            assert_eq!(
                Response::decode(Frame::new(command, [1, 4].as_slice())).expect("decode scene object status"),
                Response::SceneObjectResult(SceneObjectResult {
                    command,
                    status: 1,
                    scene_id: 4,
                })
            );
        }
        assert_eq!(
            Response::decode(Frame::new(CommandId::GW_GET_SCENE_LIST_CFM, [7].as_slice()))
                .expect("decode scene-list confirmation"),
            Response::SceneListAccepted { total_scenes: 7 }
        );

        let mut list = vec![0; 132];
        list[0] = 2;
        list[1] = 4;
        list[2..9].copy_from_slice(b"Evening");
        list[66] = 5;
        list[67..72].copy_from_slice(b"Night");
        list[131] = 3;
        assert_eq!(
            Response::decode(Frame::new(CommandId::GW_GET_SCENE_LIST_NTF, list)).expect("decode scene list"),
            Response::SceneList(SceneList {
                scenes: vec![
                    SceneSummary {
                        scene_id: 4,
                        name: "Evening".to_owned(),
                    },
                    SceneSummary {
                        scene_id: 5,
                        name: "Night".to_owned(),
                    },
                ],
                remaining_scenes: 3,
            })
        );
    }

    #[test]
    fn decodes_scene_product_group_and_contact_vectors() {
        let session_id = SessionId::new(0x1234).expect("nonzero");
        let mut information = vec![0; 75];
        information[0] = 4;
        information[1..8].copy_from_slice(b"Evening");
        information[65] = 2;
        information[66..70].copy_from_slice(&[9, 0, 0x64, 0]);
        information[70..74].copy_from_slice(&[10, 4, 0xC8, 0]);
        information[74] = 1;
        assert_eq!(
            Response::decode(Frame::new(CommandId::GW_GET_SCENE_INFOAMATION_NTF, information))
                .expect("decode scene information"),
            Response::SceneInformation(SceneInformation {
                scene_id: 4,
                name: "Evening".to_owned(),
                nodes: vec![
                    SceneNode {
                        node_id: NodeId::new(9),
                        parameter_id: 0,
                        parameter: StandardParameter::Relative(Percentage::from_percent(50)),
                    },
                    SceneNode {
                        node_id: NodeId::new(10),
                        parameter_id: 4,
                        parameter: StandardParameter::Relative(Percentage::FULLY_CLOSED),
                    },
                ],
                remaining_nodes: 1,
            })
        );

        for command in [CommandId::GW_ACTIVATE_SCENE_CFM, CommandId::GW_STOP_SCENE_CFM] {
            assert_eq!(
                Response::decode(Frame::new(command, [0, 0x12, 0x34].as_slice())).expect("decode scene session result"),
                Response::SceneSessionResult(SceneSessionResult {
                    command,
                    status: 0,
                    session_id,
                })
            );
        }
        assert_eq!(
            Response::decode(Frame::new(
                CommandId::GW_SCENE_INFORMATION_CHANGED_NTF,
                [1, 4].as_slice(),
            ))
            .expect("decode scene change"),
            Response::SceneChange(SceneChange {
                change_type: 1,
                scene_id: 4,
            })
        );
        assert_eq!(
            Response::decode(Frame::new(
                CommandId::GW_ACTIVATE_PRODUCTGROUP_CFM,
                [0x12, 0x34, 0].as_slice(),
            ))
            .expect("decode product-group result"),
            Response::ProductGroupResult(ProductGroupResult { session_id, status: 0 })
        );
        assert_eq!(
            Response::decode(Frame::new(
                CommandId::GW_ACTIVATE_PRODUCTGROUP_NTF,
                [1, 2, 3].as_slice()
            ))
            .expect("decode opaque product-group notification"),
            Response::ProductGroupNotification(Bytes::from_static(&[1, 2, 3]))
        );

        let mut links = vec![0; 18];
        links[0] = 1;
        links[1..18].copy_from_slice(&[2, 1, 7, 1, 3, 0, 0x64, 0, 2, 1, 3, 2, 1, 0, 3, 4, 5]);
        assert_eq!(
            Response::decode(Frame::new(CommandId::GW_GET_CONTACT_INPUT_LINK_LIST_CFM, links))
                .expect("decode contact-input links"),
            Response::ContactInputLinks(vec![ContactInputLink {
                contact_input_id: 2,
                assignment: 1,
                action_id: 7,
                command_originator: 1,
                priority_level: 3,
                parameter_id: 0,
                position: StandardParameter::Relative(Percentage::from_percent(50)),
                velocity: 2,
                lock_priority_level: 1,
                priority_level_settings: [3, 2, 1, 0, 3],
                success_output_id: 4,
                error_output_id: 5,
            }])
        );
        for command in [
            CommandId::GW_SET_CONTACT_INPUT_LINK_CFM,
            CommandId::GW_REMOVE_CONTACT_INPUT_LINK_CFM,
        ] {
            assert_eq!(
                Response::decode(Frame::new(command, [2, 1].as_slice())).expect("decode contact-input operation"),
                Response::ContactInputOperationResult(ContactInputOperationResult {
                    command,
                    contact_input_id: 2,
                    status: 1,
                })
            );
        }
    }

    #[test]
    fn rejects_malformed_scene_and_contact_payloads() {
        for (command, expected) in [
            (CommandId::GW_INITIALIZE_SCENE_CFM, 1),
            (CommandId::GW_INITIALIZE_SCENE_NTF, 26),
            (CommandId::GW_INITIALIZE_SCENE_CANCEL_CFM, 1),
            (CommandId::GW_RECORD_SCENE_CFM, 1),
            (CommandId::GW_RECORD_SCENE_NTF, 2),
            (CommandId::GW_DELETE_SCENE_CFM, 2),
            (CommandId::GW_RENAME_SCENE_CFM, 2),
            (CommandId::GW_GET_SCENE_LIST_CFM, 1),
            (CommandId::GW_GET_SCENE_INFOAMATION_CFM, 2),
            (CommandId::GW_ACTIVATE_SCENE_CFM, 3),
            (CommandId::GW_STOP_SCENE_CFM, 3),
            (CommandId::GW_SCENE_INFORMATION_CHANGED_NTF, 2),
            (CommandId::GW_ACTIVATE_PRODUCTGROUP_CFM, 3),
            (CommandId::GW_SET_CONTACT_INPUT_LINK_CFM, 2),
            (CommandId::GW_REMOVE_CONTACT_INPUT_LINK_CFM, 2),
        ] {
            assert!(matches!(
                Response::decode(Frame::new(command, vec![0; expected - 1])),
                Err(KlfError::TruncatedPayload { .. })
            ));
            assert!(matches!(
                Response::decode(Frame::new(command, vec![0; expected + 1])),
                Err(KlfError::InvalidPayloadLength { .. })
            ));
        }

        let mut short_scene_information = vec![0; 67];
        short_scene_information[65] = 1;
        for (command, payload) in [
            (CommandId::GW_GET_SCENE_LIST_NTF, vec![1, 0]),
            (CommandId::GW_GET_SCENE_INFOAMATION_NTF, short_scene_information),
            (CommandId::GW_GET_CONTACT_INPUT_LINK_LIST_CFM, vec![1]),
        ] {
            assert!(matches!(
                Response::decode(Frame::new(command, payload)),
                Err(KlfError::TruncatedPayload { .. })
            ));
        }
        assert!(matches!(
            Response::decode(Frame::new(CommandId::GW_GET_SCENE_LIST_NTF, [4, 0].as_slice())),
            Err(KlfError::InvalidRequest { .. })
        ));
        let mut too_many_nodes = vec![0; 67];
        too_many_nodes[65] = 46;
        assert!(matches!(
            Response::decode(Frame::new(CommandId::GW_GET_SCENE_INFOAMATION_NTF, too_many_nodes,)),
            Err(KlfError::InvalidRequest { .. })
        ));
        assert!(matches!(
            Response::decode(Frame::new(
                CommandId::GW_GET_CONTACT_INPUT_LINK_LIST_CFM,
                [11].as_slice(),
            )),
            Err(KlfError::InvalidRequest { .. })
        ));
    }

    #[test]
    fn decodes_activation_log_vectors() {
        assert_eq!(
            Response::decode(Frame::new(
                CommandId::GW_GET_ACTIVATION_LOG_HEADER_CFM,
                [0x01, 0x00, 0, 7].as_slice(),
            ))
            .expect("decode activation-log header"),
            Response::ActivationLogHeader(ActivationLogHeader {
                maximum_lines: 256,
                line_count: 7,
            })
        );
        for command in [
            CommandId::GW_CLEAR_ACTIVATION_LOG_CFM,
            CommandId::GW_ACTIVATION_LOG_UPDATED_NTF,
        ] {
            assert_eq!(
                Response::decode(Frame::new(command, Bytes::new())).expect("decode activation-log acknowledgement"),
                Response::Acknowledgement { command }
            );
        }

        let payload = [
            0x65, 0x53, 0xF1, 0, 0x12, 0x34, 7, 9, 0, 0x64, 0, 2, 2, 0xDE, 0xAD, 0xBE, 0xEF,
        ];
        for command in [
            CommandId::GW_GET_ACTIVATION_LOG_LINE_CFM,
            CommandId::GW_GET_MULTIPLE_ACTIVATION_LOG_LINES_NTF,
        ] {
            assert_eq!(
                Response::decode(Frame::new(command, payload.to_vec())).expect("decode activation-log entry"),
                Response::ActivationLogEntry(ActivationLogEntry {
                    command,
                    timestamp: ProtocolTimestamp::from_unix_seconds(0x6553_F100),
                    session_id: SessionId::new(0x1234).expect("nonzero"),
                    status_id: 7,
                    node_id: NodeId::new(9),
                    node_parameter: 0,
                    parameter_value: StandardParameter::Relative(Percentage::from_percent(50)),
                    run_status: RunStatus::Active,
                    status_reply: 2,
                    information_code: 0xDEAD_BEEF,
                })
            );
        }
        assert_eq!(
            Response::decode(Frame::new(
                CommandId::GW_GET_MULTIPLE_ACTIVATION_LOG_LINES_CFM,
                [0, 7, 1].as_slice(),
            ))
            .expect("decode multiple activation-log result"),
            Response::MultipleActivationLogResult(MultipleActivationLogResult {
                line_count: 7,
                status: 1,
            })
        );
    }

    #[test]
    fn rejects_malformed_activation_log_payloads() {
        for (command, expected) in [
            (CommandId::GW_GET_ACTIVATION_LOG_HEADER_CFM, 4),
            (CommandId::GW_CLEAR_ACTIVATION_LOG_CFM, 0),
            (CommandId::GW_GET_ACTIVATION_LOG_LINE_CFM, 17),
            (CommandId::GW_ACTIVATION_LOG_UPDATED_NTF, 0),
            (CommandId::GW_GET_MULTIPLE_ACTIVATION_LOG_LINES_NTF, 17),
            (CommandId::GW_GET_MULTIPLE_ACTIVATION_LOG_LINES_CFM, 3),
        ] {
            if expected > 0 {
                assert!(matches!(
                    Response::decode(Frame::new(command, vec![0; expected - 1])),
                    Err(KlfError::TruncatedPayload { .. })
                ));
            }
            assert!(matches!(
                Response::decode(Frame::new(command, vec![0; expected + 1])),
                Err(KlfError::InvalidPayloadLength { .. })
            ));
        }
    }

    #[test]
    fn decodes_node_information_with_aliases() {
        let mut payload = vec![0; 124];
        payload[0] = 7;
        payload[1..3].copy_from_slice(&42_u16.to_be_bytes());
        payload[3] = 2;
        payload[4..10].copy_from_slice(b"Office");
        payload[68] = 1;
        payload[69..71].copy_from_slice(&0x1234_u16.to_be_bytes());
        payload[71] = 1;
        payload[72] = 2;
        payload[73] = 3;
        payload[74] = 1;
        payload[75] = 9;
        payload[76..84].copy_from_slice(&[0, 1, 2, 3, 4, 5, 6, 7]);
        payload[84] = 4;
        payload[85..87].copy_from_slice(&0x6400_u16.to_be_bytes());
        payload[87..89].copy_from_slice(&0xC800_u16.to_be_bytes());
        for offset in [89, 91, 93, 95] {
            payload[offset..offset + 2].copy_from_slice(&0xF7FF_u16.to_be_bytes());
        }
        payload[97..99].copy_from_slice(&12_u16.to_be_bytes());
        payload[99..103].copy_from_slice(&1_700_000_000_u32.to_be_bytes());
        payload[103] = 2;
        payload[104..108].copy_from_slice(&[0, 1, 0, 2]);
        payload[108..112].copy_from_slice(&[0, 3, 0, 4]);

        let response = Response::decode(Frame::new(CommandId::GW_GET_ALL_NODES_INFORMATION_NTF, payload))
            .expect("decode node information");
        let Response::NodeInformation(node) = response else {
            panic!("expected node information");
        };
        assert_eq!(node.node_id, NodeId::new(7));
        assert_eq!(node.name, "Office");
        assert_eq!(node.operating_state, OperatingState::Executing);
        assert_eq!(
            node.current_position,
            StandardParameter::Relative(Percentage::from_percent(50))
        );
        assert_eq!(node.aliases, [Alias { kind: 1, value: 2 }, Alias { kind: 3, value: 4 }]);

        assert!(matches!(
            Response::decode(Frame::new(CommandId::GW_GET_NODE_INFORMATION_NTF, vec![0; 123])),
            Err(KlfError::TruncatedPayload { .. })
        ));
        assert!(matches!(
            Response::decode(Frame::new(CommandId::GW_GET_NODE_INFORMATION_NTF, vec![0; 125])),
            Err(KlfError::InvalidPayloadLength { .. })
        ));
        let mut too_many_aliases = vec![0; 124];
        too_many_aliases[103] = 6;
        assert!(matches!(
            Response::decode(Frame::new(CommandId::GW_GET_NODE_INFORMATION_NTF, too_many_aliases)),
            Err(KlfError::InvalidRequest { .. })
        ));
    }

    #[test]
    fn decodes_main_status_and_preserves_unknown_enums() {
        let payload = [0, 9, 0xFE, 7, 0xA5, 0, 3, 0xC8, 0, 0x64, 0, 0, 12, 1, 2, 3, 4, 0xDD];
        let response =
            Response::decode(Frame::new(CommandId::GW_STATUS_REQUEST_NTF, payload.to_vec())).expect("decode status");
        let Response::StatusNotification(status) = response else {
            panic!("expected status notification");
        };
        assert_eq!(status.run_status, RunStatus::Unknown(0xA5));
        assert_eq!(status.status_id, 0xFE);
        assert_eq!(
            status.detail,
            StatusNotificationDetail::Main {
                target_position: StandardParameter::Relative(Percentage::FULLY_CLOSED),
                current_position: StandardParameter::Relative(Percentage::from_percent(50)),
                remaining_time: 12,
                last_master_execution_address: 0x0102_0304,
                last_command_originator: 0xDD,
            }
        );
    }

    #[test]
    fn malformed_payloads_return_errors_without_panicking() {
        for length in 0..9 {
            let result = Response::decode(Frame::new(CommandId::GW_GET_VERSION_CFM, vec![0; length]));
            assert_eq!(
                result,
                Err(KlfError::TruncatedPayload {
                    command: CommandId::GW_GET_VERSION_CFM,
                    expected: 9,
                    actual: length,
                })
            );
        }

        let unknown = Response::decode(Frame::new(CommandId::new(0xDEAD), [1, 2].as_slice()))
            .expect("unknown command is preserved");
        assert_eq!(
            unknown,
            Response::Unknown {
                command: CommandId::new(0xDEAD),
                payload: Bytes::from_static(&[1, 2]),
            }
        );
    }

    #[test]
    fn rejects_oversized_core_payloads() {
        let cases = [
            (CommandId::GW_REBOOT_CFM, 0),
            (CommandId::GW_SET_FACTORY_DEFAULT_CFM, 0),
            (CommandId::GW_ERROR_NTF, 1),
            (CommandId::GW_GET_VERSION_CFM, 9),
            (CommandId::GW_GET_PROTOCOL_VERSION_CFM, 4),
            (CommandId::GW_GET_STATE_CFM, 6),
            (CommandId::GW_SET_NETWORK_SETUP_CFM, 0),
            (CommandId::GW_SET_UTC_CFM, 0),
            (CommandId::GW_LEAVE_LEARN_STATE_CFM, 1),
            (CommandId::GW_RTC_SET_TIME_ZONE_CFM, 1),
            (CommandId::GW_GET_NETWORK_SETUP_CFM, 13),
            (CommandId::GW_GET_LOCAL_TIME_CFM, 15),
            (CommandId::GW_PASSWORD_ENTER_CFM, 1),
            (CommandId::GW_PASSWORD_CHANGE_CFM, 1),
            (CommandId::GW_PASSWORD_CHANGE_NTF, 32),
            (CommandId::GW_GET_NODE_INFORMATION_CFM, 2),
            (CommandId::GW_GET_ALL_NODES_INFORMATION_CFM, 2),
            (CommandId::GW_GET_ALL_NODES_INFORMATION_FINISHED_NTF, 0),
            (CommandId::GW_NODE_INFORMATION_CHANGED_NTF, 69),
            (CommandId::GW_NODE_STATE_POSITION_CHANGED_NTF, 20),
            (CommandId::GW_COMMAND_REMAINING_TIME_NTF, 6),
            (CommandId::GW_HOUSE_STATUS_MONITOR_ENABLE_CFM, 0),
            (CommandId::GW_HOUSE_STATUS_MONITOR_DISABLE_CFM, 0),
        ];
        for (command, expected) in cases {
            assert!(matches!(
                Response::decode(Frame::new(command, vec![0; expected + 1])),
                Err(KlfError::InvalidPayloadLength { .. })
            ));
        }

        for (command, mut payload) in [
            (CommandId::GW_GET_NODE_INFORMATION_NTF, vec![0; 124]),
            (CommandId::GW_COMMAND_SEND_CFM, vec![0, 1, 0]),
            (
                CommandId::GW_COMMAND_RUN_STATUS_NTF,
                vec![0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            ),
            (CommandId::GW_SESSION_FINISHED_NTF, vec![0, 1]),
            (CommandId::GW_STATUS_REQUEST_CFM, vec![0, 1, 0]),
            (CommandId::GW_STATUS_REQUEST_NTF, vec![0, 1, 0, 0, 0, 0, 0, 0]),
        ] {
            payload.push(0);
            assert!(matches!(
                Response::decode(Frame::new(command, payload)),
                Err(KlfError::InvalidPayloadLength { .. })
            ));
        }
    }
}
