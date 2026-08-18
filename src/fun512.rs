pub const LOG_FUN512_MARKER: &str = "Log FUN512: ";
pub const FUN512_MAX_IDX: u8 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogVerify {
    Valid,
    Mismatch,
    NoChecksum,
    TrailingData,
    IoError,
}
