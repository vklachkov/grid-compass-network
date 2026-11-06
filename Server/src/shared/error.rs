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
    Framing(crate::frame::Error),
    Io(io::Error),
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation { reason } => write!(f, "validation error: {reason}"),
            Self::FrameTooLarge { max } => write!(f, "frame is too large: max {max} bytes"),
            Self::UnexpectedEof => f.write_str("unexpected end of frame"),
            Self::Framing(err) => err.fmt(f),
            Self::Io(err) => err.fmt(f),
        }
    }
}

impl std::error::Error for FrameError {
    /// `Io` forwards rather than wraps: its `Display` is already the inner
    /// error's, so reporting itself as the source would print it twice.
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Framing(err) => Some(err),
            Self::Io(err) => err.source(),
            _ => None,
        }
    }
}

impl From<crate::frame::Error> for FrameError {
    fn from(err: crate::frame::Error) -> Self {
        match err {
            crate::frame::Error::Io(err) => Self::from(err),
            err => Self::Framing(err),
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
