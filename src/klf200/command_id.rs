use std::fmt;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CommandId(u16);

impl CommandId {
    #[must_use]
    pub const fn new(raw: u16) -> Self {
        Self(raw)
    }

    #[must_use]
    pub const fn raw(self) -> u16 {
        self.0
    }

    #[must_use]
    pub fn known(self) -> Option<&'static KnownCommand> {
        ALL_KNOWN_COMMANDS.iter().find(|command| command.id == self)
    }

    #[must_use]
    pub fn name(self) -> Option<&'static str> {
        self.known().map(|command| command.name)
    }

    #[must_use]
    pub fn kind(self) -> CommandKind {
        self.known().map_or(CommandKind::Unknown, KnownCommand::kind)
    }

    #[must_use]
    pub fn expected_confirmation(self) -> Option<Self> {
        let request_prefix = self.name()?.strip_suffix("_REQ")?;
        ALL_KNOWN_COMMANDS
            .iter()
            .find(|candidate| candidate.name.strip_suffix("_CFM") == Some(request_prefix))
            .map(|candidate| candidate.id)
    }
}

impl From<u16> for CommandId {
    fn from(value: u16) -> Self {
        Self(value)
    }
}

impl From<CommandId> for u16 {
    fn from(value: CommandId) -> Self {
        value.raw()
    }
}

