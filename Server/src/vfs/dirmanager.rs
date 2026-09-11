use std::{
    collections::{HashMap, hash_map::Entry},
    ffi::OsStr,
    fs, io,
    num::NonZeroU16,
    path::{Path, PathBuf},
};

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;

use anyhow::{Context, bail};
use bstr::BStr;
use parking_lot::Mutex;

use super::{Error, Result};
use crate::shared::bitmap::IdMap16;

const MAIL_DIR: &str = "Mail";
const SENTRY_DIR: &str = "Sentry";
const COMPANIES_DIR: &str = "Companies";
const GROUPS_DIR: &str = "Groups";
const USERS_DIR: &str = "Users";
const SHARED_DIR: &str = "Shared";
const SERVER_DIR: &str = "Server";
const SOFTWARE_DIR: &str = "Software";

const SUBJECT_SUFFIX: &[u8] = b"~Subject~";

/// Linux only accepts extended attributes in a namespace, and `user` is the
/// only one writable without privileges.
const XATTR_SUPPORT_KEY: &str = "user.grid.xattr-probe";
const XATTR_FILE_ID_KEY: &str = "user.grid.file-id";

macro_rules! path_err {
    ($action:expr, $path:expr) => {
        || format!(concat!($action, " '{}'"), $path.display())
    };
}

/// A path inside one of the virtual disks, kept together with the disk it
/// belongs to: file ids are only unique within a single disk.
#[derive(Clone, Debug)]
pub struct VfsPath {
    disk: PathBuf,
    path: PathBuf,
}

impl VfsPath {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

pub struct VfsDirManager(Inner);

struct Inner {
    root_path: PathBuf,
    _root_fd: fs::File,
    file_ids: Mutex<HashMap<PathBuf, IdMap16>>,
}

impl VfsDirManager {
    pub fn new(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref();
        let path = fs::canonicalize(path).with_context(path_err!("normalize path", path))?;

        let fd = fs::File::open(&path).with_context(path_err!("open dir", path))?;
        match fd.try_lock() {
            Ok(()) => {}
            Err(fs::TryLockError::WouldBlock) => {
                bail!("vfs dir is already in use: {}", path.display());
            }
            Err(fs::TryLockError::Error(err)) => {
                bail!("failed to lock vfd dir '{}': {err}", path.display());
            }
        }

        let fd_meta = fd
            .metadata()
            .with_context(path_err!("get dir metadata", path))?;

        if !fd_meta.is_dir() {
            bail!("path is not a directory: {}", path.display());
        }

        Self::is_xattr_supported(&path).with_context(path_err!("check xattr support", path))?;

        Ok(Self(Inner {
            root_path: path,
            _root_fd: fd,
            file_ids: Mutex::new(HashMap::new()),
        }))
    }

    fn is_xattr_supported(path: &Path) -> io::Result<()> {
        const SUPPORTED: &[u8] = b"supported";

        xattr::set(path, XATTR_SUPPORT_KEY, SUPPORTED)?;

        if xattr::get(path, XATTR_SUPPORT_KEY)? != Some(SUPPORTED.to_owned()) {
            Err(io::ErrorKind::Unsupported.into())
        } else {
            Ok(())
        }
    }

    /// Ids live in an extended attribute on every object, so the allocator of a
    /// disk has to be rebuilt from its tree before handing out an id, or it
    /// would reuse one that an object created by an earlier run already owns.
    fn collect_file_ids(path: &Path, ids: &mut IdMap16) -> io::Result<()> {
        if let Some(id) = Self::read_file_id(path)? {
            ids.set(id);
        }

        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let entry_path = entry.path();

            if entry.file_type()?.is_dir() {
                Self::collect_file_ids(&entry_path, ids)?;
            } else if let Some(id) = Self::read_file_id(&entry_path)? {
                ids.set(id);
            }
        }

        Ok(())
    }

    /// Returns the id of an object, assigning and persisting one on first use.
    pub fn file_id(&self, path: &VfsPath) -> Result<NonZeroU16> {
        self.allocate_file_id(&path.disk, &path.path)
    }

    /// Returns the id of the directory holding an object.
    pub fn parent_file_id(&self, path: &VfsPath) -> Result<NonZeroU16> {
        let parent = path.path.parent().ok_or(Error::ResourceUnavailable)?;

        if !parent.starts_with(&path.disk) {
            return Err(Error::ResourceUnavailable);
        }

        self.allocate_file_id(&path.disk, parent)
    }

