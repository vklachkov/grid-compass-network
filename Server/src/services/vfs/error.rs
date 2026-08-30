use crate::vfs::{Error, Result};
use log::warn;

use super::protocol::{
    VFS_ERROR_ALREADY_OPEN, VFS_ERROR_BAD_CONNECTION, VFS_ERROR_BAD_PARAMETER,
    VFS_ERROR_DEVICE_FULL, VFS_ERROR_FILE_EXISTS, VFS_ERROR_FILE_NOT_OPEN, VFS_ERROR_NOT_SUPPORTED,
    VFS_ERROR_RESOURCE_UNAVAILABLE,
};

pub(super) trait ErrorCodeExt {
    fn error_code(&self) -> u16;
}

impl<T> ErrorCodeExt for Result<T> {
    fn error_code(&self) -> u16 {
        match self {
            Ok(_) => 0,
            Err(error) => error_code(error),
        }
    }
}

pub(super) fn error_code(error: &Error) -> u16 {
    match error {
        Error::NotSupported => VFS_ERROR_NOT_SUPPORTED,
        Error::DeviceFull => VFS_ERROR_DEVICE_FULL,
        Error::FileNotOpen => VFS_ERROR_FILE_NOT_OPEN,
        Error::BadConnection => VFS_ERROR_BAD_CONNECTION,
        Error::AlreadyOpen => VFS_ERROR_ALREADY_OPEN,
        Error::BadParameter => VFS_ERROR_BAD_PARAMETER,
        Error::FileExists => VFS_ERROR_FILE_EXISTS,
        Error::ResourceUnavailable => VFS_ERROR_RESOURCE_UNAVAILABLE,
        Error::Io(error) => {
            warn!(target: "vfs", "backend I/O error: {error}");
            VFS_ERROR_RESOURCE_UNAVAILABLE
        }
    }
}
