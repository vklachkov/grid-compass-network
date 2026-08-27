use bstr::BString;

use super::GRiDPath;

pub(crate) trait Backend {
    type Handle;

    fn attach(
        &mut self,
        path: &GRiDPath,
        mode: AttachMode,
        access: AccessMode,
    ) -> Result<Self::Handle, u16>;

    fn open(&mut self, handle: &mut Self::Handle) -> Result<(), u16>;

    fn close(&mut self, handle: &mut Self::Handle) -> Result<(), u16>;

    fn read(&mut self, handle: &mut Self::Handle, length: usize) -> Result<Vec<u8>, u16>;

    fn write(&mut self, handle: &mut Self::Handle, data: &[u8]) -> Result<(), u16>;

    fn seek(&mut self, handle: &mut Self::Handle, mode: SeekMode, position: u32)
    -> Result<(), u16>;

    fn flush(&mut self, handle: &mut Self::Handle) -> Result<(), u16>;

    fn read_desc(&mut self, handle: &mut Self::Handle, length: usize) -> Result<Vec<u8>, u16>;

    fn write_desc(&mut self, handle: &mut Self::Handle, descriptor: &[u8]) -> Result<(), u16>;

    fn get_status(&mut self, handle: &mut Self::Handle) -> Result<FileStatus, u16>;

    fn set_status(
        &mut self,
        handle: &mut Self::Handle,
        actions: &[StatusAction],
    ) -> Result<(), u16>;

    fn read_dir(
        &mut self,
        handle: &mut Self::Handle,
        max_entries: usize,
        direction: ReadDirection,
        object_mode: ObjectMode,
    ) -> Result<Vec<DirEntry>, u16>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AttachMode {
    OldFile,
    UpdateFile,
    NewFile,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AccessMode {
    Read,
    Write,
    Update,
    UpdateDescriptor,
    ShortDirectory,
    LongDirectory,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SeekMode {
    Backward,
    Absolute,
    Forward,
    FromEnd,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReadDirection {
    Forward,
    Backward,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ObjectMode {
    Byte,
    Directory,
    CompleteDirectory,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DirEntry {
    pub name: BString,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FileStatus {
    pub access: AccessMode,
    pub seek: bool,
    pub file_position: u32,
    pub file_length: u32,
    pub num_pages: u16,
    pub num_pages_alloc: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum StatusAction {
    SetDirection(ReadDirection),
    SetWildcard(BString),
    SetObjectMode(ObjectMode),
    Unsupported,
}