impl fmt::Display for CommandId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(name) = self.name() {
            formatter.write_str(name)
        } else {
            write!(formatter, "UNKNOWN_COMMAND_0x{:04X}", self.raw())
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandKind {
    Request,
    Confirmation,
    Notification,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KnownCommand {
    pub id: CommandId,
    pub name: &'static str,
}

impl KnownCommand {
    #[must_use]
    pub fn kind(&self) -> CommandKind {
        if self.name.ends_with("_REQ") {
            CommandKind::Request
        } else if self.name.ends_with("_CFM") {
            CommandKind::Confirmation
        } else if self.name.ends_with("_NTF") {
            CommandKind::Notification
        } else {
            CommandKind::Unknown
        }
    }
}

macro_rules! known_commands {
    ($( $name:ident = $value:expr ),+ $(,)?) => {
        impl CommandId {
            $(pub const $name: Self = Self($value);)+
        }

        pub const ALL_KNOWN_COMMANDS: &[KnownCommand] = &[
            $(KnownCommand { id: CommandId::$name, name: stringify!($name) },)+
        ];
    };
}

known_commands! {
    GW_ERROR_NTF = 0x0000,
    GW_REBOOT_REQ = 0x0001,
    GW_REBOOT_CFM = 0x0002,
    GW_SET_FACTORY_DEFAULT_REQ = 0x0003,
    GW_SET_FACTORY_DEFAULT_CFM = 0x0004,
    GW_GET_VERSION_REQ = 0x0008,
    GW_GET_VERSION_CFM = 0x0009,
    GW_GET_PROTOCOL_VERSION_REQ = 0x000A,
    GW_GET_PROTOCOL_VERSION_CFM = 0x000B,
    GW_GET_STATE_REQ = 0x000C,
    GW_GET_STATE_CFM = 0x000D,
    GW_LEAVE_LEARN_STATE_REQ = 0x000E,
    GW_LEAVE_LEARN_STATE_CFM = 0x000F,
    GW_GET_NETWORK_SETUP_REQ = 0x00E0,
    GW_GET_NETWORK_SETUP_CFM = 0x00E1,
    GW_SET_NETWORK_SETUP_REQ = 0x00E2,
    GW_SET_NETWORK_SETUP_CFM = 0x00E3,
    GW_CS_GET_SYSTEMTABLE_DATA_REQ = 0x0100,
    GW_CS_GET_SYSTEMTABLE_DATA_CFM = 0x0101,
    GW_CS_GET_SYSTEMTABLE_DATA_NTF = 0x0102,
    GW_CS_DISCOVER_NODES_REQ = 0x0103,
    GW_CS_DISCOVER_NODES_CFM = 0x0104,
    GW_CS_DISCOVER_NODES_NTF = 0x0105,
    GW_CS_REMOVE_NODES_REQ = 0x0106,
    GW_CS_REMOVE_NODES_CFM = 0x0107,
    GW_CS_VIRGIN_STATE_REQ = 0x0108,
    GW_CS_VIRGIN_STATE_CFM = 0x0109,
    GW_CS_CONTROLLER_COPY_REQ = 0x010A,
    GW_CS_CONTROLLER_COPY_CFM = 0x010B,
    GW_CS_CONTROLLER_COPY_NTF = 0x010C,
    GW_CS_CONTROLLER_COPY_CANCEL_NTF = 0x010D,
    GW_CS_RECEIVE_KEY_REQ = 0x010E,
    GW_CS_RECEIVE_KEY_CFM = 0x010F,
    GW_CS_RECEIVE_KEY_NTF = 0x0110,
    GW_CS_PGC_JOB_NTF = 0x0111,
    GW_CS_SYSTEM_TABLE_UPDATE_NTF = 0x0112,
    GW_CS_GENERATE_NEW_KEY_REQ = 0x0113,
    GW_CS_GENERATE_NEW_KEY_CFM = 0x0114,
    GW_CS_GENERATE_NEW_KEY_NTF = 0x0115,
    GW_CS_REPAIR_KEY_REQ = 0x0116,
    GW_CS_REPAIR_KEY_CFM = 0x0117,
    GW_CS_REPAIR_KEY_NTF = 0x0118,
    GW_CS_ACTIVATE_CONFIGURATION_MODE_REQ = 0x0119,
    GW_CS_ACTIVATE_CONFIGURATION_MODE_CFM = 0x011A,
    GW_GET_NODE_INFORMATION_REQ = 0x0200,
    GW_GET_NODE_INFORMATION_CFM = 0x0201,
    GW_GET_ALL_NODES_INFORMATION_REQ = 0x0202,
    GW_GET_ALL_NODES_INFORMATION_CFM = 0x0203,
    GW_GET_ALL_NODES_INFORMATION_NTF = 0x0204,
    GW_GET_ALL_NODES_INFORMATION_FINISHED_NTF = 0x0205,
    GW_SET_NODE_VARIATION_REQ = 0x0206,
    GW_SET_NODE_VARIATION_CFM = 0x0207,
    GW_SET_NODE_NAME_REQ = 0x0208,
    GW_SET_NODE_NAME_CFM = 0x0209,
    GW_NODE_INFORMATION_CHANGED_NTF = 0x020C,
    GW_SET_NODE_ORDER_AND_PLACEMENT_REQ = 0x020D,
    GW_SET_NODE_ORDER_AND_PLACEMENT_CFM = 0x020E,
    GW_GET_NODE_INFORMATION_NTF = 0x0210,
    GW_NODE_STATE_POSITION_CHANGED_NTF = 0x0211,
    GW_GET_GROUP_INFORMATION_REQ = 0x0220,
    GW_GET_GROUP_INFORMATION_CFM = 0x0221,
    GW_SET_GROUP_INFORMATION_REQ = 0x0222,
    GW_SET_GROUP_INFORMATION_CFM = 0x0223,
    GW_GROUP_INFORMATION_CHANGED_NTF = 0x0224,
    GW_DELETE_GROUP_REQ = 0x0225,
    GW_DELETE_GROUP_CFM = 0x0226,
    GW_NEW_GROUP_REQ = 0x0227,
    GW_NEW_GROUP_CFM = 0x0228,
    GW_GET_ALL_GROUPS_INFORMATION_REQ = 0x0229,
    GW_GET_ALL_GROUPS_INFORMATION_CFM = 0x022A,
    GW_GET_ALL_GROUPS_INFORMATION_NTF = 0x022B,
    GW_GET_ALL_GROUPS_INFORMATION_FINISHED_NTF = 0x022C,
    GW_GROUP_DELETED_NTF = 0x022D,
    GW_GET_GROUP_INFORMATION_NTF = 0x0230,
    GW_HOUSE_STATUS_MONITOR_ENABLE_REQ = 0x0240,
    GW_HOUSE_STATUS_MONITOR_ENABLE_CFM = 0x0241,
    GW_HOUSE_STATUS_MONITOR_DISABLE_REQ = 0x0242,
    GW_HOUSE_STATUS_MONITOR_DISABLE_CFM = 0x0243,
    GW_COMMAND_SEND_REQ = 0x0300,
    GW_COMMAND_SEND_CFM = 0x0301,
    GW_COMMAND_RUN_STATUS_NTF = 0x0302,
    GW_COMMAND_REMAINING_TIME_NTF = 0x0303,
    GW_SESSION_FINISHED_NTF = 0x0304,
    GW_STATUS_REQUEST_REQ = 0x0305,
    GW_STATUS_REQUEST_CFM = 0x0306,
    GW_STATUS_REQUEST_NTF = 0x0307,
    GW_WINK_SEND_REQ = 0x0308,
    GW_WINK_SEND_CFM = 0x0309,
    GW_WINK_SEND_NTF = 0x030A,
    GW_SET_LIMITATION_REQ = 0x0310,
    GW_SET_LIMITATION_CFM = 0x0311,
    GW_GET_LIMITATION_STATUS_REQ = 0x0312,
    GW_GET_LIMITATION_STATUS_CFM = 0x0313,
    GW_LIMITATION_STATUS_NTF = 0x0314,
    GW_MODE_SEND_REQ = 0x0320,
    GW_MODE_SEND_CFM = 0x0321,
    GW_MODE_SEND_NTF = 0x0322,
    GW_INITIALIZE_SCENE_REQ = 0x0400,
    GW_INITIALIZE_SCENE_CFM = 0x0401,
    GW_INITIALIZE_SCENE_NTF = 0x0402,
    GW_INITIALIZE_SCENE_CANCEL_REQ = 0x0403,
    GW_INITIALIZE_SCENE_CANCEL_CFM = 0x0404,
    GW_RECORD_SCENE_REQ = 0x0405,
    GW_RECORD_SCENE_CFM = 0x0406,
    GW_RECORD_SCENE_NTF = 0x0407,
    GW_DELETE_SCENE_REQ = 0x0408,
    GW_DELETE_SCENE_CFM = 0x0409,
    GW_RENAME_SCENE_REQ = 0x040A,
    GW_RENAME_SCENE_CFM = 0x040B,
    GW_GET_SCENE_LIST_REQ = 0x040C,
    GW_GET_SCENE_LIST_CFM = 0x040D,
    GW_GET_SCENE_LIST_NTF = 0x040E,
    GW_GET_SCENE_INFOAMATION_REQ = 0x040F,
    GW_GET_SCENE_INFOAMATION_CFM = 0x0410,
    GW_GET_SCENE_INFOAMATION_NTF = 0x0411,
    GW_ACTIVATE_SCENE_REQ = 0x0412,
    GW_ACTIVATE_SCENE_CFM = 0x0413,
    GW_STOP_SCENE_REQ = 0x0415,
    GW_STOP_SCENE_CFM = 0x0416,
    GW_SCENE_INFORMATION_CHANGED_NTF = 0x0419,
    GW_ACTIVATE_PRODUCTGROUP_REQ = 0x0447,
    GW_ACTIVATE_PRODUCTGROUP_CFM = 0x0448,
    GW_ACTIVATE_PRODUCTGROUP_NTF = 0x0449,
    GW_GET_CONTACT_INPUT_LINK_LIST_REQ = 0x0460,
    GW_GET_CONTACT_INPUT_LINK_LIST_CFM = 0x0461,
    GW_SET_CONTACT_INPUT_LINK_REQ = 0x0462,
    GW_SET_CONTACT_INPUT_LINK_CFM = 0x0463,
    GW_REMOVE_CONTACT_INPUT_LINK_REQ = 0x0464,
    GW_REMOVE_CONTACT_INPUT_LINK_CFM = 0x0465,
    GW_GET_ACTIVATION_LOG_HEADER_REQ = 0x0500,
    GW_GET_ACTIVATION_LOG_HEADER_CFM = 0x0501,
    GW_CLEAR_ACTIVATION_LOG_REQ = 0x0502,
    GW_CLEAR_ACTIVATION_LOG_CFM = 0x0503,
    GW_GET_ACTIVATION_LOG_LINE_REQ = 0x0504,
    GW_GET_ACTIVATION_LOG_LINE_CFM = 0x0505,
    GW_ACTIVATION_LOG_UPDATED_NTF = 0x0506,
    GW_GET_MULTIPLE_ACTIVATION_LOG_LINES_REQ = 0x0507,
    GW_GET_MULTIPLE_ACTIVATION_LOG_LINES_NTF = 0x0508,
    GW_GET_MULTIPLE_ACTIVATION_LOG_LINES_CFM = 0x0509,
    GW_SET_UTC_REQ = 0x2000,
    GW_SET_UTC_CFM = 0x2001,
    GW_RTC_SET_TIME_ZONE_REQ = 0x2002,
    GW_RTC_SET_TIME_ZONE_CFM = 0x2003,
    GW_GET_LOCAL_TIME_REQ = 0x2004,
    GW_GET_LOCAL_TIME_CFM = 0x2005,
    GW_PASSWORD_ENTER_REQ = 0x3000,
    GW_PASSWORD_ENTER_CFM = 0x3001,
    GW_PASSWORD_CHANGE_REQ = 0x3002,
    GW_PASSWORD_CHANGE_CFM = 0x3003,
    GW_PASSWORD_CHANGE_NTF = 0x3004,
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn registry_contains_all_reference_commands_without_duplicates() {
        assert_eq!(ALL_KNOWN_COMMANDS.len(), 150);

        let ids = ALL_KNOWN_COMMANDS
            .iter()
            .map(|command| command.id)
            .collect::<HashSet<_>>();
        let names = ALL_KNOWN_COMMANDS
            .iter()
            .map(|command| command.name)
            .collect::<HashSet<_>>();
        assert_eq!(ids.len(), ALL_KNOWN_COMMANDS.len());
        assert_eq!(names.len(), ALL_KNOWN_COMMANDS.len());
    }

    #[test]
    fn known_ids_round_trip_and_unknown_ids_are_preserved() {
        for command in ALL_KNOWN_COMMANDS {
            assert_eq!(command.id.name(), Some(command.name));
            assert_eq!(command.id.known(), Some(command));
        }

        let unknown = CommandId::new(0xDEAD);
        assert_eq!(unknown.name(), None);
        assert_eq!(unknown.kind(), CommandKind::Unknown);
        assert_eq!(unknown.raw(), 0xDEAD);
        assert_eq!(unknown.to_string(), "UNKNOWN_COMMAND_0xDEAD");
    }

    #[test]
    fn requests_resolve_to_their_confirmation() {
        for command in ALL_KNOWN_COMMANDS
            .iter()
            .filter(|command| command.kind() == CommandKind::Request)
        {
            let confirmation = command.id.expected_confirmation();
            assert!(confirmation.is_some(), "{} has no confirmation", command.name);
            assert_eq!(confirmation.map(CommandId::kind), Some(CommandKind::Confirmation));
        }
    }
}
