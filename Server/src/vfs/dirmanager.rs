use std::{
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
};

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;

use anyhow::{Context, bail};
use bstr::BStr;

const MAIL_DIR: &str = "Mail";
const SENTRY_DIR: &str = "Sentry";
const COMPANIES_DIR: &str = "Companies";
const GROUPS_DIR: &str = "Groups";
const USERS_DIR: &str = "Users";
const SHARED_DIR: &str = "Shared";
const SERVER_DIR: &str = "Server";
const SOFTWARE_DIR: &str = "Software";

const SUBJECT_SUFFIX: &[u8] = b"~Subject~";

macro_rules! path_err {
    ($action:expr, $path:expr) => {
        || format!(concat!($action, " '{}'"), $path.display())
    };
}

pub struct VfsDirManager(Inner);

struct Inner {
    root_path: PathBuf,
    _root_fd: fs::File,
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
        }))
    }

    fn is_xattr_supported(path: &Path) -> io::Result<()> {
        const KEY: &str = "is-xattr-supported";
        const SUPPORTED: &[u8] = b"supported";

        xattr::set(path, KEY, SUPPORTED)?;

        if xattr::get(path, KEY)? != Some(SUPPORTED.to_owned()) {
            Err(io::ErrorKind::Unsupported.into())
        } else {
            Ok(())
        }
    }

    pub fn create_base_dirs(&self) -> anyhow::Result<()> {
        let root = self.0.root_path.as_path();

        for path in [
            root.join(MAIL_DIR),
            root.join(SENTRY_DIR).join(COMPANIES_DIR),
            root.join(SENTRY_DIR).join(SHARED_DIR),
            root.join(SERVER_DIR),
            root.join(SOFTWARE_DIR),
        ] {
            fs::create_dir_all(&path)
                .with_context(|| format!("create dir at {}", path.display()))?;
        }

        Ok(())
    }

    pub fn mail_file_path(&self, folder: Option<&BStr>, file: Option<&BStr>) -> PathBuf {
        Self::map_file_path(self.0.root_path.join(MAIL_DIR), folder, file)
    }

    pub fn company_file_path(
        &self,
        company_id: i64,
        folder: Option<&BStr>,
        file: Option<&BStr>,
    ) -> PathBuf {
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
    ) -> PathBuf {
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
    ) -> PathBuf {
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

    pub fn software_file_path(&self, folder: Option<&BStr>, file: Option<&BStr>) -> PathBuf {
        Self::map_file_path(self.0.root_path.join(SOFTWARE_DIR), folder, file)
    }

    pub fn server_file_path(&self, folder: Option<&BStr>, file: Option<&BStr>) -> PathBuf {
        Self::map_file_path(self.0.root_path.join(SERVER_DIR), folder, file)
    }

    pub fn shared_file_path(&self, folder: Option<&BStr>, file: Option<&BStr>) -> PathBuf {
        Self::map_file_path(
            self.0.root_path.join(SENTRY_DIR).join(SHARED_DIR),
            folder,
            file,
        )
    }

    fn map_file_path(mut path: PathBuf, folder: Option<&BStr>, file: Option<&BStr>) -> PathBuf {
        fn push_component(path: &mut PathBuf, component: &BStr) {
            let component = component
                .strip_prefix(SUBJECT_SUFFIX)
                .unwrap_or(component.as_ref());

            let component = component
                .iter()
                .map(|byte| match byte {
                    b'/' | b'\\' | b'.' => b'_',
                    byte => *byte,
                })
                .collect::<Vec<_>>();

            #[cfg(unix)]
            path.push(OsStr::from_bytes(component.as_slice()));

            #[cfg(not(unix))]
            unreachable!();
        }

        if let Some(folder) = folder {
            push_component(&mut path, folder);
        }
        if let Some(file) = file {
            push_component(&mut path, file);
        }

        return path;
    }
}
