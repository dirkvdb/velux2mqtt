//! Typed building blocks for the VELUX KLF 200 local API.
//!
//! The transport actor is intentionally not part of this first migration slice. This module
//! already owns the wire contract: command identifiers, request and response payloads, KLF
//! envelopes, streaming SLIP framing, and protocol value types.

mod command;
mod command_id;
mod connection;
mod error;
mod frame;
mod response;
mod slip;
mod types;

pub use command::{
    ActivationLogRequest, CommandExtensionRequest, CommandRequest, CommandTarget, ConfigurationRequest,
    GetLimitationStatusRequest, ModeRequest, NodeGroupRequest, ProductGroupActivationRequest, Request,
    SceneActivationRequest, SceneContactRequest, SceneStopRequest, SetLimitationRequest, StatusRequestType,
    WinkRequest,
};
pub use command_id::{ALL_KNOWN_COMMANDS, CommandId, CommandKind, KnownCommand};
pub use connection::{ConnectionEvent, ConnectionSettings, Klf200Client, Klf200Config};
pub use error::{KlfError, Result};
pub use frame::{Frame, KLF_PROTOCOL_ID};
pub use response::{
    ActivationLogEntry, ActivationLogHeader, AllGroupsAccepted, CommandRunStatus, ConfigurationActivationResult,
    ContactInputOperationResult, ControllerCopyResult, GatewayState, GroupChange, GroupInformationNotification,
    GroupOperationResult, KeyChangeResult, LimitationStatus, LocalTime, MultipleActivationLogResult,
    NodeDiscoveryResult, NodeInformation, NodeInformationChanged, NodeMutationResult, NodeStatePosition,
    OperatingState, ParameterStatus, PasswordChangedNotification, PgcJob, ProductGroupResult, ProtocolVersion,
    Response, RunStatus, SceneChange, SceneInformation, SceneInitializationResult, SceneList, SceneNode,
    SceneObjectResult, SceneSessionResult, SceneStatusResult, SceneSummary, SessionCommandResult, StatusNotification,
    StatusNotificationDetail, SystemTableData, SystemTableObject, SystemTableUpdate, Version,
};
pub use slip::{Decoder as SlipDecoder, encode as slip_encode};
pub use types::{
    ACTUATOR_COUNT, ActuatorSet, Alias, BeaconSet, ContactInputLink, GroupId, GroupInformation, NetworkSetup,
    NewGroupInformation, NodeId, NodeSet, Percentage, ProtocolTimestamp, RawPosition, SessionId, SessionIdAllocator,
    StandardParameter, decode_fixed_string, encode_fixed_string,
};
