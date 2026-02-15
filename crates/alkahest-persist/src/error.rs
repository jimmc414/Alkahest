/// Errors that can occur during save/load operations.
#[derive(Debug, thiserror::Error)]
pub enum PersistError {
    #[error("invalid magic bytes (expected ALKA)")]
    InvalidMagic,

    #[error("unsupported save format version {0}")]
    UnsupportedVersion(u16),

    #[error("file too small ({0} bytes, minimum {1})")]
    FileTooSmall(usize, usize),

    #[error("truncated file: expected {expected} bytes, got {actual}")]
    TruncatedFile { expected: usize, actual: usize },

    #[error("LZ4 decompression failed: {0}")]
    DecompressError(String),

    #[error("invalid chunk size: expected {expected}, got {actual}")]
    InvalidChunkSize { expected: usize, actual: usize },

    #[error("invalid fill chunk data (expected 4 bytes)")]
    InvalidFillChunk,
}
