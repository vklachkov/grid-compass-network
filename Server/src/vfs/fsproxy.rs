use super::{AccessMode, AttachMode, Backend, DirEntry, ObjectMode, Path, ReadDirection, SeekMode};

const RESOURCES: &[&str] = &["Hard Disk~FS~"];
const HARD_DISK: &[&str] = &[
    "Folder 1~Subject~",
    "Folder 3~Subject~",
    "Folder 2~Subject~",
];
const HARD_DISK_FILES: &[&str] = &["Demo file~Text~"];
const READ_STUB: &[u8] = b"Read stub";

#[derive(Clone, Copy)]
enum Resource {
    Resources,
    HardDisk,
    HardDiskFiles,
    MailObject,
    Unknown,
}

pub(crate) struct FsBackend;

pub(crate) struct FsHandle {
    directory: Directory,
    read_dir_offset: usize,
}

#[derive(Clone, Copy)]
enum Directory {
    Resources,
    HardDisk,
    HardDiskFiles,
    Other,
}

impl FsBackend {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl Backend for FsBackend {
    type Handle = FsHandle;

    fn open(
        &mut self,
        path: &Path,
        _mode: AttachMode,
        _access: AccessMode,
    ) -> Result<Self::Handle, u16> {
        Ok(FsHandle {
            directory: Directory::from_resource(Resource::from_path(path)),
            read_dir_offset: 0,
        })
    }

    fn close(&mut self, _handle: &mut Self::Handle) -> Result<(), u16> {
        Ok(())
    }

    fn read(&mut self, _handle: &mut Self::Handle, length: usize) -> Result<Vec<u8>, u16> {
        Ok(READ_STUB[..READ_STUB.len().min(length)].to_vec())
    }

    fn write(&mut self, _handle: &mut Self::Handle, _data: &[u8]) -> Result<(), u16> {
        Ok(())
    }

    fn seek(
        &mut self,
        _handle: &mut Self::Handle,
        _mode: SeekMode,
        _position: u32,
    ) -> Result<(), u16> {
        Ok(())
    }

    fn flush(&mut self, _handle: &mut Self::Handle) -> Result<(), u16> {
        Ok(())
    }

    fn read_dir(
        &mut self,
        handle: &mut Self::Handle,
        max_entries: usize,
        _direction: ReadDirection,
        _object_mode: ObjectMode,
    ) -> Result<Vec<DirEntry>, u16> {
        let entries = match handle.directory {
            Directory::Resources => RESOURCES,
            Directory::HardDisk => HARD_DISK,
            Directory::HardDiskFiles => HARD_DISK_FILES,
            Directory::Other => return Ok(Vec::new()),
        };

        let page = entries
            .iter()
            .skip(handle.read_dir_offset)
            .take(max_entries)
            .map(|name| DirEntry {
                name: name.as_bytes().to_vec(),
            })
            .collect::<Vec<_>>();
        handle.read_dir_offset += page.len();
        Ok(page)
    }
}

impl Resource {
    fn from_path(path: &Path) -> Self {
        let components = &path.components;
        if components.len() == 2
            && components[0] == b"Name Device"
            && components[1] == b"Resources~Subject~"
        {
            Self::Resources
        } else if components.len() == 1 && components[0] == b"Hard Disk" {
            Self::HardDisk
        } else if components.len() == 2
            && components[0] == b"Hard Disk"
            && components[1] == b"Folder 3~Subject~"
        {
            Self::HardDiskFiles
        } else if components.len() >= 3 && components[0] == b"Mail" && components[1] == b"Mail" {
            Self::MailObject
        } else {
            Self::Unknown
        }
    }
}

impl Directory {
    fn from_resource(resource: Resource) -> Self {
        match resource {
            Resource::Resources => Self::Resources,
            Resource::HardDisk => Self::HardDisk,
            Resource::HardDiskFiles => Self::HardDiskFiles,
            Resource::MailObject | Resource::Unknown => Self::Other,
        }
    }
}
