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
    AccessMode, AttachMode, Backend, DIRECTORY_ENTRY_PREAMBLE_LEN, DirEntry, Error, FileStatus,
    GRiDFile, GRiDFileDescriptor, GRiDPath, GRiDPathComponents, ObjectMode, ReadDirection, Result,
    SeekMode, StatusAction,
};

const MAX_DIRECTORY_PAGE_SIZE: usize = 504;

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
}

pub(crate) enum FsAttachment {
    File {
        path: PathBuf,
        mode: AttachMode,
        access: AccessMode,
    },
    Directory(FsDirectory),
    Resources(FsDirectory),
}

pub(crate) struct FsDirectory {
    entries: Vec<DirEntry>,
    read_offset: usize,
    direction: ReadDirection,
    object_mode: ObjectMode,
    wildcard: Option<bstr::BString>,
}

pub(crate) struct FsFileHandle {
    file: GRiDFile,
}

impl FsDirectory {
    fn new(entries: Vec<DirEntry>) -> Self {
        Self {
            entries,
            read_offset: 0,
            direction: ReadDirection::Forward,
            object_mode: ObjectMode::Directory,
            wildcard: None,
        }
    }

    fn new_resources() -> Self {
        Self {
            entries: FsProxy::resources_entries(),
            read_offset: 0,
            direction: ReadDirection::Forward,
            object_mode: ObjectMode::Directory,
            wildcard: None,
        }
    }
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

