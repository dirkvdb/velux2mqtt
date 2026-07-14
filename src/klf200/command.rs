use std::fmt;

use bytes::Bytes;

use super::{
    CommandId, ContactInputLink, Frame, GroupId, GroupInformation, KlfError, NetworkSetup, NewGroupInformation, NodeId,
    NodeSet, ProtocolTimestamp, Result, SessionId, StandardParameter, encode_fixed_string,
};

const MAX_TARGETS: usize = 20;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandTarget(Vec<NodeId>);

impl CommandTarget {
    /// Builds a non-empty, duplicate-free target list.
    ///
    /// # Errors
    ///
    /// Returns an error when no nodes are supplied, more than 20 nodes are supplied, or a node
    /// occurs more than once.
    pub fn new(nodes: impl IntoIterator<Item = NodeId>) -> Result<Self> {
        let nodes = nodes.into_iter().collect::<Vec<_>>();
        if nodes.is_empty() {
            return Err(KlfError::InvalidRequest {
                message: "at least one target node is required",
            });
        }
        if nodes.len() > MAX_TARGETS {
            return Err(KlfError::InvalidRequest {
                message: "a request can target at most 20 nodes",
            });
        }
        if nodes
            .iter()
            .enumerate()
            .any(|(index, node)| nodes[..index].contains(node))
        {
            return Err(KlfError::InvalidRequest {
                message: "target node IDs must be unique",
            });
        }
        Ok(Self(nodes))
    }

