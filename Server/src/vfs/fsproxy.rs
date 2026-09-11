use std::{
    fs::{self, OpenOptions},
    io::{self, Read, Seek, Write},
    path::Path,
    sync::Arc,
};

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;

use bstr::{BStr, BString};
use log::{debug, error, warn};
use zerocopy::byteorder::{U16, U32};

use crate::{
    db,
    vfs::{GRiDDate, VfsDirManager, VfsPath},
};

use super::{
    AccessMode, AttachMode, Backend, DIRECTORY_ENTRY_PREAMBLE_LEN, DirEntry, Error, FileStatus,
    GRiDFile, GRiDFileDescriptor, GRiDFileName, GRiDPath, ObjectMode, ReadDirection, Result,
    SeekMode, StatusAction,
};

const BUFFER_SIZE: usize = 512;
const PAGE_SIZE: u32 = 512;

const MAX_DIRECTORY_NAME_LENGTH: usize = 80;

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

const RESOURCE_SUBJECTS: &[&[u8]] = &[
    USER_SUBJECTS,
    GROUP_SUBJECTS,
    COMPANY_SUBJECTS,
    SOFTWARE_SUBJECTS,
    SERVER_SUBJECTS,
    SHARED_SUBJECTS,
];

enum MappedPath {
    File(VfsPath),
    Directory(VfsPath),
    Resources,
}

/// Everything the connection keeps between attach and detach. Directory and
/// status requests are answered from here, without the object being opened.
pub struct FsAttachment {
    target: MappedPath,
    access: AccessMode,
    position: usize,
    listing: Option<Vec<DirEntry>>,
    status: AttachmentStatus,
}

/// Values accepted by set-status. They only steer future requests on the same
/// attachment, so they are kept here and reported back verbatim by get-status.
struct AttachmentStatus {
    direction: ReadDirection,
    object_mode: ObjectMode,
    wildcard: Option<BString>,
}

impl Default for AttachmentStatus {
    fn default() -> Self {
        Self {
            direction: ReadDirection::Forward,
            object_mode: ObjectMode::Byte,
            wildcard: None,
        }
    }
}

pub enum FsHandle {
    File(VfsPath, GRiDFile),
    Directory(VfsPath),
}

pub struct FsProxy {
    company_id: i64,
    group_id: i64,
    user_id: i64,
    dirman: Arc<VfsDirManager>,
}

impl FsProxy {
    pub fn new(account: &db::Account, dirman: Arc<VfsDirManager>) -> io::Result<Self> {
        Ok(Self {
            company_id: account.company_id,
            group_id: account.group_id,
            user_id: account.id,
            dirman,
        })
    }

    fn map_gridpath(&self, path: &GRiDPath) -> Result<MappedPath> {
        let path_components = path.components();

        let device = path_components.device.ok_or(Error::ResourceUnavailable)?;
        let folder = path_components.folder;
        let file = path_components.file;

        if device == BStr::new(NAME_DEVICE) && folder == Some(BStr::new(RESOURCES_FOLDER)) {
            return Ok(MappedPath::Resources);
        }

        let real_path = match device.as_ref() {
            MAIL_DEVICE => self.dirman.mail_file_path(
                folder, //
                file,
            )?,
            USER_SUBJECTS => self.dirman.user_file_path(
                self.company_id, //
                self.group_id,
                self.user_id,
                folder,
                file,
            )?,
            GROUP_SUBJECTS => self.dirman.group_file_path(
                self.company_id, //
                self.group_id,
                folder,
                file,
            )?,
            COMPANY_SUBJECTS => self.dirman.company_file_path(
                self.company_id, //
                folder,
                file,
            )?,
            SOFTWARE_SUBJECTS => self.dirman.software_file_path(
                folder, //
                file,
            )?,
            SERVER_SUBJECTS => self.dirman.server_file_path(
                folder, //
                file,
            )?,
            SHARED_SUBJECTS => self.dirman.shared_file_path(
                folder, //
                file,
            )?,
            _ => return Err(Error::ResourceUnavailable),
        };

        Ok(if file.is_some() {
            MappedPath::File(real_path)
        } else {
            MappedPath::Directory(real_path)
        })
    }

