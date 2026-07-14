use thiserror::Error;

use super::CommandId;

pub type Result<T> = std::result::Result<T, KlfError>;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum KlfError {
    #[error("payload is too large for a KLF frame: {length} bytes")]
    PayloadTooLarge { length: usize },

    #[error("KLF frame is too short: expected at least 5 bytes, got {actual}")]
    FrameTooShort { actual: usize },

    #[error("invalid KLF protocol identifier 0x{actual:02x}")]
    InvalidProtocolId { actual: u8 },

    #[error("invalid KLF frame length: header declares {declared} bytes, actual length field covers {actual}")]
    InvalidFrameLength { declared: usize, actual: usize },

    #[error("invalid KLF frame checksum: expected 0x{expected:02x}, got 0x{actual:02x}")]
    InvalidChecksum { expected: u8, actual: u8 },

    #[error("invalid SLIP escape sequence: 0xdb 0x{byte:02x}")]
    InvalidSlipEscape { byte: u8 },

    #[error("SLIP frame ended with an incomplete escape sequence")]
    IncompleteSlipEscape,

    #[error("SLIP frame exceeded the configured maximum of {maximum} bytes")]
    SlipFrameTooLarge { maximum: usize },

    #[error("fixed-width string contains invalid UTF-8")]
    InvalidUtf8,

    #[error("string needs {actual} bytes but the field permits at most {maximum}")]
    StringTooLong { actual: usize, maximum: usize },

    #[error("session ID space is exhausted")]
    SessionIdsExhausted,

    #[error("session ID {session_id} is already active")]
    SessionIdInUse { session_id: u16 },

    #[error("request command {command} is not supported by the typed encoder")]
    UnsupportedRequest { command: CommandId },

    #[error("request command {command} has no known confirmation")]
    MissingConfirmation { command: CommandId },

    #[error("request for {command} timed out")]
    RequestTimeout { command: CommandId },

    #[error("connection attempt timed out")]
    ConnectTimeout,

    #[error("KLF connection is closed")]
    ConnectionClosed,

    #[error("KLF client command channel is closed")]
    ClientClosed,

    #[error("KLF authentication was rejected")]
    AuthenticationRejected,

    #[error("KLF discovery request was rejected with status {status}")]
    DiscoveryRejected { status: u8 },

    #[error("unexpected response to {operation}: {command}")]
    UnexpectedResponse {
        operation: &'static str,
        command: CommandId,
    },

    #[error("I/O error: {message}")]
    Io { message: String },

    #[error("TLS error: {message}")]
    Tls { message: String },

    #[error("protocol error: {message}")]
    Protocol { message: String },

    #[error("certificate file did not contain any certificates")]
    EmptyCertificateFile,

    #[error("command {command} payload is too short: expected at least {expected} bytes, got {actual}")]
    TruncatedPayload {
        command: CommandId,
        expected: usize,
        actual: usize,
    },

    #[error("command {command} payload has invalid length: expected exactly {expected} bytes, got {actual}")]
    InvalidPayloadLength {
        command: CommandId,
        expected: usize,
        actual: usize,
    },

    #[error("invalid request: {message}")]
    InvalidRequest { message: &'static str },
}
