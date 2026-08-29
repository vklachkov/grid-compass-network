use std::io;

#[derive(Debug)]
pub(crate) enum Error {
    NotSupported,
    DeviceFull,
    FileNotOpen,
    BadConnection,
    AlreadyOpen,
    BadParameter,
    FileExists,
    ResourceUnavailable,
    Io(io::Error),
}

pub(crate) type Result<T> = core::result::Result<T, Error>;

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
