use std::io;

#[derive(Debug)]
pub(crate) enum Error {
    NotSupported,
    AccessDenied,
    DeviceFull,
    FileNotFound,
    WriteProtected,
    FileNotOpen,
    BadConnection,
    AlreadyOpen,
    BadParameter,
    FileExists,
    ResourceUnavailable,
    Io(io::Error),
}

pub(crate) type Result<T> = core::result::Result<T, Error>;

/// GRiD has no equivalent of an arbitrary host failure, so only the kinds with
/// a faithful counterpart are translated; the rest keep the original error and
/// reach the operator through the log before degrading to a generic code.
impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        use io::ErrorKind::*;

        match error.kind() {
            NotFound => Self::FileNotFound,
            PermissionDenied => Self::AccessDenied,
            AlreadyExists => Self::FileExists,
            ReadOnlyFilesystem => Self::WriteProtected,
            StorageFull | QuotaExceeded | FileTooLarge => Self::DeviceFull,
            InvalidInput | InvalidData => Self::BadParameter,
            Unsupported => Self::NotSupported,
            _ => Self::Io(error),
        }
    }
}
