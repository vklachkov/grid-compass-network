use std::{
    fs::{self, OpenOptions},
    io::{self, Read, Seek, Write},
    path::PathBuf,
    sync::Arc,
};

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;

use bstr::BStr;
use log::warn;

use crate::{db, vfs::VfsDirManager};

use super::{
    AccessMode, AttachMode, Backend, Error, FileStatus, GRiDFile, GRiDFileDescriptor, GRiDPath,
    GRiDPathComponents, ObjectMode, ReadDirection, Result, SeekMode, ShortDirEntry, StatusAction,
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
    dirman: Arc<VfsDirManager>,
}

pub struct FsHandle {}

impl FsProxy {
    pub fn new(account: &db::Account, dirman: Arc<VfsDirManager>) -> io::Result<Self> {
        Ok(Self {
            company_id: account.company_id,
            group_id: account.group_id,
            user_id: account.id,
            dirman,
        })
    }

    fn attach(
        &mut self,
        path: &GRiDPath,
        mode: AttachMode,
        access: AccessMode,
    ) -> Result<FsHandle> {
        todo!()
    }

    fn open(&mut self, attachment: &mut FsHandle) -> Result<()> {
        todo!()
    }

    fn close(&mut self, _handle: &mut FsHandle) -> Result<()> {
        todo!()
    }

    fn read(&mut self, handle: &mut FsHandle, length: usize) -> Result<Vec<u8>> {
        todo!()
    }

    fn write(&mut self, handle: &mut FsHandle, data: &[u8]) -> Result<()> {
        todo!()
    }

    fn seek(&mut self, handle: &mut FsHandle, mode: SeekMode, position: u32) -> Result<()> {
        todo!()
    }

    fn flush(&mut self, handle: &mut FsHandle) -> Result<()> {
        todo!()
    }

    fn read_desc(&mut self, handle: &mut FsHandle, length: usize) -> Result<Vec<u8>> {
        todo!()
    }

    fn write_desc(&mut self, handle: &mut FsHandle, descriptor: &[u8]) -> Result<()> {
        todo!()
    }

    fn get_status(&mut self, handle: &mut FsHandle) -> Result<FileStatus> {
        todo!()
    }

    fn set_status(&mut self, handle: &mut FsHandle, actions: &[StatusAction]) -> Result<()> {
        todo!()
    }

    fn read_dir(
        &mut self,
        handle: &mut FsHandle,
        max_entries: usize,
        max_bytes: usize,
    ) -> Result<Vec<ShortDirEntry>> {
        todo!()
    }
}

impl Backend for FsProxy {
    type Handle = FsHandle;

    fn attach(
        &mut self,
        path: &GRiDPath,
        mode: AttachMode,
        access: AccessMode,
    ) -> Result<Self::Handle> {
        FsProxy::attach(self, path, mode, access)
    }

    fn open(&mut self, attachment: &mut Self::Handle) -> Result<()> {
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

    fn flush(&mut self, handle: &mut Self::Handle) -> Result<()> {
        FsProxy::flush(self, handle)
    }

    fn read_desc(&mut self, handle: &mut Self::Handle, length: usize) -> Result<Vec<u8>> {
        FsProxy::read_desc(self, handle, length)
    }

    fn write_desc(&mut self, handle: &mut Self::Handle, descriptor: &[u8]) -> Result<()> {
        FsProxy::write_desc(self, handle, descriptor)
    }

    fn get_status(&mut self, handle: &mut Self::Handle) -> Result<FileStatus> {
        FsProxy::get_status(self, handle)
    }

    fn set_status(
        &mut self,
        handle: &mut Self::Handle,
        actions: &[super::StatusAction],
    ) -> Result<()> {
        FsProxy::set_status(self, handle, actions)
    }

    fn read_dir(
        &mut self,
        attachment: &mut Self::Handle,
        max_entries: usize,
        max_bytes: usize,
    ) -> Result<Vec<ShortDirEntry>> {
        FsProxy::read_dir(self, attachment, max_entries, max_bytes)
    }
}