    fn read(&mut self, handle: &mut FsHandle, length: usize) -> Result<Vec<u8>> {
        let FsHandle::File(f) = handle;

        // TODO(vklachkov): can we prealloc buffer here?
        let mut buffer = vec![0; length];

        let mut offset = 0;
        while offset < buffer.len() {
            match f.file.read(&mut buffer[offset..]) {
                Ok(0) => break,
                Ok(count) => offset += count,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error.into()),
            }
        }
        buffer.truncate(offset);
        Ok(buffer)
    }

    fn write(&mut self, handle: &mut FsHandle, data: &[u8]) -> Result<()> {
        let FsHandle::File(f) = handle;

        f.file.write_all(data).map_err(Into::into)
    }

    fn seek(&mut self, handle: &mut FsHandle, mode: SeekMode, position: u32) -> Result<()> {
        let FsHandle::File(f) = handle;

        let pos = match mode {
            SeekMode::Backward => io::SeekFrom::Current(-i64::from(position)),
            SeekMode::Absolute => io::SeekFrom::Start(u64::from(position)),
            SeekMode::Forward => io::SeekFrom::Current(i64::from(position)),
            SeekMode::FromEnd => io::SeekFrom::End(-i64::from(position)),
        };

        f.file.seek(pos).map(|_| ()).map_err(Into::into)
    }

    fn flush(&mut self, handle: &mut FsHandle) -> Result<()> {
        let FsHandle::File(f) = handle;

        f.file.flush().map_err(Into::into)
    }

    fn read_dir(
        &mut self,
        attachment: &mut FsAttachment,
        max_entries: usize,
        max_bytes: usize,
    ) -> Result<Vec<DirEntry>> {
        let directory = match attachment {
            FsAttachment::Directory(directory) | FsAttachment::Resources(directory) => directory,
            FsAttachment::File { .. } => return Err(Error::NotSupported),
        };
        let mut entries = directory.entries.clone();
        if let Some(pattern) = &directory.wildcard {
            entries.retain(|entry| Self::matches_wildcard(entry.name.as_ref(), pattern.as_ref()));
        }
        if directory.direction == ReadDirection::Backward {
            entries.reverse();
        }
        let mut page = Vec::new();
        let mut page_bytes = 0;
        for entry in entries
            .into_iter()
            .skip(directory.read_offset)
            .take(max_entries)
        {
            let entry_bytes = DIRECTORY_ENTRY_PREAMBLE_LEN + entry.name.len();
            if entry.name.len() > 80
                || page_bytes + entry_bytes > max_bytes
                || page_bytes + entry_bytes > MAX_DIRECTORY_PAGE_SIZE
            {
                break;
            }
            page_bytes += entry_bytes;
            page.push(entry);
        }
        directory.read_offset += page.len();
        Ok(page)
    }

    fn matches_wildcard(name: &BStr, pattern: &BStr) -> bool {
        let name = name.to_ascii_lowercase();
        let pattern = pattern.to_ascii_lowercase();
        Self::wildcard_match(&name, &pattern)
    }

    fn wildcard_match(name: &[u8], pattern: &[u8]) -> bool {
        let (mut name_index, mut pattern_index) = (0, 0);
        let (mut wildcard, mut retry_name) = (None, 0);

        while name_index < name.len() {
            if pattern.get(pattern_index) == Some(&name[name_index]) {
                name_index += 1;
                pattern_index += 1;
            } else if pattern.get(pattern_index) == Some(&0xf7) {
                wildcard = Some(pattern_index);
                pattern_index += 1;
                retry_name = name_index;
            } else if let Some(wildcard_index) = wildcard {
                retry_name += 1;
                name_index = retry_name;
                pattern_index = wildcard_index + 1;
            } else {
                return false;
            }
        }

        pattern[pattern_index..].iter().all(|byte| *byte == 0xf7)
    }

    fn is_attachable(
        &mut self,
        path: &GRiDPath,
        mode: AttachMode,
        access: AccessMode,
    ) -> Result<FsAttachment> {
        let components = path.components();

        if Self::is_resources(&components) {
            return if access == AccessMode::ShortDirectory {
                Ok(FsAttachment::Resources(FsDirectory::new_resources()))
            } else {
                Err(Error::ResourceUnavailable)
            };
        }

        let real_path = self
            .real_path(&components)
            .ok_or(Error::ResourceUnavailable)?;
        let directory_access = matches!(
            access,
            AccessMode::ShortDirectory | AccessMode::LongDirectory
        );

        if directory_access && Self::is_subject_root(&components) {
            fs::create_dir_all(&real_path).map_err(|err| {
                warn!(target: "vfs", "failed to create resource {}: {err}", real_path.display());
                Error::ResourceUnavailable
            })?;
        }

        if components.file.is_none() && mode == AttachMode::NewFile {
            return Err(Error::ResourceUnavailable);
        }
        if directory_access {
            if mode == AttachMode::NewFile || !real_path.is_dir() {
                return Err(Error::ResourceUnavailable);
            }
            return if access == AccessMode::LongDirectory {
                Err(Error::NotSupported)
            } else {
                let entries = Self::read_directory_entries(&real_path)?;
                Ok(FsAttachment::Directory(FsDirectory::new(entries)))
            };
        }
        if components.file.is_none() || (mode != AttachMode::NewFile && !real_path.is_file()) {
            return Err(Error::ResourceUnavailable);
        }
        if mode == AttachMode::NewFile && real_path.exists() {
            return Err(Error::ResourceUnavailable);
        }

        Ok(FsAttachment::File {
            path: real_path,
            mode,
            access,
        })
    }

    fn read_directory_entries(path: &PathBuf) -> Result<Vec<DirEntry>> {
        let read_dir = fs::read_dir(path).map_err(|err| {
            warn!(target: "vfs", "failed to read resource {}: {err}", path.display());
            Error::ResourceUnavailable
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
            if name.len() > 80 {
                warn!(target: "vfs", "skipping directory entry with an overlong name in {}", path.display());
                continue;
            }
            entries.push(DirEntry { name: name.into() });
        }
        entries.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(entries)
    }

    fn read_desc(&mut self, handle: &mut FsHandle, length: usize) -> Result<Vec<u8>> {
        let FsHandle::File(file) = handle;
        let descriptor = file.file.descriptor().to_bytes();
        Ok(descriptor[..length.min(descriptor.len())].to_vec())
    }

    fn write_desc(&mut self, handle: &mut FsHandle, descriptor: &[u8]) -> Result<()> {
        let FsHandle::File(file) = handle;
        let descriptor =
            GRiDFileDescriptor::from_bytes(descriptor).map_err(|_| Error::BadParameter)?;
        file.file.set_descriptor(descriptor).map_err(Into::into)
    }

    fn get_status(
        &mut self,
        attachment: &FsAttachment,
        handle: Option<&mut FsHandle>,
    ) -> Result<FileStatus> {
        let (access, directory) = match attachment {
            FsAttachment::File { access, .. } => (*access, None),
            FsAttachment::Directory(directory) | FsAttachment::Resources(directory) => {
                (AccessMode::ShortDirectory, Some(directory))
            }
        };
        let (seek, file_position, file_length) = match handle {
            Some(FsHandle::File(file)) => (
                true,
                file.file.position().min(u64::from(u32::MAX)) as u32,
                file.file.descriptor().file_length,
            ),
            None => directory
                .map(|directory| {
                    (
                        false,
                        directory.read_offset as u32,
                        directory.entries.len() as u32,
                    )
                })
                .unwrap_or((false, 0, 0)),
        };
        Ok(FileStatus {
            access,
            seek,
            file_position,
            file_length,
            num_pages: 0,
            num_pages_alloc: 0,
        })
    }

    fn set_status(
        &mut self,
        attachment: &mut FsAttachment,
        _handle: Option<&mut FsHandle>,
        actions: &[StatusAction],
    ) -> Result<()> {
        if actions.iter().any(|action| {
            matches!(
                action,
                StatusAction::Unsupported
                    | StatusAction::SetObjectMode(ObjectMode::Byte | ObjectMode::CompleteDirectory)
            )
        }) {
            return Err(Error::NotSupported);
        }
        let directory = match attachment {
            FsAttachment::Directory(directory) | FsAttachment::Resources(directory) => directory,
            FsAttachment::File { .. } => return Err(Error::NotSupported),
        };
        for action in actions {
            match action {
                StatusAction::SetDirection(value) => directory.direction = *value,
                StatusAction::SetObjectMode(ObjectMode::Directory) => {
                    directory.object_mode = ObjectMode::Directory;
                }
                StatusAction::SetObjectMode(_) => unreachable!(),
                StatusAction::SetWildcard(value) => directory.wildcard = Some(value.clone()),
                StatusAction::Unsupported => unreachable!(),
            }
        }
        directory.read_offset = 0;
        Ok(())
    }

    fn open(&mut self, attachment: &mut FsAttachment) -> Result<FsHandle> {
        let FsAttachment::File { path, mode, access } = attachment else {
            return Err(Error::NotSupported);
        };

        let mut options = OpenOptions::new();
        options.write(matches!(
            access,
            AccessMode::Write | AccessMode::Update | AccessMode::UpdateDescriptor
        ));

        options.read(true);

        let creating = *mode == AttachMode::NewFile;
        if creating {
            fs::create_dir_all(path.parent().unwrap())?;
            options.create_new(true);
        }

        let physical_file = options.open(&*path).map_err(|error| {
            if error.kind() == io::ErrorKind::AlreadyExists {
                Error::FileExists
            } else {
                error.into()
            }
        })?;

        let file = if creating {
            let file_name = path.file_name().ok_or(Error::BadParameter)?;
            GRiDFile::create(
                physical_file,
                GRiDFileDescriptor::new(
                    super::GRiDFileName::new(file_name.as_encoded_bytes())
                        .map_err(io::Error::from)?,
                ),
                &[],
            )?
        } else {
            GRiDFile::open(physical_file)?
        };

        if creating {
            *mode = AttachMode::OldFile;
        }

        Ok(FsHandle::File(FsFileHandle { file }))
    }

    fn close(&mut self, _handle: &mut FsHandle) -> Result<()> {
        Ok(())
    }
}