    fn allocate_file_id(&self, disk: &Path, path: &Path) -> Result<NonZeroU16> {
        if let Some(id) = Self::read_file_id(path)? {
            return Ok(id);
        }

        let mut disks = self.0.file_ids.lock();

        let ids = match disks.entry(disk.to_path_buf()) {
            Entry::Occupied(occupied) => occupied.into_mut(),
            Entry::Vacant(vacant) => {
                let mut ids = IdMap16::new();
                Self::collect_file_ids(disk, &mut ids)?;
                vacant.insert(ids)
            }
        };

        // Another attachment may have assigned an id while the tree was scanned.
        if let Some(id) = Self::read_file_id(path)? {
            return Ok(id);
        }

        let id = ids.find_free().ok_or(Error::DeviceFull)?;
        xattr::set(path, XATTR_FILE_ID_KEY, &id.get().to_le_bytes())?;
        ids.set(id);

        Ok(id)
    }

    fn read_file_id(path: &Path) -> io::Result<Option<NonZeroU16>> {
        let Some(raw) = xattr::get(path, XATTR_FILE_ID_KEY)? else {
            return Ok(None);
        };

        let raw = <[u8; 2]>::try_from(raw.as_slice())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "malformed file id"))?;

        NonZeroU16::new(u16::from_le_bytes(raw))
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "zero file id"))
            .map(Some)
    }

    pub fn mail_file_path(&self, folder: Option<&BStr>, file: Option<&BStr>) -> Result<VfsPath> {
        Self::map_file_path(self.0.root_path.join(MAIL_DIR), folder, file)
    }

    pub fn company_file_path(
        &self,
        company_id: i64,
        folder: Option<&BStr>,
        file: Option<&BStr>,
    ) -> Result<VfsPath> {
        Self::map_file_path(
            self.0
                .root_path
                .join(SENTRY_DIR)
                .join(COMPANIES_DIR)
                .join(company_id.to_string())
                .join(SHARED_DIR),
            folder,
            file,
        )
    }

    pub fn group_file_path(
        &self,
        company_id: i64,
        group_id: i64,
        folder: Option<&BStr>,
        file: Option<&BStr>,
    ) -> Result<VfsPath> {
        Self::map_file_path(
            self.0
                .root_path
                .join(SENTRY_DIR)
                .join(COMPANIES_DIR)
                .join(company_id.to_string())
                .join(GROUPS_DIR)
                .join(group_id.to_string())
                .join(SHARED_DIR),
            folder,
            file,
        )
    }

    pub fn user_file_path(
        &self,
        company_id: i64,
        group_id: i64,
        user_id: i64,
        folder: Option<&BStr>,
        file: Option<&BStr>,
    ) -> Result<VfsPath> {
        Self::map_file_path(
            self.0
                .root_path
                .join(SENTRY_DIR)
                .join(COMPANIES_DIR)
                .join(company_id.to_string())
                .join(GROUPS_DIR)
                .join(group_id.to_string())
                .join(USERS_DIR)
                .join(user_id.to_string()),
            folder,
            file,
        )
    }

    pub fn software_file_path(
        &self,
        folder: Option<&BStr>,
        file: Option<&BStr>,
    ) -> Result<VfsPath> {
        Self::map_file_path(self.0.root_path.join(SOFTWARE_DIR), folder, file)
    }

    pub fn server_file_path(&self, folder: Option<&BStr>, file: Option<&BStr>) -> Result<VfsPath> {
        Self::map_file_path(self.0.root_path.join(SERVER_DIR), folder, file)
    }

    pub fn shared_file_path(&self, folder: Option<&BStr>, file: Option<&BStr>) -> Result<VfsPath> {
        Self::map_file_path(
            self.0.root_path.join(SENTRY_DIR).join(SHARED_DIR),
            folder,
            file,
        )
    }

    fn map_file_path(
        disk_path: PathBuf,
        folder: Option<&BStr>,
        file: Option<&BStr>,
    ) -> Result<VfsPath> {
        fn push_component(path: &mut PathBuf, component: &BStr) -> Result<()> {
            let component = component
                .strip_suffix(SUBJECT_SUFFIX)
                .unwrap_or(component.as_ref());

            for &byte in component {
                if byte == 0 || byte == b'/' {
                    return Err(Error::BadParameter);
                }
            }

            if matches!(component, b"." | b"..") {
                return Err(Error::BadParameter);
            }

            #[cfg(unix)]
            path.push(OsStr::from_bytes(component));

            #[cfg(not(unix))]
            const { unreachable!() };

            Ok(())
        }

        let mut path = disk_path.clone();

        if let Some(folder) = folder {
            push_component(&mut path, folder)?;
        }
        if let Some(file) = file {
            push_component(&mut path, file)?;
        }

        Ok(VfsPath {
            disk: disk_path,
            path,
        })
    }
}
