use std::collections::BTreeSet;
use std::fmt;

use super::{KlfError, Result};

pub const ACTUATOR_COUNT: usize = 200;
const ACTUATOR_BYTES: usize = ACTUATOR_COUNT / 8;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NodeId(u8);

impl NodeId {
    #[must_use]
    pub const fn new(value: u8) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

impl From<u8> for NodeId {
    fn from(value: u8) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GroupId(u8);

impl GroupId {
    #[must_use]
    pub const fn new(value: u8) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

impl From<u8> for GroupId {
    fn from(value: u8) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SessionId(u16);

impl SessionId {
    pub const MIN: Self = Self(1);
    pub const MAX: Self = Self(u16::MAX);

    #[must_use]
    pub const fn new(value: u16) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug)]
pub struct SessionIdAllocator {
    next: u16,
    active: BTreeSet<u16>,
}

impl Default for SessionIdAllocator {
    fn default() -> Self {
        Self {
            next: SessionId::MIN.get(),
            active: BTreeSet::new(),
        }
    }
}

impl SessionIdAllocator {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn starting_at(session_id: SessionId) -> Self {
        Self {
            next: session_id.get(),
            active: BTreeSet::new(),
        }
    }

    /// Allocates and reserves the next available non-zero session ID.
    ///
    /// # Errors
    ///
    /// Returns an error when all non-zero 16-bit IDs are already active.
    pub fn allocate(&mut self) -> Result<SessionId> {
        for _ in 0..u16::MAX {
            let candidate = self.next;
            self.next = if candidate == u16::MAX { 1 } else { candidate + 1 };
            if self.active.insert(candidate) {
                return Ok(SessionId(candidate));
            }
        }
        Err(KlfError::SessionIdsExhausted)
    }

    /// Reserves an externally selected session ID.
    ///
    /// # Errors
    ///
    /// Returns an error when the ID is already active.
    pub fn reserve(&mut self, session_id: SessionId) -> Result<()> {
        if self.active.insert(session_id.get()) {
            Ok(())
        } else {
            Err(KlfError::SessionIdInUse {
                session_id: session_id.get(),
            })
        }
    }

    pub fn release(&mut self, session_id: SessionId) -> bool {
        self.active.remove(&session_id.get())
    }