impl Backend for FsProxy {
    type Attachment = FsAttachment;
    type Handle = FsHandle;

    fn read(&mut self, handle: &mut Self::Handle, length: usize) -> Result<Vec<u8>> {
        FsProxy::read(self, handle, length)
    }

    fn write(&mut self, handle: &mut Self::Handle, data: &[u8]) -> Result<()> {
        FsProxy::write(self, handle, data)
    }

    fn seek(&mut self, handle: &mut Self::Handle, mode: SeekMode, position: u32) -> Result<()> {
        FsProxy::seek(self, handle, mode, position)
    }

    fn flush(&mut self, handle: &mut Self::Handle) -> Result<()> {
        FsProxy::flush(self, handle)
    }

    fn read_dir(
        &mut self,
        attachment: &mut Self::Attachment,
        max_entries: usize,
        max_bytes: usize,
    ) -> Result<Vec<DirEntry>> {
        FsProxy::read_dir(self, attachment, max_entries, max_bytes)
    }

    fn is_attachable(
        &mut self,
        path: &GRiDPath,
        mode: AttachMode,
        access: AccessMode,
    ) -> Result<Self::Attachment> {
        FsProxy::is_attachable(self, path, mode, access)
    }

    fn read_desc(&mut self, handle: &mut Self::Handle, length: usize) -> Result<Vec<u8>> {
        FsProxy::read_desc(self, handle, length)
    }