    #[must_use]
    pub fn nodes(&self) -> &[NodeId] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommandRequest {
    pub session_id: SessionId,
    pub main_parameter: StandardParameter,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WinkRequest {
    pub session_id: SessionId,
    pub command_originator: u8,
    pub priority_level: u8,
    pub enabled: bool,
    pub wink_time: u8,
    pub target: CommandTarget,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SetLimitationRequest {
    pub session_id: SessionId,
    pub command_originator: u8,
    pub priority_level: u8,
    pub target: CommandTarget,
    pub parameter_id: u8,
    pub minimum: StandardParameter,
    pub maximum: StandardParameter,
    pub limitation_time: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GetLimitationStatusRequest {
    pub session_id: SessionId,
    pub target: CommandTarget,
    pub parameter_id: u8,
    pub limitation_type: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModeRequest {
    pub session_id: SessionId,
    pub command_originator: u8,
    pub priority_level: u8,
    pub mode_number: u8,
    pub mode_parameter: u8,
    pub target: CommandTarget,
    pub priority_level_lock: bool,
    pub priority_level_settings: [u8; 8],
    pub lock_time: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandExtensionRequest {
    Wink(WinkRequest),
    SetLimitation(SetLimitationRequest),
    GetLimitationStatus(GetLimitationStatusRequest),
    Mode(ModeRequest),
}

impl CommandExtensionRequest {
    #[must_use]
    pub const fn command_id(&self) -> CommandId {
        match self {
            Self::Wink(_) => CommandId::GW_WINK_SEND_REQ,
            Self::SetLimitation(_) => CommandId::GW_SET_LIMITATION_REQ,
            Self::GetLimitationStatus(_) => CommandId::GW_GET_LIMITATION_STATUS_REQ,
            Self::Mode(_) => CommandId::GW_MODE_SEND_REQ,
        }
    }

    #[must_use]
    pub const fn session_id(&self) -> SessionId {
        match self {
            Self::Wink(request) => request.session_id,
            Self::SetLimitation(request) => request.session_id,
            Self::GetLimitationStatus(request) => request.session_id,
            Self::Mode(request) => request.session_id,
        }
    }

    fn encode(self) -> Result<Bytes> {
        match self {
            Self::Wink(request) => Ok(encode_wink(&request)),
            Self::SetLimitation(request) => Ok(encode_set_limitation(&request)),
            Self::GetLimitationStatus(request) => Ok(encode_get_limitation_status(&request)),
            Self::Mode(request) => encode_mode(&request),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SceneActivationRequest {
    pub session_id: SessionId,
    pub command_originator: u8,
    pub priority_level: u8,
    pub scene_id: u8,
    pub velocity: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SceneStopRequest {
    pub session_id: SessionId,
    pub command_originator: u8,
    pub priority_level: u8,
    pub scene_id: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductGroupActivationRequest {
    pub session_id: SessionId,
    pub command_originator: u8,
    pub priority_level: u8,
    pub product_group_id: u8,
    pub parameter_id: u8,
    pub position: StandardParameter,
    pub velocity: u8,
    pub priority_level_lock: bool,
    pub priority_level_settings: [u8; 8],
    pub lock_time: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SceneContactRequest {
    InitializeScene,
    CancelSceneInitialization,
    RecordScene(String),
    DeleteScene(u8),
    RenameScene { scene_id: u8, name: String },
    GetSceneList,
    GetSceneInformation(u8),
    ActivateScene(SceneActivationRequest),
    StopScene(SceneStopRequest),
    ActivateProductGroup(ProductGroupActivationRequest),
    GetContactInputLinks,
    SetContactInputLink(ContactInputLink),
    RemoveContactInputLink(u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivationLogRequest {
    GetHeader,
    Clear,
    GetLine(u16),
    GetLinesSince(ProtocolTimestamp),
}

impl ActivationLogRequest {
    #[must_use]
    pub const fn command_id(self) -> CommandId {
        match self {
            Self::GetHeader => CommandId::GW_GET_ACTIVATION_LOG_HEADER_REQ,
            Self::Clear => CommandId::GW_CLEAR_ACTIVATION_LOG_REQ,
            Self::GetLine(_) => CommandId::GW_GET_ACTIVATION_LOG_LINE_REQ,
            Self::GetLinesSince(_) => CommandId::GW_GET_MULTIPLE_ACTIVATION_LOG_LINES_REQ,
        }
    }

    fn encode(self) -> Bytes {
        match self {
            Self::GetHeader | Self::Clear => Bytes::new(),
            Self::GetLine(line) => Bytes::copy_from_slice(&line.to_be_bytes()),
            Self::GetLinesSince(timestamp) => Bytes::copy_from_slice(&timestamp.unix_seconds().to_be_bytes()),
        }
    }
}

impl SceneContactRequest {
    #[must_use]
    pub const fn command_id(&self) -> CommandId {
        match self {
            Self::InitializeScene => CommandId::GW_INITIALIZE_SCENE_REQ,
            Self::CancelSceneInitialization => CommandId::GW_INITIALIZE_SCENE_CANCEL_REQ,
            Self::RecordScene(_) => CommandId::GW_RECORD_SCENE_REQ,
            Self::DeleteScene(_) => CommandId::GW_DELETE_SCENE_REQ,
            Self::RenameScene { .. } => CommandId::GW_RENAME_SCENE_REQ,
            Self::GetSceneList => CommandId::GW_GET_SCENE_LIST_REQ,
            Self::GetSceneInformation(_) => CommandId::GW_GET_SCENE_INFOAMATION_REQ,
            Self::ActivateScene(_) => CommandId::GW_ACTIVATE_SCENE_REQ,
            Self::StopScene(_) => CommandId::GW_STOP_SCENE_REQ,
            Self::ActivateProductGroup(_) => CommandId::GW_ACTIVATE_PRODUCTGROUP_REQ,
            Self::GetContactInputLinks => CommandId::GW_GET_CONTACT_INPUT_LINK_LIST_REQ,
            Self::SetContactInputLink(_) => CommandId::GW_SET_CONTACT_INPUT_LINK_REQ,
            Self::RemoveContactInputLink(_) => CommandId::GW_REMOVE_CONTACT_INPUT_LINK_REQ,
        }
    }

    #[must_use]
    pub const fn session_id(&self) -> Option<SessionId> {
        match self {
            Self::ActivateScene(request) => Some(request.session_id),
            Self::StopScene(request) => Some(request.session_id),
            Self::ActivateProductGroup(request) => Some(request.session_id),
            _ => None,
        }
    }

    fn encode(self) -> Result<Bytes> {
        match self {
            Self::InitializeScene
            | Self::CancelSceneInitialization
            | Self::GetSceneList
            | Self::GetContactInputLinks => Ok(Bytes::new()),
            Self::RecordScene(name) => Ok(Bytes::copy_from_slice(&encode_fixed_string::<64>(&name)?)),
            Self::DeleteScene(scene_id)
            | Self::GetSceneInformation(scene_id)
            | Self::RemoveContactInputLink(scene_id) => Ok(Bytes::copy_from_slice(&[scene_id])),
            Self::RenameScene { scene_id, name } => {
                let mut payload = [0; 65];
                payload[0] = scene_id;
                payload[1..].copy_from_slice(&encode_fixed_string::<64>(&name)?);
                Ok(Bytes::copy_from_slice(&payload))
            }
            Self::ActivateScene(request) => Ok(encode_activate_scene(request)),
            Self::StopScene(request) => Ok(encode_stop_scene(request)),
            Self::ActivateProductGroup(request) => encode_activate_product_group(request),
            Self::SetContactInputLink(link) => encode_contact_input_link(link),
        }
    }
}

impl CommandRequest {
    #[must_use]
    pub const fn new(session_id: SessionId, main_parameter: StandardParameter) -> Self {
        Self {
            session_id,
            main_parameter,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum StatusRequestType {
    TargetPosition = 0,
    CurrentPosition = 1,
    RemainingTime = 2,
    MainInformation = 3,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConfigurationRequest {
    GetSystemTableData,
    DiscoverNodes { node_type: u8 },
    RemoveNodes(NodeSet),
    VirginState,
    ControllerCopy { mode: u8 },
    ReceiveKey,
    GenerateNewKey,
    RepairKey,
    ActivateConfiguration(NodeSet),
}

impl ConfigurationRequest {
    #[must_use]
    pub const fn command_id(&self) -> CommandId {
        match self {
            Self::GetSystemTableData => CommandId::GW_CS_GET_SYSTEMTABLE_DATA_REQ,
            Self::DiscoverNodes { .. } => CommandId::GW_CS_DISCOVER_NODES_REQ,
            Self::RemoveNodes(_) => CommandId::GW_CS_REMOVE_NODES_REQ,
            Self::VirginState => CommandId::GW_CS_VIRGIN_STATE_REQ,
            Self::ControllerCopy { .. } => CommandId::GW_CS_CONTROLLER_COPY_REQ,
            Self::ReceiveKey => CommandId::GW_CS_RECEIVE_KEY_REQ,
            Self::GenerateNewKey => CommandId::GW_CS_GENERATE_NEW_KEY_REQ,
            Self::RepairKey => CommandId::GW_CS_REPAIR_KEY_REQ,
            Self::ActivateConfiguration(_) => CommandId::GW_CS_ACTIVATE_CONFIGURATION_MODE_REQ,
        }
    }

    fn encode(self) -> Bytes {
        match self {
            Self::GetSystemTableData
            | Self::VirginState
            | Self::ReceiveKey
            | Self::GenerateNewKey
            | Self::RepairKey => Bytes::new(),
            Self::DiscoverNodes { node_type } => Bytes::copy_from_slice(&[node_type]),
            Self::ControllerCopy { mode } => Bytes::copy_from_slice(&[mode]),
            Self::RemoveNodes(nodes) | Self::ActivateConfiguration(nodes) => encode_node_set(&nodes),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NodeGroupRequest {
    SetNodeVariation { node_id: NodeId, variation: u8 },
    SetNodeName { node_id: NodeId, name: String },
    SetNodeOrderAndPlacement { node_id: NodeId, order: u16, placement: u8 },
    GetGroup(GroupId),
    SetGroup(GroupInformation),
    DeleteGroup(GroupId),
    NewGroup(NewGroupInformation),
    GetAllGroups { use_filter: bool, group_type: u8 },
}

impl NodeGroupRequest {
    #[must_use]
    pub const fn command_id(&self) -> CommandId {
        match self {
            Self::SetNodeVariation { .. } => CommandId::GW_SET_NODE_VARIATION_REQ,
            Self::SetNodeName { .. } => CommandId::GW_SET_NODE_NAME_REQ,
            Self::SetNodeOrderAndPlacement { .. } => CommandId::GW_SET_NODE_ORDER_AND_PLACEMENT_REQ,
            Self::GetGroup(_) => CommandId::GW_GET_GROUP_INFORMATION_REQ,
            Self::SetGroup(_) => CommandId::GW_SET_GROUP_INFORMATION_REQ,
            Self::DeleteGroup(_) => CommandId::GW_DELETE_GROUP_REQ,
            Self::NewGroup(_) => CommandId::GW_NEW_GROUP_REQ,
            Self::GetAllGroups { .. } => CommandId::GW_GET_ALL_GROUPS_INFORMATION_REQ,
        }
    }

    fn encode(self) -> Result<Bytes> {
        match self {
            Self::SetNodeVariation { node_id, variation } => Ok(Bytes::copy_from_slice(&[node_id.get(), variation])),
            Self::SetNodeName { node_id, name } => {
                let mut payload = [0; 65];
                payload[0] = node_id.get();
                payload[1..].copy_from_slice(&encode_fixed_string::<64>(&name)?);
                Ok(Bytes::copy_from_slice(&payload))
            }
            Self::SetNodeOrderAndPlacement {
                node_id,
                order,
                placement,
            } => {
                let mut payload = [0; 4];
                payload[0] = node_id.get();
                payload[1..3].copy_from_slice(&order.to_be_bytes());
                payload[3] = placement;
                Ok(Bytes::copy_from_slice(&payload))
            }
            Self::GetGroup(group_id) | Self::DeleteGroup(group_id) => Ok(Bytes::copy_from_slice(&[group_id.get()])),
            Self::SetGroup(group) => encode_group_information(&group),
            Self::NewGroup(group) => encode_new_group_information(&group),
            Self::GetAllGroups { use_filter, group_type } => {
                Ok(Bytes::copy_from_slice(&[u8::from(use_filter), group_type]))
            }
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub enum Request {
    Reboot,
    SetFactoryDefault,
    GetVersion,
    GetProtocolVersion,
    GetState,
    LeaveLearnState,
    GetNetworkSetup,
    SetNetworkSetup(NetworkSetup),
    SetUtc(ProtocolTimestamp),
    SetTimeZone(String),
    GetLocalTime,
    Configuration(ConfigurationRequest),
    NodeGroup(NodeGroupRequest),
    CommandExtension(CommandExtensionRequest),
    SceneContact(SceneContactRequest),
    ActivationLog(ActivationLogRequest),
    GetAllNodesInformation,
    HouseStatusMonitorEnable,
    HouseStatusMonitorDisable,
    GetNodeInformation(NodeId),
    PasswordEnter(String),
    PasswordChange {
        current_password: String,
        new_password: String,
    },
    CommandSend {
        command: CommandRequest,
        target: CommandTarget,
    },
    StatusRequest {
        session_id: SessionId,
        target: CommandTarget,
        status_type: StatusRequestType,
    },
    Raw {
        command: CommandId,
        payload: Bytes,
    },
}

impl fmt::Debug for Request {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PasswordEnter(_) => formatter.write_str("PasswordEnter([REDACTED])"),
            Self::PasswordChange { .. } => formatter.write_str("PasswordChange([REDACTED])"),
            Self::Raw { command, payload } => formatter
                .debug_struct("Raw")
                .field("command", command)
                .field("payload_length", &payload.len())
                .finish(),
            Self::Reboot => formatter.write_str("Reboot"),
            Self::SetFactoryDefault => formatter.write_str("SetFactoryDefault"),
            Self::GetVersion => formatter.write_str("GetVersion"),
            Self::GetProtocolVersion => formatter.write_str("GetProtocolVersion"),
            Self::GetState => formatter.write_str("GetState"),
            Self::LeaveLearnState => formatter.write_str("LeaveLearnState"),
            Self::GetNetworkSetup => formatter.write_str("GetNetworkSetup"),
            Self::SetNetworkSetup(setup) => formatter.debug_tuple("SetNetworkSetup").field(setup).finish(),
            Self::SetUtc(timestamp) => formatter.debug_tuple("SetUtc").field(timestamp).finish(),
            Self::SetTimeZone(time_zone) => formatter.debug_tuple("SetTimeZone").field(time_zone).finish(),
            Self::GetLocalTime => formatter.write_str("GetLocalTime"),
            Self::Configuration(request) => formatter.debug_tuple("Configuration").field(request).finish(),
            Self::NodeGroup(request) => formatter.debug_tuple("NodeGroup").field(request).finish(),
            Self::CommandExtension(request) => formatter.debug_tuple("CommandExtension").field(request).finish(),
            Self::SceneContact(request) => formatter.debug_tuple("SceneContact").field(request).finish(),
            Self::ActivationLog(request) => formatter.debug_tuple("ActivationLog").field(request).finish(),
            Self::GetAllNodesInformation => formatter.write_str("GetAllNodesInformation"),
            Self::HouseStatusMonitorEnable => formatter.write_str("HouseStatusMonitorEnable"),
            Self::HouseStatusMonitorDisable => formatter.write_str("HouseStatusMonitorDisable"),
            Self::GetNodeInformation(node_id) => formatter.debug_tuple("GetNodeInformation").field(node_id).finish(),
            Self::CommandSend { command, target } => formatter
                .debug_struct("CommandSend")
                .field("command", command)
                .field("target", target)
                .finish(),
            Self::StatusRequest {
                session_id,
                target,
                status_type,
            } => formatter
                .debug_struct("StatusRequest")
                .field("session_id", session_id)
                .field("target", target)
                .field("status_type", status_type)
                .finish(),
        }
    }
}

impl Request {
    /// Builds an authentication request while enforcing the protocol's 32-byte field limit.
    ///
    /// # Errors
    ///
    /// Returns an error when the password contains a NUL byte or requires more than 31 bytes.
    pub fn password_enter(password: impl Into<String>) -> Result<Self> {
        let password = password.into();
        encode_fixed_string::<32>(&password)?;
        Ok(Self::PasswordEnter(password))
    }

    /// Builds a password-change request without exposing either password through `Debug`.
    ///
    /// # Errors
    ///
    /// Returns an error when either password contains a NUL byte or needs more than 31 bytes.
    pub fn password_change(current_password: impl Into<String>, new_password: impl Into<String>) -> Result<Self> {
        let current_password = current_password.into();
        let new_password = new_password.into();
        encode_fixed_string::<32>(&current_password)?;
        encode_fixed_string::<32>(&new_password)?;
        Ok(Self::PasswordChange {
            current_password,
            new_password,
        })
    }

    #[must_use]
    pub fn command_id(&self) -> CommandId {
        match self {
            Self::Reboot => CommandId::GW_REBOOT_REQ,
            Self::SetFactoryDefault => CommandId::GW_SET_FACTORY_DEFAULT_REQ,
            Self::GetVersion => CommandId::GW_GET_VERSION_REQ,
            Self::GetProtocolVersion => CommandId::GW_GET_PROTOCOL_VERSION_REQ,
            Self::GetState => CommandId::GW_GET_STATE_REQ,
            Self::LeaveLearnState => CommandId::GW_LEAVE_LEARN_STATE_REQ,
            Self::GetNetworkSetup => CommandId::GW_GET_NETWORK_SETUP_REQ,
            Self::SetNetworkSetup(_) => CommandId::GW_SET_NETWORK_SETUP_REQ,
            Self::SetUtc(_) => CommandId::GW_SET_UTC_REQ,
            Self::SetTimeZone(_) => CommandId::GW_RTC_SET_TIME_ZONE_REQ,
            Self::GetLocalTime => CommandId::GW_GET_LOCAL_TIME_REQ,
            Self::Configuration(request) => request.command_id(),
            Self::NodeGroup(request) => request.command_id(),
            Self::CommandExtension(request) => request.command_id(),
            Self::SceneContact(request) => request.command_id(),
            Self::ActivationLog(request) => request.command_id(),
            Self::GetAllNodesInformation => CommandId::GW_GET_ALL_NODES_INFORMATION_REQ,
            Self::HouseStatusMonitorEnable => CommandId::GW_HOUSE_STATUS_MONITOR_ENABLE_REQ,
            Self::HouseStatusMonitorDisable => CommandId::GW_HOUSE_STATUS_MONITOR_DISABLE_REQ,
            Self::GetNodeInformation(_) => CommandId::GW_GET_NODE_INFORMATION_REQ,
            Self::PasswordEnter(_) => CommandId::GW_PASSWORD_ENTER_REQ,
            Self::PasswordChange { .. } => CommandId::GW_PASSWORD_CHANGE_REQ,
            Self::CommandSend { .. } => CommandId::GW_COMMAND_SEND_REQ,
            Self::StatusRequest { .. } => CommandId::GW_STATUS_REQUEST_REQ,
            Self::Raw { command, .. } => *command,
        }
    }

    #[must_use]
    pub fn session_id(&self) -> Option<SessionId> {
        match self {
            Self::CommandSend { command, .. } => Some(command.session_id),
            Self::StatusRequest { session_id, .. } => Some(*session_id),
            Self::CommandExtension(request) => Some(request.session_id()),
            Self::SceneContact(request) => request.session_id(),
            _ => None,
        }
    }

    /// Encodes this typed request into a KLF frame.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid fixed-width fields or when a raw request uses a known
    /// confirmation or notification command identifier.
    pub fn encode(self) -> Result<Frame> {
        let command_id = self.command_id();
        let payload = match self {
            Self::Reboot
            | Self::SetFactoryDefault
            | Self::GetVersion
            | Self::GetProtocolVersion
            | Self::GetState
            | Self::LeaveLearnState
            | Self::GetNetworkSetup
            | Self::GetLocalTime
            | Self::GetAllNodesInformation
            | Self::HouseStatusMonitorEnable
            | Self::HouseStatusMonitorDisable => Bytes::new(),
            Self::SetNetworkSetup(setup) => encode_network_setup(setup),
            Self::SetUtc(timestamp) => Bytes::copy_from_slice(&timestamp.unix_seconds().to_be_bytes()),
            Self::SetTimeZone(time_zone) => Bytes::copy_from_slice(&encode_fixed_string::<64>(&time_zone)?),
            Self::Configuration(request) => request.encode(),
            Self::NodeGroup(request) => request.encode()?,
            Self::CommandExtension(request) => request.encode()?,
            Self::SceneContact(request) => request.encode()?,
            Self::ActivationLog(request) => request.encode(),
            Self::GetNodeInformation(node_id) => Bytes::copy_from_slice(&[node_id.get()]),
            Self::PasswordEnter(password) => Bytes::copy_from_slice(&encode_fixed_string::<32>(&password)?),
            Self::PasswordChange {
                current_password,
                new_password,
            } => {
                let mut payload = [0; 64];
                payload[..32].copy_from_slice(&encode_fixed_string::<32>(&current_password)?);
                payload[32..].copy_from_slice(&encode_fixed_string::<32>(&new_password)?);
                Bytes::copy_from_slice(&payload)
            }
            Self::CommandSend { command, target } => encode_command_send(command, &target),
            Self::StatusRequest {
                session_id,
                target,
                status_type,
            } => encode_status_request(session_id, &target, status_type),
            Self::Raw { command, payload } => {
                if command.known().is_some() && command.kind() != super::CommandKind::Request {
                    return Err(KlfError::UnsupportedRequest { command });
                }
                payload
            }
        };
        Ok(Frame::new(command_id, payload))
    }
}

fn encode_network_setup(setup: NetworkSetup) -> Bytes {
    let mut payload = [0; 13];
    payload[..4].copy_from_slice(&setup.ip_address);
    payload[4..8].copy_from_slice(&setup.subnet_mask);
    payload[8..12].copy_from_slice(&setup.default_gateway);
    payload[12] = u8::from(setup.dhcp);
    Bytes::copy_from_slice(&payload)
}

fn encode_node_set(nodes: &NodeSet) -> Bytes {
    let mut payload = [0; 26];
    payload[..25].copy_from_slice(nodes.actuators.as_bytes());
    payload[25] = nodes.beacons.as_byte();
    Bytes::copy_from_slice(&payload)
}

fn encode_group_information(group: &GroupInformation) -> Result<Bytes> {
    let mut payload = [0; 99];
    payload[0] = group.group_id.get();
    payload[1..3].copy_from_slice(&group.order.to_be_bytes());
    payload[3] = group.placement;
    payload[4..68].copy_from_slice(&encode_fixed_string::<64>(&group.name)?);
    payload[68] = group.velocity;
    payload[69] = group.node_variation;
    payload[70] = group.group_type;
    payload[71] = group.object_count;
    payload[72..97].copy_from_slice(group.actuators.as_bytes());
    payload[97..99].copy_from_slice(&group.revision.to_be_bytes());
    Ok(Bytes::copy_from_slice(&payload))
}

fn encode_new_group_information(group: &NewGroupInformation) -> Result<Bytes> {
    let mut payload = [0; 96];
    payload[..2].copy_from_slice(&group.order.to_be_bytes());
    payload[2] = group.placement;
    payload[3..67].copy_from_slice(&encode_fixed_string::<64>(&group.name)?);
    payload[67] = group.velocity;
    payload[68] = group.node_variation;
    payload[69] = group.group_type;
    payload[70] = group.object_count;
    payload[71..96].copy_from_slice(group.actuators.as_bytes());
    Ok(Bytes::copy_from_slice(&payload))
}

fn encode_target(destination: &mut [u8], target: &CommandTarget) -> u8 {
    for (slot, node_id) in destination.iter_mut().zip(target.nodes()) {
        *slot = node_id.get();
    }
    u8::try_from(target.nodes().len()).unwrap_or(0)
}

fn encode_wink(request: &WinkRequest) -> Bytes {
    let mut payload = [0; 27];
    payload[..2].copy_from_slice(&request.session_id.get().to_be_bytes());
    payload[2] = request.command_originator;
    payload[3] = request.priority_level;
    payload[4] = u8::from(request.enabled);
    payload[5] = request.wink_time;
    payload[6] = encode_target(&mut payload[7..27], &request.target);
    Bytes::copy_from_slice(&payload)
}

fn encode_set_limitation(request: &SetLimitationRequest) -> Bytes {
    let mut payload = [0; 31];
    payload[..2].copy_from_slice(&request.session_id.get().to_be_bytes());
    payload[2] = request.command_originator;
    payload[3] = request.priority_level;
    payload[4] = encode_target(&mut payload[5..25], &request.target);
    payload[25] = request.parameter_id;
    payload[26..28].copy_from_slice(&request.minimum.to_raw().get().to_be_bytes());
    payload[28..30].copy_from_slice(&request.maximum.to_raw().get().to_be_bytes());
    payload[30] = request.limitation_time;
    Bytes::copy_from_slice(&payload)
}

fn encode_get_limitation_status(request: &GetLimitationStatusRequest) -> Bytes {
    let mut payload = [0; 25];
    payload[..2].copy_from_slice(&request.session_id.get().to_be_bytes());
    payload[2] = encode_target(&mut payload[3..23], &request.target);
    payload[23] = request.parameter_id;
    payload[24] = request.limitation_type;
    Bytes::copy_from_slice(&payload)
}

fn encode_mode(request: &ModeRequest) -> Result<Bytes> {
    validate_priority_settings(&request.priority_level_settings)?;
    let mut payload = [0; 31];
    payload[..2].copy_from_slice(&request.session_id.get().to_be_bytes());
    payload[2] = request.command_originator;
    payload[3] = request.priority_level;
    payload[4] = request.mode_number;
    payload[5] = request.mode_parameter;
    payload[6] = encode_target(&mut payload[7..27], &request.target);
    payload[27] = u8::from(request.priority_level_lock);
    if request.priority_level_lock {
        payload[28] = pack_two_bit_settings(&request.priority_level_settings[..4]);
        payload[29] = pack_two_bit_settings(&request.priority_level_settings[4..]);
    }
    payload[30] = request.lock_time;
    Ok(Bytes::copy_from_slice(&payload))
}

fn pack_two_bit_settings(settings: &[u8]) -> u8 {
    settings.iter().fold(0, |packed, setting| (packed << 2) | setting)
}

fn validate_priority_settings(settings: &[u8]) -> Result<()> {
    if settings.iter().any(|setting| *setting > 3) {
        Err(KlfError::InvalidRequest {
            message: "priority-level settings must be in the range 0..=3",
        })
    } else {
        Ok(())
    }
}

fn encode_activate_scene(request: SceneActivationRequest) -> Bytes {
    let mut payload = [0; 6];
    payload[..2].copy_from_slice(&request.session_id.get().to_be_bytes());
    payload[2] = request.command_originator;
    payload[3] = request.priority_level;
    payload[4] = request.scene_id;
    payload[5] = request.velocity;
    Bytes::copy_from_slice(&payload)
}

fn encode_stop_scene(request: SceneStopRequest) -> Bytes {
    let mut payload = [0; 5];
    payload[..2].copy_from_slice(&request.session_id.get().to_be_bytes());
    payload[2] = request.command_originator;
    payload[3] = request.priority_level;
    payload[4] = request.scene_id;
    Bytes::copy_from_slice(&payload)
}

fn encode_activate_product_group(request: ProductGroupActivationRequest) -> Result<Bytes> {
    validate_priority_settings(&request.priority_level_settings)?;
    let mut payload = [0; 13];
    payload[..2].copy_from_slice(&request.session_id.get().to_be_bytes());
    payload[2] = request.command_originator;
    payload[3] = request.priority_level;
    payload[4] = request.product_group_id;
    payload[5] = request.parameter_id;
    payload[6..8].copy_from_slice(&request.position.to_raw().get().to_be_bytes());
    payload[8] = request.velocity;
    payload[9] = u8::from(request.priority_level_lock);
    if request.priority_level_lock {
        payload[10] = pack_two_bit_settings(&request.priority_level_settings[..4]);
        payload[11] = pack_two_bit_settings(&request.priority_level_settings[4..]);
    }
    payload[12] = request.lock_time;
    Ok(Bytes::copy_from_slice(&payload))
}

fn encode_contact_input_link(link: ContactInputLink) -> Result<Bytes> {
    validate_priority_settings(&link.priority_level_settings)?;
    let mut payload = [0; 17];
    payload[0] = link.contact_input_id;
    payload[1] = link.assignment;
    payload[2] = link.action_id;
    payload[3] = link.command_originator;
    payload[4] = link.priority_level;
    payload[5] = link.parameter_id;
    payload[6..8].copy_from_slice(&link.position.to_raw().get().to_be_bytes());
    payload[8] = link.velocity;
    payload[9] = link.lock_priority_level;
    payload[10..15].copy_from_slice(&link.priority_level_settings);
    payload[15] = link.success_output_id;
    payload[16] = link.error_output_id;
    Ok(Bytes::copy_from_slice(&payload))
}

fn encode_command_send(command: CommandRequest, target: &CommandTarget) -> Bytes {
    let mut payload = [0; 66];
    payload[0..2].copy_from_slice(&command.session_id.get().to_be_bytes());
    payload[2] = 1; // User command originator.
    payload[3] = 3; // User priority level 2.
    payload[4] = 0; // Main parameter active.
    payload[7..9].copy_from_slice(&command.main_parameter.to_raw().get().to_be_bytes());
    payload[41] = match u8::try_from(target.nodes().len()) {
        Ok(count) => count,
        Err(_) => return Bytes::new(),
    };
    for (destination, node_id) in payload[42..62].iter_mut().zip(target.nodes()) {
        *destination = node_id.get();
    }
    Bytes::copy_from_slice(&payload)
}

fn encode_status_request(session_id: SessionId, target: &CommandTarget, status_type: StatusRequestType) -> Bytes {
    let mut payload = [0; 26];
    payload[0..2].copy_from_slice(&session_id.get().to_be_bytes());
    payload[2] = match u8::try_from(target.nodes().len()) {
        Ok(count) => count,
        Err(_) => return Bytes::new(),
    };
    for (destination, node_id) in payload[3..23].iter_mut().zip(target.nodes()) {
        *destination = node_id.get();
    }
    payload[23] = status_type as u8;
    Bytes::copy_from_slice(&payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::klf200::{Percentage, slip_encode};

    #[test]
    fn password_request_is_fixed_width_and_debug_redacted() {
        let request = Request::password_enter("secret").expect("valid password");
        assert_eq!(format!("{request:?}"), "PasswordEnter([REDACTED])");
        let frame = request.encode().expect("encode request");
        assert_eq!(frame.command, CommandId::GW_PASSWORD_ENTER_REQ);
        assert_eq!(&frame.payload[..6], b"secret");
        assert!(frame.payload[6..].iter().all(|byte| *byte == 0));
        assert_eq!(
            format!("{frame:?}"),
            "Frame { protocol_id: 0, command: CommandId(12288), payload: \"[REDACTED]\" }"
        );
        assert_eq!(
            Request::password_enter("x".repeat(32)),
            Err(KlfError::StringTooLong {
                actual: 32,
                maximum: 31
            })
        );
    }

    #[test]
    fn encodes_gateway_network_time_and_password_vectors() {
        let network = Request::SetNetworkSetup(NetworkSetup {
            ip_address: [192, 168, 1, 78],
            subnet_mask: [255, 255, 255, 0],
            default_gateway: [192, 168, 1, 1],
            dhcp: true,
        })
        .encode()
        .expect("encode network setup");
        assert_eq!(network.command, CommandId::GW_SET_NETWORK_SETUP_REQ);
        assert_eq!(
            network.payload.as_ref(),
            [192, 168, 1, 78, 255, 255, 255, 0, 192, 168, 1, 1, 1]
        );

        let utc = Request::SetUtc(ProtocolTimestamp::from_unix_seconds(0x6553_F100))
            .encode()
            .expect("encode UTC timestamp");
        assert_eq!(utc.command, CommandId::GW_SET_UTC_REQ);
        assert_eq!(utc.payload.as_ref(), [0x65, 0x53, 0xF1, 0x00]);

        let time_zone = Request::SetTimeZone(":UTC:UTC:0000".to_owned())
            .encode()
            .expect("encode time zone");
        assert_eq!(time_zone.command, CommandId::GW_RTC_SET_TIME_ZONE_REQ);
        assert_eq!(time_zone.payload.len(), 64);
        assert_eq!(&time_zone.payload[..13], b":UTC:UTC:0000");
        assert!(time_zone.payload[13..].iter().all(|byte| *byte == 0));

        let local_time = Request::GetLocalTime.encode().expect("encode local time request");
        assert_eq!(local_time.command, CommandId::GW_GET_LOCAL_TIME_REQ);
        assert!(local_time.payload.is_empty());

        let password = Request::password_change("current", "replacement")
            .expect("valid passwords")
            .encode()
            .expect("encode password change");
        assert_eq!(password.command, CommandId::GW_PASSWORD_CHANGE_REQ);
        assert_eq!(&password.payload[..7], b"current");
        assert_eq!(&password.payload[32..43], b"replacement");
        assert_eq!(
            format!("{password:?}"),
            "Frame { protocol_id: 0, command: CommandId(12290), payload: \"[REDACTED]\" }"
        );
    }

    #[test]
    fn rejects_invalid_gateway_strings_without_exposing_passwords() {
        assert_eq!(
            Request::SetTimeZone("x".repeat(64)).encode(),
            Err(KlfError::StringTooLong {
                actual: 64,
                maximum: 63,
            })
        );
        let request = Request::password_change("old", "new").expect("valid passwords");
        assert_eq!(format!("{request:?}"), "PasswordChange([REDACTED])");
        assert_eq!(
            Request::password_change("old", "x".repeat(32)),
            Err(KlfError::StringTooLong {
                actual: 32,
                maximum: 31,
            })
        );
    }

    #[test]
    fn encodes_configuration_service_vectors() {
        let mut actuator_bytes = [0; 25];
        actuator_bytes[0] = 0x81;
        actuator_bytes[24] = 0x80;
        let nodes = NodeSet::new(
            crate::klf200::ActuatorSet::from_bytes(actuator_bytes),
            crate::klf200::BeaconSet::from_byte(0b101),
        );
        let cases = [
            (
                ConfigurationRequest::GetSystemTableData,
                CommandId::GW_CS_GET_SYSTEMTABLE_DATA_REQ,
                Vec::new(),
            ),
            (
                ConfigurationRequest::DiscoverNodes { node_type: 2 },
                CommandId::GW_CS_DISCOVER_NODES_REQ,
                vec![2],
            ),
            (
                ConfigurationRequest::RemoveNodes(nodes.clone()),
                CommandId::GW_CS_REMOVE_NODES_REQ,
                [actuator_bytes.as_slice(), &[0b101]].concat(),
            ),
            (
                ConfigurationRequest::VirginState,
                CommandId::GW_CS_VIRGIN_STATE_REQ,
                Vec::new(),
            ),
            (
                ConfigurationRequest::ControllerCopy { mode: 1 },
                CommandId::GW_CS_CONTROLLER_COPY_REQ,
                vec![1],
            ),
            (
                ConfigurationRequest::ReceiveKey,
                CommandId::GW_CS_RECEIVE_KEY_REQ,
                Vec::new(),
            ),
            (
                ConfigurationRequest::GenerateNewKey,
                CommandId::GW_CS_GENERATE_NEW_KEY_REQ,
                Vec::new(),
            ),
            (
                ConfigurationRequest::RepairKey,
                CommandId::GW_CS_REPAIR_KEY_REQ,
                Vec::new(),
            ),
            (
                ConfigurationRequest::ActivateConfiguration(nodes),
                CommandId::GW_CS_ACTIVATE_CONFIGURATION_MODE_REQ,
                [actuator_bytes.as_slice(), &[0b101]].concat(),
            ),
        ];
        for (request, command, payload) in cases {
            let frame = Request::Configuration(request)
                .encode()
                .expect("encode configuration request");
            assert_eq!(frame.command, command);
            assert_eq!(frame.payload.as_ref(), payload);
        }
    }

    fn group_request_fixtures() -> ([u8; 25], GroupInformation, NewGroupInformation) {
        let mut actuator_bytes = [0; 25];
        actuator_bytes[0] = 0x81;
        actuator_bytes[24] = 0x80;
        let actuators = crate::klf200::ActuatorSet::from_bytes(actuator_bytes);
        let group = GroupInformation {
            group_id: GroupId::new(4),
            order: 0x1234,
            placement: 7,
            name: "West windows".to_owned(),
            velocity: 2,
            node_variation: 3,
            group_type: 1,
            object_count: 3,
            actuators: actuators.clone(),
            revision: 0x4567,
        };
        let new_group = NewGroupInformation {
            order: group.order,
            placement: group.placement,
            name: group.name.clone(),
            velocity: group.velocity,
            node_variation: group.node_variation,
            group_type: group.group_type,
            object_count: group.object_count,
            actuators,
        };
        (actuator_bytes, group, new_group)
    }

    #[test]
    fn encodes_node_and_group_vectors() {
        let (actuator_bytes, group, new_group) = group_request_fixtures();
        let cases = [
            (
                NodeGroupRequest::SetNodeVariation {
                    node_id: NodeId::new(9),
                    variation: 3,
                },
                CommandId::GW_SET_NODE_VARIATION_REQ,
                vec![9, 3],
            ),
            (
                NodeGroupRequest::SetNodeName {
                    node_id: NodeId::new(9),
                    name: "Kitchen".to_owned(),
                },
                CommandId::GW_SET_NODE_NAME_REQ,
                {
                    let mut payload = vec![0; 65];
                    payload[0] = 9;
                    payload[1..8].copy_from_slice(b"Kitchen");
                    payload
                },
            ),
            (
                NodeGroupRequest::SetNodeOrderAndPlacement {
                    node_id: NodeId::new(9),
                    order: 0x1234,
                    placement: 7,
                },
                CommandId::GW_SET_NODE_ORDER_AND_PLACEMENT_REQ,
                vec![9, 0x12, 0x34, 7],
            ),
            (
                NodeGroupRequest::GetGroup(GroupId::new(4)),
                CommandId::GW_GET_GROUP_INFORMATION_REQ,
                vec![4],
            ),
            (
                NodeGroupRequest::SetGroup(group),
                CommandId::GW_SET_GROUP_INFORMATION_REQ,
                {
                    let mut payload = vec![0; 99];
                    payload[0..4].copy_from_slice(&[4, 0x12, 0x34, 7]);
                    payload[4..16].copy_from_slice(b"West windows");
                    payload[68..72].copy_from_slice(&[2, 3, 1, 3]);
                    payload[72..97].copy_from_slice(&actuator_bytes);
                    payload[97..99].copy_from_slice(&[0x45, 0x67]);
                    payload
                },
            ),
            (
                NodeGroupRequest::DeleteGroup(GroupId::new(4)),
                CommandId::GW_DELETE_GROUP_REQ,
                vec![4],
            ),
            (NodeGroupRequest::NewGroup(new_group), CommandId::GW_NEW_GROUP_REQ, {
                let mut payload = vec![0; 96];
                payload[0..3].copy_from_slice(&[0x12, 0x34, 7]);
                payload[3..15].copy_from_slice(b"West windows");
                payload[67..71].copy_from_slice(&[2, 3, 1, 3]);
                payload[71..96].copy_from_slice(&actuator_bytes);
                payload
            }),
            (
                NodeGroupRequest::GetAllGroups {
                    use_filter: true,
                    group_type: 1,
                },
                CommandId::GW_GET_ALL_GROUPS_INFORMATION_REQ,
                vec![1, 1],
            ),
        ];

        for (request, command, payload) in cases {
            let frame = Request::NodeGroup(request).encode().expect("encode node/group request");
            assert_eq!(frame.command, command);
            assert_eq!(frame.payload.as_ref(), payload);
        }
    }

    #[test]
    fn rejects_oversized_node_and_group_names() {
        let result = Request::NodeGroup(NodeGroupRequest::SetNodeName {
            node_id: NodeId::new(1),
            name: "x".repeat(64),
        })
        .encode();
        assert_eq!(
            result,
            Err(KlfError::StringTooLong {
                actual: 64,
                maximum: 63,
            })
        );

        let result = Request::NodeGroup(NodeGroupRequest::NewGroup(NewGroupInformation {
            order: 0,
            placement: 0,
            name: "x".repeat(64),
            velocity: 0,
            node_variation: 0,
            group_type: 0,
            object_count: 0,
            actuators: crate::klf200::ActuatorSet::new(),
        }))
        .encode();
        assert!(matches!(result, Err(KlfError::StringTooLong { .. })));
    }

    #[test]
    fn encodes_wink_limitation_and_mode_vectors() {
        let session_id = SessionId::new(0x1234).expect("nonzero");
        let target = CommandTarget::new([NodeId::new(2), NodeId::new(9)]).expect("targets");
        let cases = [
            (
                CommandExtensionRequest::Wink(WinkRequest {
                    session_id,
                    command_originator: 1,
                    priority_level: 3,
                    enabled: true,
                    wink_time: 5,
                    target: target.clone(),
                }),
                CommandId::GW_WINK_SEND_REQ,
                {
                    let mut payload = vec![0; 27];
                    payload[..9].copy_from_slice(&[0x12, 0x34, 1, 3, 1, 5, 2, 2, 9]);
                    payload
                },
            ),
            (
                CommandExtensionRequest::SetLimitation(SetLimitationRequest {
                    session_id,
                    command_originator: 1,
                    priority_level: 3,
                    target: target.clone(),
                    parameter_id: 0,
                    minimum: StandardParameter::Relative(Percentage::from_percent(25)),
                    maximum: StandardParameter::Relative(Percentage::from_percent(75)),
                    limitation_time: 253,
                }),
                CommandId::GW_SET_LIMITATION_REQ,
                {
                    let mut payload = vec![0; 31];
                    payload[..7].copy_from_slice(&[0x12, 0x34, 1, 3, 2, 2, 9]);
                    payload[25..31].copy_from_slice(&[0, 0x32, 0, 0x96, 0, 253]);
                    payload
                },
            ),
            (
                CommandExtensionRequest::GetLimitationStatus(GetLimitationStatusRequest {
                    session_id,
                    target: target.clone(),
                    parameter_id: 4,
                    limitation_type: 1,
                }),
                CommandId::GW_GET_LIMITATION_STATUS_REQ,
                {
                    let mut payload = vec![0; 25];
                    payload[..5].copy_from_slice(&[0x12, 0x34, 2, 2, 9]);
                    payload[23..25].copy_from_slice(&[4, 1]);
                    payload
                },
            ),
            (
                CommandExtensionRequest::Mode(ModeRequest {
                    session_id,
                    command_originator: 1,
                    priority_level: 3,
                    mode_number: 2,
                    mode_parameter: 7,
                    target,
                    priority_level_lock: true,
                    priority_level_settings: [0, 1, 2, 3, 3, 2, 1, 0],
                    lock_time: 8,
                }),
                CommandId::GW_MODE_SEND_REQ,
                {
                    let mut payload = vec![0; 31];
                    payload[..9].copy_from_slice(&[0x12, 0x34, 1, 3, 2, 7, 2, 2, 9]);
                    payload[27..31].copy_from_slice(&[1, 0x1B, 0xE4, 8]);
                    payload
                },
            ),
        ];

        for (request, command, payload) in cases {
            let wrapped = Request::CommandExtension(request);
            assert_eq!(wrapped.session_id(), Some(session_id));
            let frame = wrapped.encode().expect("encode extended command");
            assert_eq!(frame.command, command);
            assert_eq!(frame.payload.as_ref(), payload);
        }
    }

    #[test]
    fn rejects_invalid_mode_priority_settings() {
        let request = ModeRequest {
            session_id: SessionId::MIN,
            command_originator: 1,
            priority_level: 3,
            mode_number: 0,
            mode_parameter: 0,
            target: CommandTarget::new([NodeId::new(1)]).expect("target"),
            priority_level_lock: true,
            priority_level_settings: [0, 1, 2, 4, 0, 1, 2, 3],
            lock_time: 0,
        };
        assert_eq!(
            Request::CommandExtension(CommandExtensionRequest::Mode(request)).encode(),
            Err(KlfError::InvalidRequest {
                message: "priority-level settings must be in the range 0..=3",
            })
        );
    }

    #[test]
    fn encodes_scene_vectors() {
        let session_id = SessionId::new(0x1234).expect("nonzero");
        let cases = [
            (
                SceneContactRequest::InitializeScene,
                CommandId::GW_INITIALIZE_SCENE_REQ,
                Vec::new(),
            ),
            (
                SceneContactRequest::CancelSceneInitialization,
                CommandId::GW_INITIALIZE_SCENE_CANCEL_REQ,
                Vec::new(),
            ),
            (
                SceneContactRequest::RecordScene("Evening".to_owned()),
                CommandId::GW_RECORD_SCENE_REQ,
                {
                    let mut payload = vec![0; 64];
                    payload[..7].copy_from_slice(b"Evening");
                    payload
                },
            ),
            (
                SceneContactRequest::DeleteScene(4),
                CommandId::GW_DELETE_SCENE_REQ,
                vec![4],
            ),
            (
                SceneContactRequest::RenameScene {
                    scene_id: 4,
                    name: "Night".to_owned(),
                },
                CommandId::GW_RENAME_SCENE_REQ,
                {
                    let mut payload = vec![0; 65];
                    payload[0] = 4;
                    payload[1..6].copy_from_slice(b"Night");
                    payload
                },
            ),
            (
                SceneContactRequest::GetSceneList,
                CommandId::GW_GET_SCENE_LIST_REQ,
                Vec::new(),
            ),
            (
                SceneContactRequest::GetSceneInformation(4),
                CommandId::GW_GET_SCENE_INFOAMATION_REQ,
                vec![4],
            ),
            (
                SceneContactRequest::ActivateScene(SceneActivationRequest {
                    session_id,
                    command_originator: 1,
                    priority_level: 3,
                    scene_id: 4,
                    velocity: 2,
                }),
                CommandId::GW_ACTIVATE_SCENE_REQ,
                vec![0x12, 0x34, 1, 3, 4, 2],
            ),
            (
                SceneContactRequest::StopScene(SceneStopRequest {
                    session_id,
                    command_originator: 1,
                    priority_level: 3,
                    scene_id: 4,
                }),
                CommandId::GW_STOP_SCENE_REQ,
                vec![0x12, 0x34, 1, 3, 4],
            ),
        ];

        for (request, command, payload) in cases {
            let expected_session_id = request.session_id();
            let wrapped = Request::SceneContact(request);
            assert_eq!(wrapped.session_id(), expected_session_id);
            let frame = wrapped.encode().expect("encode scene/contact request");
            assert_eq!(frame.command, command);
            assert_eq!(frame.payload.as_ref(), payload);
        }
    }

    #[test]
    fn encodes_product_group_and_contact_vectors() {
        let session_id = SessionId::new(0x1234).expect("nonzero");
        let contact_link = ContactInputLink {
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
        };
        let cases = [
            (
                SceneContactRequest::ActivateProductGroup(ProductGroupActivationRequest {
                    session_id,
                    command_originator: 1,
                    priority_level: 3,
                    product_group_id: 8,
                    parameter_id: 0,
                    position: StandardParameter::Relative(Percentage::from_percent(50)),
                    velocity: 2,
                    priority_level_lock: true,
                    priority_level_settings: [0, 1, 2, 3, 3, 2, 1, 0],
                    lock_time: 9,
                }),
                CommandId::GW_ACTIVATE_PRODUCTGROUP_REQ,
                vec![0x12, 0x34, 1, 3, 8, 0, 0x64, 0, 2, 1, 0x1B, 0xE4, 9],
            ),
            (
                SceneContactRequest::GetContactInputLinks,
                CommandId::GW_GET_CONTACT_INPUT_LINK_LIST_REQ,
                Vec::new(),
            ),
            (
                SceneContactRequest::SetContactInputLink(contact_link),
                CommandId::GW_SET_CONTACT_INPUT_LINK_REQ,
                vec![2, 1, 7, 1, 3, 0, 0x64, 0, 2, 1, 3, 2, 1, 0, 3, 4, 5],
            ),
            (
                SceneContactRequest::RemoveContactInputLink(2),
                CommandId::GW_REMOVE_CONTACT_INPUT_LINK_REQ,
                vec![2],
            ),
        ];

        for (request, command, payload) in cases {
            let expected_session_id = request.session_id();
            let wrapped = Request::SceneContact(request);
            assert_eq!(wrapped.session_id(), expected_session_id);
            let frame = wrapped.encode().expect("encode product/contact request");
            assert_eq!(frame.command, command);
            assert_eq!(frame.payload.as_ref(), payload);
        }
    }

    #[test]
    fn rejects_invalid_scene_contact_fields() {
        assert!(matches!(
            Request::SceneContact(SceneContactRequest::RecordScene("x".repeat(64))).encode(),
            Err(KlfError::StringTooLong { .. })
        ));
        let invalid_link = ContactInputLink {
            contact_input_id: 0,
            assignment: 0,
            action_id: 0,
            command_originator: 1,
            priority_level: 3,
            parameter_id: 0,
            position: StandardParameter::Default,
            velocity: 0,
            lock_priority_level: 0,
            priority_level_settings: [0, 1, 2, 3, 4],
            success_output_id: 0,
            error_output_id: 0,
        };
        assert!(matches!(
            Request::SceneContact(SceneContactRequest::SetContactInputLink(invalid_link)).encode(),
            Err(KlfError::InvalidRequest { .. })
        ));
    }

    #[test]
    fn encodes_activation_log_vectors() {
        let cases = [
            (
                ActivationLogRequest::GetHeader,
                CommandId::GW_GET_ACTIVATION_LOG_HEADER_REQ,
                Vec::new(),
            ),
            (
                ActivationLogRequest::Clear,
                CommandId::GW_CLEAR_ACTIVATION_LOG_REQ,
                Vec::new(),
            ),
            (
                ActivationLogRequest::GetLine(0x1234),
                CommandId::GW_GET_ACTIVATION_LOG_LINE_REQ,
                vec![0x12, 0x34],
            ),
            (
                ActivationLogRequest::GetLinesSince(ProtocolTimestamp::from_unix_seconds(0x6553_F100)),
                CommandId::GW_GET_MULTIPLE_ACTIVATION_LOG_LINES_REQ,
                vec![0x65, 0x53, 0xF1, 0],
            ),
        ];
        for (request, command, payload) in cases {
            let frame = Request::ActivationLog(request)
                .encode()
                .expect("encode activation-log request");
            assert_eq!(frame.command, command);
            assert_eq!(frame.payload.as_ref(), payload);
        }
    }

    #[test]
    fn encodes_golden_cover_command() {
        let target = CommandTarget::new([NodeId::new(7)]).expect("one target");
        let request = Request::CommandSend {
            command: CommandRequest::new(
                SessionId::new(0x1234).expect("nonzero"),
                StandardParameter::Relative(Percentage::FULLY_CLOSED),
            ),
            target,
        };
        let frame = request.encode().expect("encode request");
        assert_eq!(frame.command, CommandId::GW_COMMAND_SEND_REQ);
        assert_eq!(frame.payload.len(), 66);
        assert_eq!(&frame.payload[0..9], [0x12, 0x34, 1, 3, 0, 0, 0, 0xC8, 0]);
        assert_eq!(frame.payload[41], 1);
        assert_eq!(frame.payload[42], 7);

        let slip_frame = slip_encode(&frame.encode().expect("encode envelope"));
        assert_eq!(slip_frame.first(), Some(&0xC0));
        assert_eq!(slip_frame.last(), Some(&0xC0));
    }

    #[test]
    fn encodes_main_status_request() {
        let target = CommandTarget::new([NodeId::new(2), NodeId::new(9)]).expect("targets");
        let frame = Request::StatusRequest {
            session_id: SessionId::new(5).expect("nonzero"),
            target,
            status_type: StatusRequestType::MainInformation,
        }
        .encode()
        .expect("encode request");
        assert_eq!(frame.payload.len(), 26);
        assert_eq!(&frame.payload[..5], [0, 5, 2, 2, 9]);
        assert_eq!(frame.payload[23], 3);
    }

    #[test]
    fn validates_targets_and_raw_command_direction() {
        assert!(CommandTarget::new([]).is_err());
        assert!(CommandTarget::new([NodeId::new(1), NodeId::new(1)]).is_err());
        assert!(CommandTarget::new((0..21).map(NodeId::new)).is_err());
        assert_eq!(
            Request::Raw {
                command: CommandId::GW_GET_VERSION_CFM,
                payload: Bytes::new()
            }
            .encode(),
            Err(KlfError::UnsupportedRequest {
                command: CommandId::GW_GET_VERSION_CFM
            })
        );
    }
}
