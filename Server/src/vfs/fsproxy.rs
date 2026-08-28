use std::{
    fs::{self, OpenOptions},
    io::{self, Read, Seek, Write},
    path::PathBuf,
};

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;

use bstr::BStr;
use log::warn;

use crate::db;

use super::{
    AccessMode, AttachMode, Backend, DirEntry, GRiDFile, GRiDFileDescriptor, GRiDPath,
    GRiDPathComponents, ObjectMode, ReadDirection, SeekMode,
};

const RESOURCE_UNAVAILABLE: u16 = 601; // eGCRscUnav
const NOT_SUPPORTED: u16 = 35; // eNotSupport

const SUBJECT_SUFFIX: &[u8] = b"~Subject~";
const FILE_SYSTEM_SUFFIX: &[u8] = b"~FS~";

const NAME_DEVICE: &[u8] = b"Name Device";
const RESOURCES_FOLDER: &[u8] = b"Resources~Subject~";
const MAIL_DEVICE: &[u8] = b"Mail";
const USER_SUBJECTS: &[u8] = b"User Subjects";
const GROUP_SUBJECTS: &[u8] = b"Group Subjects";
const COMPANY_SUBJECTS: &[u8] = b"Company Subjects";
const SOFTWARE_SUBJECTS: &[u8] = b"Software Subjects";
const SERVER_SUBJECTS: &[u8] = b"Server Subjects";
const SHARED_SUBJECTS: &[u8] = b"Shared Subjects";

const MAIL_DIR: &str = "Mail";
const SENTRY_DIR: &str = "Sentry";
const COMPANIES_DIR: &str = "Companies";
const GROUPS_DIR: &str = "Groups";
const USERS_DIR: &str = "Users";
const SHARED_DIR: &str = "Shared";
const SERVER_DIR: &str = "Server";
const SOFTWARE_DIR: &str = "Software";

const RESOURCE_SUBJECTS: &[&[u8]] = &[
    USER_SUBJECTS,
    GROUP_SUBJECTS,
    COMPANY_SUBJECTS,
    SOFTWARE_SUBJECTS,
    SERVER_SUBJECTS,
    SHARED_SUBJECTS,
];

pub(crate) struct FsProxy {
    company_id: i64,
    group_id: i64,
    user_id: i64,
    root: PathBuf,
}

pub(crate) enum FsHandle {
    File(FsFileHandle),
    Directory(FsDirHandle),
    Resources,
}

pub(crate) struct FsFileHandle {
    file: GRiDFile,
}

pub(crate) struct FsDirHandle {
    path: PathBuf,
    read_offset: usize,
}

impl FsProxy {
    pub(crate) fn new(account: &db::Account, root: PathBuf) -> io::Result<Self> {
        for path in [
            root.join(MAIL_DIR),
            root.join(SENTRY_DIR).join(COMPANIES_DIR),
            root.join(SENTRY_DIR).join(SHARED_DIR),
            root.join(SERVER_DIR),
            root.join(SOFTWARE_DIR),
        ] {
            fs::create_dir_all(path)?;
        }

        Ok(Self {
            company_id: account.company_id,
            group_id: account.group_id,
            user_id: account.id,
            root,
        })
    }

    fn resources_entries() -> Vec<DirEntry> {
        RESOURCE_SUBJECTS
            .iter()
            .map(|name| {
                let mut name = name.to_vec();
                name.extend_from_slice(FILE_SYSTEM_SUFFIX);
                DirEntry { name: name.into() }
            })
            .collect()
    }

    fn is_resources(components: &GRiDPathComponents<'_>) -> bool {
        components.device == Some(BStr::new(NAME_DEVICE))
            && components.folder == Some(BStr::new(RESOURCES_FOLDER))
            && components.file.is_none()
    }

    fn real_path(&self, components: &GRiDPathComponents<'_>) -> Option<PathBuf> {
        let device = components.device?;
        if device == BStr::new(NAME_DEVICE) {
            return None;
        }

        self.subject_real_path(components)
    }

