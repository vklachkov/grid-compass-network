use bstr::BString;

use super::{GRiDPath, Result};

pub(crate) trait Backend {
    type Handle;

    fn attach(
        &mut self,
        path: &GRiDPath,
        mode: AttachMode,
        access: AccessMode,
    ) -> Result<Self::Handle>;

    fn open(&mut self, attachment: &mut Self::Handle) -> Result<()>;

    fn close(&mut self, handle: &mut Self::Handle) -> Result<()>;

    fn read(&mut self, handle: &mut Self::Handle, length: usize) -> Result<Vec<u8>>;

    fn write(&mut self, handle: &mut Self::Handle, data: &[u8]) -> Result<()>;

    fn seek(&mut self, handle: &mut Self::Handle, mode: SeekMode, position: u32) -> Result<()>;

    fn flush(&mut self, handle: &mut Self::Handle) -> Result<()>;

    fn read_desc(&mut self, handle: &mut Self::Handle, length: usize) -> Result<Vec<u8>>;

    fn write_desc(&mut self, handle: &mut Self::Handle, descriptor: &[u8]) -> Result<()>;

    fn get_status(&mut self, handle: &mut Self::Handle) -> Result<FileStatus>;

    fn set_status(&mut self, handle: &mut Self::Handle, actions: &[StatusAction]) -> Result<()>;

    fn read_dir(
        &mut self,
        handle: &mut Self::Handle,
        max_entries: usize,
        max_bytes: usize,
    ) -> Result<Vec<ShortDirEntry>>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttachMode {
    OldFile,
    UpdateFile,
    NewFile,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessMode {
    Read,
    Write,
    Update,
    UpdateDescriptor,
    ShortDirectory,
    LongDirectory,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeekMode {
    Backward,
    Absolute,
    Forward,
    FromEnd,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadDirection {
    Forward,
    Backward,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectMode {
    Byte,
    Directory,
    CompleteDirectory,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShortDirEntry {
    pub name: BString,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileStatus {
    pub access: AccessMode,
    pub seek: bool,
    pub file_position: u32,
    pub file_length: u32,
    pub num_pages: u16,
    pub num_pages_alloc: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StatusAction {
    SetDirection(ReadDirection),
    SetWildcard(BString),
    SetObjectMode(ObjectMode),
    Unsupported,
}
