use std::fmt;
use std::io;

#[derive(Debug)]
pub enum FrameError {
    Validation {
        reason: String,
    },
    FrameTooLarge {
        max: usize,
    },
    UnexpectedEof,
    MalformedFrameMarker {
        marker: u8,
    },
    InvalidCrc {
        expected: u16,
        found: u16,
    },
    Io(io::Error),
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation { reason } => write!(f, "validation error: {reason}"),
            Self::FrameTooLarge { max } => write!(f, "frame is too large: max {max} bytes"),
            Self::UnexpectedEof => f.write_str("unexpected end of frame"),
            Self::MalformedFrameMarker { marker } => {
                write!(f, "malformed frame marker {marker:#04x}")
            }
            Self::InvalidCrc { expected, found } => {
                write!(
                    f,
                    "invalid frame CRC: expected {expected:#06x}, found {found:#06x}"
                )
            }
            Self::Io(err) => err.fmt(f),
        }
    }
}

impl std::error::Error for FrameError {
    /// `Io` forwards rather than wraps: its `Display` is already the inner
    /// error's, so reporting itself as the source would print it twice.
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => err.source(),
            _ => None,
        }
    }
}

impl From<io::Error> for FrameError {
    fn from(err: io::Error) -> Self {
        if err.kind() == io::ErrorKind::UnexpectedEof {
            FrameError::UnexpectedEof
        } else {
            FrameError::Io(err)
        }
    }
}