    fn is_subject_root(components: &GRiDPathComponents<'_>) -> bool {
        components.folder.is_none() && components.file.is_none()
    }

    fn subject_real_path(&self, components: &GRiDPathComponents<'_>) -> Option<PathBuf> {
        let device: &[u8] = components.device?.as_ref();
        let mut path = match device {
            MAIL_DEVICE => self.root.join(MAIL_DIR),
            USER_SUBJECTS => self
                .root
                .join(SENTRY_DIR)
                .join(COMPANIES_DIR)
                .join(self.company_id.to_string())
                .join(GROUPS_DIR)
                .join(self.group_id.to_string())
                .join(USERS_DIR)
                .join(self.user_id.to_string()),
            GROUP_SUBJECTS => self
                .root
                .join(SENTRY_DIR)
                .join(COMPANIES_DIR)
                .join(self.company_id.to_string())
                .join(GROUPS_DIR)
                .join(self.group_id.to_string())
                .join(SHARED_DIR),
            COMPANY_SUBJECTS => self
                .root
                .join(SENTRY_DIR)
                .join(COMPANIES_DIR)
                .join(self.company_id.to_string())
                .join(SHARED_DIR),
            SOFTWARE_SUBJECTS => self.root.join(SOFTWARE_DIR),
            SERVER_SUBJECTS => self.root.join(SERVER_DIR),
            SHARED_SUBJECTS => self.root.join(SENTRY_DIR).join(SHARED_DIR),
            _ => return None,
        };

        if let Some(folder) = components.folder {
            Self::push_component(&mut path, folder);
        }
        if let Some(file) = components.file {
            Self::push_component(&mut path, file);
        }

        Some(path)
    }

    fn push_component(path: &mut PathBuf, component: &BStr) {
        let component = Self::strip_subject_suffix(component).unwrap_or(component);
        let component = component
            .iter()
            .map(|byte| match byte {
                b'/' | b'\\' | b'.' => b'_',
                byte => *byte,
            })
            .collect::<Vec<_>>();

        #[cfg(unix)]
        path.push(std::ffi::OsStr::from_bytes(&component));

        #[cfg(not(unix))]
        path.push(String::from_utf8_lossy(&component).into_owned());
    }

    fn strip_subject_suffix(name: &BStr) -> Option<&BStr> {
        name.strip_suffix(SUBJECT_SUFFIX).map(BStr::new)
    }

    fn read(&mut self, handle: &mut FsHandle, length: usize) -> Result<Vec<u8>, u16> {
        let FsHandle::File(f) = handle else {
            return Err(NOT_SUPPORTED);
        };

        // TODO(vklachkov): can we prealloc buffer here?
        let mut buffer = vec![0; length];

        f.file
            .read_exact(&mut buffer)
            .map(|_| buffer)
            .map_err(Self::map_io_err)
    }

    fn write(&mut self, handle: &mut FsHandle, data: &[u8]) -> Result<(), u16> {
        let FsHandle::File(f) = handle else {
            return Err(NOT_SUPPORTED);
        };

        f.file.write_all(data).map_err(Self::map_io_err)
    }

    fn seek(&mut self, handle: &mut FsHandle, mode: SeekMode, position: u32) -> Result<(), u16> {
        let FsHandle::File(f) = handle else {
            return Err(NOT_SUPPORTED);
        };

        let pos = match mode {
            SeekMode::Backward => io::SeekFrom::Current(-i64::from(position)),
            SeekMode::Absolute => io::SeekFrom::Start(u64::from(position)),
            SeekMode::Forward => io::SeekFrom::Current(i64::from(position)),
            SeekMode::FromEnd => io::SeekFrom::End(i64::from(position)),
        };

        f.file.seek(pos).map(|_| ()).map_err(Self::map_io_err)
    }

