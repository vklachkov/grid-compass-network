use std::{
    fs, io,
    path::{Path, PathBuf},
};

use anyhow::Context;

const MAIL_DIR: &str = "Mail";
const SENTRY_DIR: &str = "Sentry";
const COMPANIES_DIR: &str = "Companies";
const GROUPS_DIR: &str = "Groups";
const USERS_DIR: &str = "Users";
const SHARED_DIR: &str = "Shared";
const SERVER_DIR: &str = "Server";
const SOFTWARE_DIR: &str = "Software";

pub struct VfsDirManager(Inner);

struct Inner {
    root_path: PathBuf,
    root_fd: fs::File,
}

impl VfsDirManager {
    pub fn new(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref();

        let path = fs::canonicalize(path)
            .with_context(|| format!("canonicalize  dir at {}", path.display()))?;

        Self::is_dir_path(&path)?;

        let fd =
            Self::lock_dir(&path).with_context(|| format!("lock dir at {}", path.display()))?;

        Self::is_xattr_supported(&path)
            .with_context(|| format!("check xattr support at {}", path.display()))?;

        Ok(Self(Inner {
            root_path: path,
            root_fd: fd,
        }))
    }

    fn is_dir_path(path: &Path) -> anyhow::Result<()> {
        let metadata = std::fs::metadata(path)
            .with_context(|| format!("read metadata at {}", path.display()))?;

        if !metadata.is_dir() {
            anyhow::bail!("path is not a directory: {}", path.display());
        }

        Ok(())
    }

    fn lock_dir(path: &Path) -> io::Result<fs::File> {
        let fd = fs::File::open(&path)?;
        fd.lock()?;
        Ok(fd)
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

    pub fn mail_dir(&self) -> PathBuf {
        self.0.root_path.join(MAIL_DIR)
    }

    pub fn company_dir(&self, company_id: i64) -> PathBuf {
        self.0
            .root_path
            .join(SENTRY_DIR)
            .join(COMPANIES_DIR)
            .join(company_id.to_string())
            .join(SHARED_DIR)
    }

    pub fn group_dir(&self, company_id: i64, group_id: i64) -> PathBuf {
        self.0
            .root_path
            .join(SENTRY_DIR)
            .join(COMPANIES_DIR)
            .join(company_id.to_string())
            .join(GROUPS_DIR)
            .join(group_id.to_string())
            .join(SHARED_DIR)
    }

    pub fn user_dir(&self, company_id: i64, group_id: i64, user_id: i64) -> PathBuf {
        self.0
            .root_path
            .join(SENTRY_DIR)
            .join(COMPANIES_DIR)
            .join(company_id.to_string())
            .join(GROUPS_DIR)
            .join(group_id.to_string())
            .join(USERS_DIR)
            .join(user_id.to_string())
    }

    pub fn software_dir(&self) -> PathBuf {
        self.0.root_path.join(SOFTWARE_DIR)
    }

    pub fn server_dir(&self) -> PathBuf {
        self.0.root_path.join(SERVER_DIR)
    }

    pub fn shared_dir(&self) -> PathBuf {
        self.0.root_path.join(SENTRY_DIR).join(SHARED_DIR)
    }
}