    #[must_use]
    pub fn is_active(&self, session_id: SessionId) -> bool {
        self.active.contains(&session_id.get())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RawPosition(u16);

impl RawPosition {
    #[must_use]
    pub const fn new(raw: u16) -> Self {
        Self(raw)
    }

    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Percentage {
    raw: u16,
}

impl Percentage {
    pub const FULLY_OPEN: Self = Self { raw: 0 };
    pub const FULLY_CLOSED: Self = Self { raw: 0xC800 };

    #[must_use]
    pub const fn from_percent(percent: u8) -> Self {
        let bounded = if percent > 100 { 100 } else { percent };
        Self {
            raw: (bounded as u16) * 512,
        }
    }

    #[must_use]
    pub const fn raw(self) -> u16 {
        self.raw
    }

    #[must_use]
    pub fn percent(self) -> f64 {
        f64::from(self.raw) / 512.0
    }

    #[must_use]
    pub fn rounded_percent(self) -> u8 {
        let rounded = (u32::from(self.raw) + 256) / 512;
        match u8::try_from(rounded) {
            Ok(percent) => percent.min(100),
            Err(_) => 100,
        }
    }

    const fn from_raw(raw: u16) -> Self {
        Self { raw }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StandardParameter {
    Relative(Percentage),
    NoFeedback,
    /// Percentage-point delta in tenths, in the inclusive range -1000..=1000.
    PercentagePoint(i16),
    Target,
    Current,
    Default,
    Ignore,
    Unknown(RawPosition),
}

impl StandardParameter {
    #[must_use]
    pub fn from_raw(raw: u16) -> Self {
        match raw {
            0x0000..=0xC800 => Self::Relative(Percentage::from_raw(raw)),
            0xC900..=0xD0D0 => match i16::try_from(raw - 0xC900) {
                Ok(offset) => Self::PercentagePoint(offset - 1000),
                Err(_) => Self::Unknown(RawPosition::new(raw)),
            },
            0xD100 => Self::Target,
            0xD200 => Self::Current,
            0xD300 => Self::Default,
            0xD400 => Self::Ignore,
            0xF7FF => Self::NoFeedback,
            _ => Self::Unknown(RawPosition::new(raw)),
        }
    }

    #[must_use]
    pub fn to_raw(self) -> RawPosition {
        let raw = match self {
            Self::Relative(percentage) => percentage.raw(),
            Self::NoFeedback => 0xF7FF,
            Self::PercentagePoint(tenths) => {
                let bounded = tenths.clamp(-1000, 1000);
                match u16::try_from(i32::from(bounded) + 1000) {
                    Ok(offset) => offset + 0xC900,
                    Err(_) => 0xC900,
                }
            }
            Self::Target => 0xD100,
            Self::Current => 0xD200,
            Self::Default => 0xD300,
            Self::Ignore => 0xD400,
            Self::Unknown(raw) => raw.get(),
        };
        RawPosition::new(raw)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtocolTimestamp(u32);

impl ProtocolTimestamp {
    #[must_use]
    pub const fn from_unix_seconds(seconds: u32) -> Self {
        Self(seconds)
    }

    #[must_use]
    pub const fn unix_seconds(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NetworkSetup {
    pub ip_address: [u8; 4],
    pub subnet_mask: [u8; 4],
    pub default_gateway: [u8; 4],
    pub dhcp: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Alias {
    pub kind: u16,
    pub value: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActuatorSet {
    bytes: [u8; ACTUATOR_BYTES],
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BeaconSet(u8);

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NodeSet {
    pub actuators: ActuatorSet,
    pub beacons: BeaconSet,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupInformation {
    pub group_id: GroupId,
    pub order: u16,
    pub placement: u8,
    pub name: String,
    pub velocity: u8,
    pub node_variation: u8,
    pub group_type: u8,
    pub object_count: u8,
    pub actuators: ActuatorSet,
    pub revision: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewGroupInformation {
    pub order: u16,
    pub placement: u8,
    pub name: String,
    pub velocity: u8,
    pub node_variation: u8,
    pub group_type: u8,
    pub object_count: u8,
    pub actuators: ActuatorSet,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContactInputLink {
    pub contact_input_id: u8,
    pub assignment: u8,
    pub action_id: u8,
    pub command_originator: u8,
    pub priority_level: u8,
    pub parameter_id: u8,
    pub position: StandardParameter,
    pub velocity: u8,
    pub lock_priority_level: u8,
    pub priority_level_settings: [u8; 5],
    pub success_output_id: u8,
    pub error_output_id: u8,
}

impl NodeSet {
    #[must_use]
    pub const fn new(actuators: ActuatorSet, beacons: BeaconSet) -> Self {
        Self { actuators, beacons }
    }
}

impl BeaconSet {
    const MASK: u8 = 0b0000_0111;

    #[must_use]
    pub const fn from_byte(byte: u8) -> Self {
        Self(byte & Self::MASK)
    }

    #[must_use]
    pub const fn as_byte(self) -> u8 {
        self.0
    }

    #[must_use]
    pub const fn contains(self, index: u8) -> bool {
        index < 3 && self.0 & (1 << index) != 0
    }

    pub fn set(&mut self, index: u8, enabled: bool) -> bool {
        if index >= 3 {
            return false;
        }
        if enabled {
            self.0 |= 1 << index;
        } else {
            self.0 &= !(1 << index);
        }
        true
    }
}

impl Default for ActuatorSet {
    fn default() -> Self {
        Self {
            bytes: [0; ACTUATOR_BYTES],
        }
    }
}

impl ActuatorSet {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub const fn from_bytes(bytes: [u8; ACTUATOR_BYTES]) -> Self {
        Self { bytes }
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; ACTUATOR_BYTES] {
        &self.bytes
    }

    #[must_use]
    pub fn contains(&self, index: usize) -> bool {
        index < ACTUATOR_COUNT && self.bytes[index / 8] & (1 << (index % 8)) != 0
    }

    pub fn insert(&mut self, index: usize) -> bool {
        if index >= ACTUATOR_COUNT {
            return false;
        }
        let was_present = self.contains(index);
        self.bytes[index / 8] |= 1 << (index % 8);
        !was_present
    }

    pub fn remove(&mut self, index: usize) -> bool {
        if index >= ACTUATOR_COUNT {
            return false;
        }
        let was_present = self.contains(index);
        self.bytes[index / 8] &= !(1 << (index % 8));
        was_present
    }

    pub fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        (0..ACTUATOR_COUNT).filter(|&index| self.contains(index))
    }
}

/// Decodes a NUL-terminated UTF-8 string from a fixed-width field.
///
/// # Errors
///
/// Returns an error when bytes before the first NUL are not valid UTF-8.
pub fn decode_fixed_string(field: &[u8]) -> Result<String> {
    let end = field.iter().position(|byte| *byte == 0).unwrap_or(field.len());
    std::str::from_utf8(&field[..end])
        .map(str::to_owned)
        .map_err(|_| KlfError::InvalidUtf8)
}

/// Encodes a NUL-terminated UTF-8 string into an `N`-byte field.
///
/// # Errors
///
/// Returns an error when the input contains NUL or does not leave room for a terminator.
pub fn encode_fixed_string<const N: usize>(value: &str) -> Result<[u8; N]> {
    let maximum = N.saturating_sub(1);
    if value.as_bytes().contains(&0) {
        return Err(KlfError::InvalidRequest {
            message: "fixed-width strings cannot contain NUL bytes",
        });
    }
    if value.len() > maximum {
        return Err(KlfError::StringTooLong {
            actual: value.len(),
            maximum,
        });
    }

    let mut field = [0; N];
    field[..value.len()].copy_from_slice(value.as_bytes());
    Ok(field)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_parameters_round_trip_every_raw_value() {
        for raw in u16::MIN..=u16::MAX {
            assert_eq!(StandardParameter::from_raw(raw).to_raw().get(), raw);
        }
    }

    #[test]
    fn position_boundaries_match_the_protocol() {
        assert_eq!(
            StandardParameter::from_raw(0),
            StandardParameter::Relative(Percentage::FULLY_OPEN)
        );
        assert_eq!(
            StandardParameter::from_raw(0xC800),
            StandardParameter::Relative(Percentage::FULLY_CLOSED)
        );
        assert_eq!(
            StandardParameter::from_raw(0xC900),
            StandardParameter::PercentagePoint(-1000)
        );
        assert_eq!(
            StandardParameter::from_raw(0xCCE8),
            StandardParameter::PercentagePoint(0)
        );
        assert_eq!(
            StandardParameter::from_raw(0xD0D0),
            StandardParameter::PercentagePoint(1000)
        );
        assert_eq!(StandardParameter::from_raw(0xF7FF), StandardParameter::NoFeedback);
    }

    #[test]
    fn fixed_strings_are_nul_terminated_and_checked() {
        let encoded = encode_fixed_string::<8>("V\u{00e9}lux").expect("valid UTF-8 fits");
        assert_eq!(decode_fixed_string(&encoded).expect("round trip"), "V\u{00e9}lux");
        assert_eq!(encoded[6..], [0, 0]);
        assert_eq!(
            encode_fixed_string::<4>("four"),
            Err(KlfError::StringTooLong { actual: 4, maximum: 3 })
        );
        assert_eq!(decode_fixed_string(&[0xFF, 0]), Err(KlfError::InvalidUtf8));
    }

    #[test]
    fn actuator_bits_use_protocol_little_endian_bit_order() {
        let mut set = ActuatorSet::new();
        assert!(set.insert(0));
        assert!(set.insert(7));
        assert!(set.insert(8));
        assert!(set.insert(199));
        assert_eq!(set.as_bytes()[0], 0x81);
        assert_eq!(set.as_bytes()[1], 0x01);
        assert_eq!(set.as_bytes()[24], 0x80);
        assert_eq!(set.iter().collect::<Vec<_>>(), vec![0, 7, 8, 199]);
        assert!(set.remove(7));
        assert!(!set.contains(7));
        assert!(!set.insert(200));
    }

    #[test]
    fn beacon_bits_ignore_reserved_high_bits() {
        let mut beacons = BeaconSet::from_byte(0xFF);
        assert_eq!(beacons.as_byte(), 0b111);
        assert!(beacons.contains(0));
        assert!(beacons.contains(2));
        assert!(!beacons.contains(3));
        assert!(beacons.set(1, false));
        assert_eq!(beacons.as_byte(), 0b101);
        assert!(!beacons.set(3, true));
    }

    #[test]
    fn session_allocator_wraps_and_avoids_active_ids() {
        let mut allocator = SessionIdAllocator::starting_at(SessionId::MAX);
        allocator.reserve(SessionId::MAX).expect("reserve max");
        let allocated = allocator.allocate().expect("wrap to one");
        assert_eq!(allocated, SessionId::MIN);
        assert!(allocator.is_active(allocated));
        assert_eq!(
            allocator.reserve(allocated),
            Err(KlfError::SessionIdInUse { session_id: 1 })
        );
        assert!(allocator.release(allocated));
        assert!(!allocator.release(allocated));
    }
}