    fn flush(&mut self, handle: &mut FsHandle) -> Result<(), u16> {
        let FsHandle::File(f) = handle else {
            return Err(NOT_SUPPORTED);
        };

        f.file.flush().map_err(Self::map_io_err)
    }

    fn read_dir(
        &mut self,
        handle: &mut FsHandle,
        max_entries: usize,
        _direction: ReadDirection, // TODO(vklachkov)
        _object_mode: ObjectMode,  // TODO(vklachkov)
    ) -> Result<Vec<DirEntry>, u16> {
        if matches!(handle, FsHandle::Resources) {
            return Ok(Self::resources_entries());
        }

        let FsHandle::Directory(d) = handle else {
            return Err(NOT_SUPPORTED);
        };

        let read_dir = fs::read_dir(&d.path).map_err(|err| {
            warn!(target: "vfs", "failed to read resource {}: {err}", d.path.display());
            RESOURCE_UNAVAILABLE
        })?;

        let mut entries = Vec::with_capacity(max_entries);
        for result in read_dir.skip(d.read_offset).take(max_entries) {
            let entry = match result {
                Ok(entry) => entry,
                Err(err) => {
                    warn!(target: "vfs", "failed to read an entry in {}: {err}", d.path.display());
                    continue;
                }
            };

            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(err) => {
                    warn!(target: "vfs", "failed to read the type of {}: {err}", entry.path().display());
                    continue;
                }
            };

            let mut name = entry.file_name().into_encoded_bytes();
            if file_type.is_dir() {
                name.extend_from_slice(SUBJECT_SUFFIX);
            } else if !file_type.is_file() {
                continue;
            }

            entries.push(DirEntry { name: name.into() });
        }

        // TODO(vklachkov): sort entries.

        d.read_offset += entries.len();

        Ok(entries)
    }

    fn is_attachable(
        &mut self,
        path: &GRiDPath,
        _mode: AttachMode,
        _access: AccessMode,
    ) -> Result<(), u16> {
        let components = path.components();

        if Self::is_resources(&components) {
            return Ok(());
        }

        let _real_path = self.real_path(&components).ok_or(RESOURCE_UNAVAILABLE)?;

        // check is path exists (must not exists if newfile and exists else)

        // if access -- shortdir/longdir, then check is path dir
        // else -- check is path file

        // what I forget?

        Ok(())
    }

    fn read_desc(&mut self, _handle: &mut FsHandle, length: usize) -> Result<Vec<u8>, u16> {
        todo!("read desc length={length} not implemented");
    }

    fn write_desc(&mut self, _handle: &mut FsHandle, descriptor: &[u8]) -> Result<(), u16> {
        todo!(
            "write desc length={} desc={:x?} not implemented",
            descriptor.len(),
            descriptor
        );
    }

    fn get_status(&mut self, _handle: &mut FsHandle) -> Result<super::FileStatus, u16> {
        todo!("get status not implemented")
    }

    fn set_status(
        &mut self,
        _handle: &mut FsHandle,
        actions: &[super::StatusAction],
    ) -> Result<(), u16> {
        eprintln!("set status {actions:?} not implemented");
        Ok(())
    }

    fn open(
        &mut self,
        path: &GRiDPath,
        mode: AttachMode,
        access: AccessMode,
    ) -> Result<FsHandle, u16> {
        let logical_path = path;
        let components = logical_path.components();

        if Self::is_resources(&components) {
            return Ok(FsHandle::Resources);
        }

        let real_path = self.real_path(&components).ok_or(RESOURCE_UNAVAILABLE)?;

        // TODO(vklachkov): handle long directory properly.
        if access == AccessMode::ShortDirectory {
            if Self::is_subject_root(&components) {
                fs::create_dir_all(&real_path).map_err(|err| {
                    warn!(target: "vfs", "failed to create resource {}: {err}", real_path.display());
                    RESOURCE_UNAVAILABLE
                })?;
            }

            return Ok(FsHandle::Directory(FsDirHandle {
                path: real_path,
                read_offset: 0,
            }));
        } else if access == AccessMode::LongDirectory {
            return Err(NOT_SUPPORTED);
        }

        let mut options = OpenOptions::new();

        options.read(true);

        options.write(matches!(
            access,
            AccessMode::Write | AccessMode::Update | AccessMode::UpdateDescriptor
        ));

        if mode == AttachMode::NewFile {
            fs::create_dir_all(real_path.parent().unwrap()).map_err(|_| RESOURCE_UNAVAILABLE)?;
            options.create(true).truncate(true);
        }

        let physical_file = options.open(&real_path).map_err(|err| {
            warn!(target: "vfs", "failed to open {}: {err}", real_path.display());
            RESOURCE_UNAVAILABLE
        })?;

        let file = if mode == AttachMode::NewFile {
            GRiDFile::create(physical_file, GRiDFileDescriptor::default(), &[])
        } else {
            GRiDFile::open(physical_file)
        }
        .map_err(|err| {
            warn!(target: "vfs", "failed to parse GRiD file {}: {err}", real_path.display());
            RESOURCE_UNAVAILABLE
        })?;

        Ok(FsHandle::File(FsFileHandle { file }))
    }

    fn close(&mut self, _handle: &mut FsHandle) -> Result<(), u16> {
        Ok(())
    }

    fn map_io_err(_err: io::Error) -> u16 {
        // FIXME(vklachkov)
        1
    }
}