    fn write_desc(&mut self, handle: &mut Self::Handle, descriptor: &[u8]) -> Result<()> {
        FsProxy::write_desc(self, handle, descriptor)
    }

    fn get_status(
        &mut self,
        attachment: &Self::Attachment,
        handle: Option<&mut Self::Handle>,
    ) -> Result<FileStatus> {
        FsProxy::get_status(self, attachment, handle)
    }

    fn set_status(
        &mut self,
        attachment: &mut Self::Attachment,
        handle: Option<&mut Self::Handle>,
        actions: &[super::StatusAction],
    ) -> Result<()> {
        FsProxy::set_status(self, attachment, handle, actions)
    }

    fn open(&mut self, attachment: &mut Self::Attachment) -> Result<Self::Handle> {
        FsProxy::open(self, attachment)
    }

    fn close(&mut self, handle: &mut Self::Handle) -> Result<()> {
        FsProxy::close(self, handle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn directory(names: &[&[u8]]) -> FsAttachment {
        FsAttachment::Directory(FsDirectory::new(
            names
                .iter()
                .map(|name| DirEntry {
                    name: (*name).into(),
                })
                .collect(),
        ))
    }

    #[test]
    fn wildcard_matches_case_insensitively_and_empty_tail() {
        assert!(FsProxy::matches_wildcard(
            BStr::new(b"Hard Disk~FS~"),
            BStr::new(b"\xf7~fs~\xf7"),
        ));
        assert!(!FsProxy::matches_wildcard(
            BStr::new(b"Hard Disk~Subject~"),
            BStr::new(b"\xf7~fs~\xf7"),
        ));
    }

    #[test]
    fn directory_pages_use_filtered_snapshot_and_direction() {
        let mut attachment = directory(&[b"A~FS~", b"B~Subject~", b"C~FS~"]);
        let FsAttachment::Directory(state) = &mut attachment else {
            unreachable!();
        };
        state.wildcard = Some(b"\xf7~fs~\xf7".as_slice().into());
        state.direction = ReadDirection::Backward;

        let mut proxy = FsProxy {
            company_id: 0,
            group_id: 0,
            user_id: 0,
            root: PathBuf::new(),
        };
        assert_eq!(
            proxy.read_dir(&mut attachment, 1, 504).unwrap()[0].name,
            b"C~FS~".as_slice()
        );
        assert_eq!(
            proxy.read_dir(&mut attachment, 1, 504).unwrap()[0].name,
            b"A~FS~".as_slice()
        );
        assert!(proxy.read_dir(&mut attachment, 1, 504).unwrap().is_empty());
    }
}
