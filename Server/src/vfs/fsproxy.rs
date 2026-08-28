use std::{
    fs::{self, OpenOptions},
    io,
    path::{Path, PathBuf},
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

const SUBJECT_SUFFIX: &[u8] = b"~Subject~";
const FILE_SYSTEM_SUFFIX: &[u8] = b"~FS~";
const READ_STUB: &[u8] = b"Read stub";

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
    File {
        _file: GRiDFile,
    },
    Directory {
        path: PathBuf,
        read_offset: usize,
    },
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

    fn entries_for(&self, path: &GRiDPath) -> Result<Vec<DirEntry>, u16> {
        let components = path.components();
        if Self::is_resources(&components) {
            return Ok(Self::resources_entries());
        }

        let real_path = self.real_path(&components).ok_or(RESOURCE_UNAVAILABLE)?;
        if Self::is_subject_root(&components) {
            fs::create_dir_all(&real_path).map_err(|err| {
                warn!(target: "vfs", "failed to create resource {}: {err}", real_path.display());
                RESOURCE_UNAVAILABLE
            })?;
        }

        self.read_real_directory(&real_path)
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

    fn read_real_directory(&self, path: &Path) -> Result<Vec<DirEntry>, u16> {
        let read_dir = fs::read_dir(path).map_err(|err| {
            warn!(target: "vfs", "failed to read resource {}: {err}", path.display());
            RESOURCE_UNAVAILABLE
        })?;

        let mut entries = Vec::new();
        for result in read_dir {
            let entry = match result {
                Ok(entry) => entry,
                Err(err) => {
                    warn!(target: "vfs", "failed to read an entry in {}: {err}", path.display());
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

        entries.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(entries)
    }

    fn is_attachable(
        &mut self,
        path: &GRiDPath,
        mode: AttachMode,
        access: AccessMode,
    ) -> Result<(), u16> {
        let components = path.components();
        let real_path = self.real_path(&components).ok_or(RESOURCE_UNAVAILABLE)?;

        // check is path exists (must not exists if newfile and exists else)

        // if access -- shortdir/longdir, then check is path dir
        // else -- check is path file

        // what I forget?

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

        let real_path = self.real_path(&components).ok_or(RESOURCE_UNAVAILABLE)?;

        // TODO(vklachkov): handle long directory properly.
        if access == AccessMode::ShortDirectory {
            return Ok(FsHandle::Directory {
                path: real_path,
                read_offset: 0,
            });
        } else if access == AccessMode::LongDirectory {
            return Err(35);  // FIXME(vklachkov): make const for not supported.
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

        Ok(FsHandle::File { _file: file })
    }

    fn close(&mut self, _handle: &mut FsHandle) -> Result<(), u16> {
        Ok(())
    }
}

impl Backend for FsProxy {
    type Handle = FsHandle;

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
        // let FsHandle::Directory {
        //     entries,
        //     read_offset,
        //     ..
        // } = handle
        // else {
        //     return Err(RESOURCE_UNAVAILABLE);
        // };

        // let page = entries
        //     .iter()
        //     .skip(*read_offset)
        //     .take(max_entries)
        //     .cloned()
        //     .collect::<Vec<_>>();

        // *read_offset += page.len();

        // Ok(page)

        todo!()
    }

    fn is_attachable(
        &mut self,
        path: &GRiDPath,
        mode: AttachMode,
        access: AccessMode,
    ) -> Result<(), u16> {
        FsProxy::is_attachable(self, path, mode, access)
    }

    fn read_desc(&mut self, _handle: &mut Self::Handle, _length: usize) -> Result<Vec<u8>, u16> {
        todo!()
    }

    fn write_desc(&mut self, _handle: &mut Self::Handle, _descriptor: &[u8]) -> Result<(), u16> {
        todo!()
    }

    fn get_status(&mut self, _handle: &mut Self::Handle) -> Result<super::FileStatus, u16> {
        todo!()
    }

    fn set_status(
        &mut self,
        _handle: &mut Self::Handle,
        _actions: &[super::StatusAction],
    ) -> Result<(), u16> {
        todo!()
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