impl Backend for FsProxy {
    type Handle = FsHandle;

    fn read(&mut self, handle: &mut Self::Handle, length: usize) -> Result<Vec<u8>, u16> {
        FsProxy::read(self, handle, length)
    }

    fn write(&mut self, handle: &mut Self::Handle, data: &[u8]) -> Result<(), u16> {
        FsProxy::write(self, handle, data)
    }

    fn seek(
        &mut self,
        handle: &mut Self::Handle,
        mode: SeekMode,
        position: u32,
    ) -> Result<(), u16> {
        FsProxy::seek(self, handle, mode, position)
    }

    fn flush(&mut self, handle: &mut Self::Handle) -> Result<(), u16> {
        FsProxy::flush(self, handle)
    }

    fn read_dir(
        &mut self,
        handle: &mut Self::Handle,
        max_entries: usize,
        direction: ReadDirection,
        object_mode: ObjectMode,
    ) -> Result<Vec<DirEntry>, u16> {
        FsProxy::read_dir(self, handle, max_entries, direction, object_mode)
    }

    fn is_attachable(
        &mut self,
        path: &GRiDPath,
        mode: AttachMode,
        access: AccessMode,
    ) -> Result<(), u16> {
        FsProxy::is_attachable(self, path, mode, access)
    }

    fn read_desc(&mut self, handle: &mut Self::Handle, length: usize) -> Result<Vec<u8>, u16> {
        FsProxy::read_desc(self, handle, length)
    }

    fn write_desc(&mut self, handle: &mut Self::Handle, descriptor: &[u8]) -> Result<(), u16> {
        FsProxy::write_desc(self, handle, descriptor)
    }

    fn get_status(&mut self, handle: &mut Self::Handle) -> Result<super::FileStatus, u16> {
        FsProxy::get_status(self, handle)
    }

    fn set_status(
        &mut self,
        handle: &mut Self::Handle,
        actions: &[super::StatusAction],
    ) -> Result<(), u16> {
        FsProxy::set_status(self, handle, actions)
    }

    fn open(
        &mut self,
        path: &GRiDPath,
        mode: AttachMode,
        access: AccessMode,
    ) -> Result<Self::Handle, u16> {
        FsProxy::open(self, path, mode, access)
    }

    fn close(&mut self, handle: &mut Self::Handle) -> Result<(), u16> {
        FsProxy::close(self, handle)
    }
}
