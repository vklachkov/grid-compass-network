#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("connection closed")]
    ConnectionClosed,

    #[error("unexpected end of stream")]
    UnexpectedEof,

    #[error("frame is too big")]
    TooBigFrame,

    #[error("malformed escape sequence")]
    MalformedEscape,

    #[error("CRC mismatch")]
    CrcMismatch,

    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}