    fn directory_entries(path: &Path) -> Result<Vec<DirEntry>> {
        let mut entries = Vec::new();

        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let mut name = entry.file_name().as_bytes().to_vec();

            if file_type.is_dir() {
                name.extend_from_slice(SUBJECT_SUFFIX);
            } else if !file_type.is_file() {
                continue;
            }

            if name.len() > MAX_DIRECTORY_NAME_LENGTH {
                warn!(
                    target: "vfs",
                    "skipping overlong directory entry: {}",
                    entry.path().display()
                );
                continue;
            }

            entries.push(DirEntry { name: name.into() });
        }

        entries.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(entries)
    }

    fn resource_entries() -> Vec<DirEntry> {
        RESOURCE_SUBJECTS
            .iter()
            .map(|subject| {
                let mut name = subject.to_vec();
                name.extend_from_slice(FILE_SYSTEM_SUFFIX);
                DirEntry { name: name.into() }
            })
            .collect()
    }

    fn file_name(path: &Path) -> Result<GRiDFileName> {
        let name = path
            .file_name()
            .ok_or(Error::ResourceUnavailable)?
            .as_bytes()
            .to_vec();

        GRiDFileName::new(name).map_err(|_| Error::BadParameter)
    }

    fn directory_name(path: &Path) -> Result<GRiDFileName> {
        let mut name = path
            .file_name()
            .ok_or(Error::ResourceUnavailable)?
            .as_bytes()
            .to_vec();

        name.extend_from_slice(SUBJECT_SUFFIX);

        GRiDFileName::new(name).map_err(|_| Error::BadParameter)
    }

    /// Creation time is not recorded by every filesystem, and its absence must
    /// not fail the whole descriptor.
    fn creation_date(metadata: &fs::Metadata) -> GRiDDate {
        metadata
            .created()
            .map_or_else(|_| GRiDDate::never(), Into::into)
    }

    fn is_attachable(
        &mut self,
        path: &GRiDPath,
        _mode: AttachMode,
        access: AccessMode,
    ) -> Result<FsAttachment> {
        let target = self.map_gridpath(path)?;

        // FIXME(vklachkov): check mode and access.
        match &target {
            MappedPath::File(path) => {
                let parent = path.path().parent().ok_or(Error::ResourceUnavailable)?;
                fs::create_dir_all(parent)?;
            }
            MappedPath::Directory(path) => fs::create_dir_all(path.path())?,
            MappedPath::Resources => {}
        }

        Ok(FsAttachment {
            target,
            access,
            position: 0,
            listing: None,
            status: AttachmentStatus::default(),
        })
    }

    fn open(&mut self, attachment: &mut FsAttachment) -> Result<FsHandle> {
        match &attachment.target {
            MappedPath::File(path) => {
                let file = OpenOptions::new()
                    .create(true)
                    .read(true)
                    .write(true)
                    .truncate(false)
                    .open(path.path())?;

                let name = Self::file_name(path.path())?;
                let grid_file = GRiDFile::from_file(file, name).map_err(io::Error::from)?;

                Ok(FsHandle::File(path.clone(), grid_file))
            }
            MappedPath::Directory(path) => Ok(FsHandle::Directory(path.clone())),
            MappedPath::Resources => Err(Error::AccessDenied),
        }
    }

    fn close(&mut self, _handle: &mut FsHandle) -> Result<()> {
        Ok(())
    }

    fn read(&mut self, handle: &mut FsHandle, length: usize) -> Result<Vec<u8>> {
        if length >= BUFFER_SIZE {
            return Err(Error::BadParameter);
        }

        let FsHandle::File(_, file) = handle else {
            return Err(Error::AccessDenied);
        };

        let mut buffer = vec![0; length];
        let read = file.read(&mut buffer)?;
        buffer.truncate(read);

        Ok(buffer)
    }

    fn write(&mut self, handle: &mut FsHandle, data: &[u8]) -> Result<()> {
        let FsHandle::File(_, file) = handle else {
            return Err(Error::AccessDenied);
        };

        file.write_all(data)?;

        Ok(())
    }

    fn seek(&mut self, handle: &mut FsHandle, mode: SeekMode, position: u32) -> Result<()> {
        let FsHandle::File(_, file) = handle else {
            return Err(Error::AccessDenied);
        };

        let pos = match mode {
            SeekMode::Backward => io::SeekFrom::Current(-i64::from(position)),
            SeekMode::Absolute => io::SeekFrom::Start(u64::from(position)),
            SeekMode::Forward => io::SeekFrom::Current(i64::from(position)),
            SeekMode::FromEnd => io::SeekFrom::End(-i64::from(position)),
        };

        file.seek(pos)?;

        Ok(())
    }

    fn truncate(&mut self, handle: &mut FsHandle) -> Result<()> {
        let FsHandle::File(_, file) = handle else {
            return Err(Error::ResourceUnavailable);
        };

        file.truncate().map_err(io::Error::from)?;

        Ok(())
    }

    fn flush(&mut self, handle: &mut FsHandle) -> Result<()> {
        let FsHandle::File(_, file) = handle else {
            return Err(Error::AccessDenied);
        };

        file.flush()?;

        Ok(())
    }

    fn read_desc(&mut self, handle: &mut FsHandle, length: usize) -> Result<Vec<u8>> {
        let desc = match handle {
            FsHandle::File(path, file) => {
                let header = file.header();
                let metadata = file.metadata().map_err(io::Error::from)?;

                GRiDFileDescriptor {
                    file_length: U32::new(
                        u32::try_from(file.body_length()?).map_err(|_| Error::BadParameter)?,
                    ),
                    file_name: file.name(),
                    creation_date: Self::creation_date(&metadata),
                    // The descriptor carries a single id, and for a file GRiD
                    // expects the id of the directory holding it.
                    dir_file_id: U16::new(self.dirman.parent_file_id(path)?.get()),
                    last_modified_date: metadata.modified()?.into(),
                    expiration_date: GRiDDate::never(),
                    uses_8087: header.flags & 0b1,
                    version1: header.version_major,
                    version2: header.version_minor,
                    version3: header.version_patch,
                    property_length: header.property_length,
                    ..Default::default()
                }
            }
            FsHandle::Directory(path) => {
                let metadata = fs::metadata(path.path())?;
                let entries = Self::directory_entries(path.path())?;

                let dir_length: usize = entries
                    .iter()
                    .map(|entry| DIRECTORY_ENTRY_PREAMBLE_LEN + entry.name.len())
                    .sum();

                GRiDFileDescriptor {
                    file_name: Self::directory_name(path.path())?,
                    creation_date: Self::creation_date(&metadata),
                    dir_file_id: U16::new(self.dirman.file_id(path)?.get()),
                    last_modified_date: metadata.modified()?.into(),
                    expiration_date: GRiDDate::never(),
                    dir_length: U32::new(u32::try_from(dir_length).unwrap_or(u32::MAX)),
                    dir_count: U16::new(u16::try_from(entries.len()).unwrap_or(u16::MAX)),
                    ..Default::default()
                }
            }
        };

        let mut bytes = desc.to_bytes();
        bytes.truncate(length);

        Ok(bytes)
    }

    fn write_desc(&mut self, _handle: &mut FsHandle, descriptor: &[u8]) -> Result<()> {
        // TODO(vklachkov)
        warn!(target: "vfs", "write desc not implemented: {descriptor:02x?}");
        Ok(())
    }

    fn get_status(
        &mut self,
        attachment: &FsAttachment,
        handle: Option<&mut FsHandle>,
    ) -> Result<FileStatus> {
        let (seek, file_position, file_length) = match handle {
            Some(FsHandle::File(_, file)) => {
                let position = u32::try_from(file.position()).map_err(|_| Error::BadParameter)?;
                let length = u32::try_from(file.body_length()?).map_err(|_| Error::BadParameter)?;
                (true, position, length)
            }
            // Directories are read page by page, so there is nothing seekable
            // and no byte length to report.
            _ => (
                false,
                u32::try_from(attachment.position).unwrap_or(u32::MAX),
                0,
            ),
        };

        let num_pages = file_length.div_ceil(PAGE_SIZE);
        let num_pages = u16::try_from(num_pages).unwrap_or(u16::MAX);

        Ok(FileStatus {
            access: attachment.access,
            seek,
            file_position,
            file_length,
            num_pages,
            num_pages_alloc: num_pages,
        })
    }

    fn set_status(
        &mut self,
        attachment: &mut FsAttachment,
        _handle: Option<&mut FsHandle>,
        actions: &[StatusAction],
    ) -> Result<()> {
        for action in actions {
            match action {
                StatusAction::SetDirection(direction) => {
                    attachment.status.direction = *direction;
                }
                StatusAction::SetObjectMode(mode) => {
                    attachment.status.object_mode = *mode;
                }
                StatusAction::SetWildcard(pattern) => {
                    attachment.status.wildcard = Some(pattern.clone());
                }
                StatusAction::Unsupported => {}
            }

            debug!(target: "vfs", "set status action {action:?}");
        }

        Ok(())
    }

    fn read_dir(
        &mut self,
        attachment: &mut FsAttachment,
        max_entries: usize,
        max_bytes: usize,
    ) -> Result<Vec<DirEntry>> {
        // The listing is taken once and kept for the rest of the walk. Rebuilding
        // it per page costs a full scan of the directory each time, and `read_dir`
        // offers neither a stable order nor a consistent snapshot, so an object
        // created or removed between two pages would shift every later index and
        // make the client see a duplicate or miss a row.
        //
        // FIXME(vklachkov): the whole listing is still materialized at once, which
        // is what sorting by name requires; streaming it would need an index of the
        // directory kept in name order.
        if attachment.listing.is_none() {
            attachment.listing = Some(match &attachment.target {
                MappedPath::Directory(path) => Self::directory_entries(path.path())?,
                MappedPath::Resources => Self::resource_entries(),
                MappedPath::File(_) => {
                    error!(target: "vfs", "read dir on a file attachment");
                    return Err(Error::NotSupported);
                }
            });
        }

        let source_entries = attachment.listing.as_deref().unwrap_or_default();

        // TODO(vklachkov): Is it allow to send multiple frames for read_dir?
        let mut entries = Vec::with_capacity(max_entries);
        let mut page_size = 0;

        while entries.len() < max_entries {
            let Some(entry) = source_entries.get(attachment.position) else {
                break;
            };
            let entry_size = DIRECTORY_ENTRY_PREAMBLE_LEN + entry.name.len();

            if page_size + entry_size > max_bytes {
                break;
            }

            page_size += entry_size;
            entries.push(entry.clone());
            attachment.position += 1;
        }

        Ok(entries)
    }
}

