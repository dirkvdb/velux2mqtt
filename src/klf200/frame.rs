use std::fmt;

use bytes::Bytes;

use super::{CommandId, KlfError, Result};

pub const KLF_PROTOCOL_ID: u8 = 0;
const FRAME_OVERHEAD: usize = 5;
const PAYLOAD_LENGTH_OVERHEAD: usize = 3;
const MAX_PAYLOAD_LENGTH: usize = 252;

#[derive(Clone, Eq, PartialEq)]
pub struct Frame {
    pub protocol_id: u8,
    pub command: CommandId,
    pub payload: Bytes,
}

impl fmt::Debug for Frame {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut frame = formatter.debug_struct("Frame");
        frame
            .field("protocol_id", &self.protocol_id)
            .field("command", &self.command);
        if matches!(
            self.command,
            CommandId::GW_PASSWORD_ENTER_REQ | CommandId::GW_PASSWORD_CHANGE_REQ | CommandId::GW_PASSWORD_CHANGE_NTF
        ) {
            frame.field("payload", &"[REDACTED]");
        } else {
            frame.field("payload", &self.payload);
        }
        frame.finish()
    }
}

impl Frame {
    #[must_use]
    pub fn new(command: CommandId, payload: impl Into<Bytes>) -> Self {
        Self {
            protocol_id: KLF_PROTOCOL_ID,
            command,
            payload: payload.into(),
        }
    }

    /// Encodes the frame envelope and XOR checksum.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid protocol identifier or a payload that cannot fit in the
    /// one-byte KLF length field.
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.protocol_id != KLF_PROTOCOL_ID {
            return Err(KlfError::InvalidProtocolId {
                actual: self.protocol_id,
            });
        }
        if self.payload.len() > MAX_PAYLOAD_LENGTH {
            return Err(KlfError::PayloadTooLarge {
                length: self.payload.len(),
            });
        }

        let mut encoded = Vec::with_capacity(self.payload.len() + FRAME_OVERHEAD);
        encoded.push(self.protocol_id);
        encoded.push(u8::try_from(self.payload.len() + PAYLOAD_LENGTH_OVERHEAD).map_err(|_| {
            KlfError::PayloadTooLarge {
                length: self.payload.len(),
            }
        })?);
        encoded.extend_from_slice(&self.command.raw().to_be_bytes());
        encoded.extend_from_slice(&self.payload);
        encoded.push(checksum(&encoded));
        Ok(encoded)
    }

    /// Decodes and validates a complete, unescaped KLF frame.
    ///
    /// # Errors
    ///
    /// Returns an error when the frame is truncated or has an invalid protocol identifier,
    /// declared length, or checksum.
    pub fn decode(encoded: &[u8]) -> Result<Self> {
        if encoded.len() < FRAME_OVERHEAD {
            return Err(KlfError::FrameTooShort { actual: encoded.len() });
        }
        if encoded[0] != KLF_PROTOCOL_ID {
            return Err(KlfError::InvalidProtocolId { actual: encoded[0] });
        }

        let declared = usize::from(encoded[1]);
        let actual = encoded.len() - 2;
        if declared != actual {
            return Err(KlfError::InvalidFrameLength { declared, actual });
        }

        let checksum_index = encoded.len() - 1;
        let expected = checksum(&encoded[..checksum_index]);
        let actual_checksum = encoded[checksum_index];
        if expected != actual_checksum {
            return Err(KlfError::InvalidChecksum {
                expected,
                actual: actual_checksum,
            });
        }

        Ok(Self {
            protocol_id: encoded[0],
            command: CommandId::new(u16::from_be_bytes([encoded[2], encoded[3]])),
            payload: Bytes::copy_from_slice(&encoded[4..checksum_index]),
        })
    }
}

fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0, |checksum, byte| checksum ^ byte)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_and_decodes_golden_get_version_frame() {
        let frame = Frame::new(CommandId::GW_GET_VERSION_REQ, Bytes::new());
        let encoded = frame.encode().expect("encode frame");
        assert_eq!(encoded, [0x00, 0x03, 0x00, 0x08, 0x0B]);
        assert_eq!(Frame::decode(&encoded).expect("decode frame"), frame);
    }

    #[test]
    fn preserves_unknown_command_ids() {
        let frame = Frame::new(CommandId::new(0xDEAD), [1, 2, 3].as_slice());
        let encoded = frame.encode().expect("encode frame");
        assert_eq!(Frame::decode(&encoded).expect("decode frame"), frame);
    }

    #[test]
    fn rejects_short_length_checksum_and_protocol_errors() {
        assert_eq!(Frame::decode(&[0; 4]), Err(KlfError::FrameTooShort { actual: 4 }));
        assert_eq!(
            Frame::decode(&[1, 3, 0, 8, 10]),
            Err(KlfError::InvalidProtocolId { actual: 1 })
        );
        assert_eq!(
            Frame::decode(&[0, 4, 0, 8, 11]),
            Err(KlfError::InvalidFrameLength { declared: 4, actual: 3 })
        );
        assert_eq!(
            Frame::decode(&[0, 3, 0, 8, 10]),
            Err(KlfError::InvalidChecksum {
                expected: 11,
                actual: 10
            })
        );
    }

    #[test]
    fn enforces_one_byte_length_limit() {
        let largest = Frame::new(CommandId::GW_GET_VERSION_REQ, vec![0; MAX_PAYLOAD_LENGTH]);
        assert!(largest.encode().is_ok());

        let too_large = Frame::new(CommandId::GW_GET_VERSION_REQ, vec![0; MAX_PAYLOAD_LENGTH + 1]);
        assert_eq!(
            too_large.encode(),
            Err(KlfError::PayloadTooLarge {
                length: MAX_PAYLOAD_LENGTH + 1
            })
        );
    }
}
