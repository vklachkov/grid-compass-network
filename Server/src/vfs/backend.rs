use bstr::BString;

use super::GRiDPath;

pub(crate) trait Backend {
    type Handle;

    fn open(
        &mut self,
        path: &GRiDPath,
        mode: AttachMode,
        access: AccessMode,
    ) -> Result<Self::Handle, u16>;

    fn close(&mut self, handle: &mut Self::Handle) -> Result<(), u16>;

    fn read(&mut self, handle: &mut Self::Handle, length: usize) -> Result<Vec<u8>, u16>;

    fn write(&mut self, handle: &mut Self::Handle, data: &[u8]) -> Result<(), u16>;

    fn seek(&mut self, handle: &mut Self::Handle, mode: SeekMode, position: u32)
    -> Result<(), u16>;

    fn flush(&mut self, handle: &mut Self::Handle) -> Result<(), u16>;

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