impl Backend for FsProxy {
    type Attachment = FsAttachment;
    type Handle = FsHandle;

    fn is_attachable(
        &mut self,
        path: &GRiDPath,
        mode: AttachMode,
        access: AccessMode,
    ) -> Result<Self::Attachment> {
        FsProxy::is_attachable(self, path, mode, access)
    }

    fn open(&mut self, attachment: &mut Self::Attachment) -> Result<Self::Handle> {
        FsProxy::open(self, attachment)
    }

    fn close(&mut self, handle: &mut Self::Handle) -> Result<()> {
        FsProxy::close(self, handle)
    }

    fn read(&mut self, handle: &mut Self::Handle, length: usize) -> Result<Vec<u8>> {
        FsProxy::read(self, handle, length)
    }

    fn write(&mut self, handle: &mut Self::Handle, data: &[u8]) -> Result<()> {
        FsProxy::write(self, handle, data)
    }

    fn seek(&mut self, handle: &mut Self::Handle, mode: SeekMode, position: u32) -> Result<()> {
        FsProxy::seek(self, handle, mode, position)
    }

    fn truncate(&mut self, handle: &mut Self::Handle) -> Result<()> {
        FsProxy::truncate(self, handle)
    }

    fn flush(&mut self, handle: &mut Self::Handle) -> Result<()> {
        FsProxy::flush(self, handle)
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
        actions: &[StatusAction],
    ) -> Result<()> {
        FsProxy::set_status(self, attachment, handle, actions)
    }

    fn read_dir(
        &mut self,
        attachment: &mut Self::Attachment,
        max_entries: usize,
        max_bytes: usize,
    ) -> Result<Vec<DirEntry>> {
        FsProxy::read_dir(self, attachment, max_entries, max_bytes)
    }
}
