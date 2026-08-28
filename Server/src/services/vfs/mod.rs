mod error;
mod protocol;

use error::*;
pub use protocol::VfsRequest;
use protocol::*;

use std::{collections::HashMap, num::NonZeroU16};

use bit_vec::BitVec;
use log::{debug, warn};
use num_traits::ToPrimitive;

use super::protocol::status;
use crate::vfs::{
    AccessMode, AttachMode, Backend, FileStatus, GRiDPath, ObjectMode, ReadDirection, StatusAction,
};

type Files<B> = HashMap<NonZeroU16, File<<B as Backend>::Handle>>;
type File<H> = VfsFileDescriptor<H>;

pub(crate) struct Vfs<B: Backend> {
    backend: B,
    connection_ids: BitVec,
    files: Files<B>,
}

struct VfsFileDescriptor<H> {
    handle: Option<H>,
    path: Vec<u8>,
    mode: AttachMode,
    access: AccessMode,
    open: bool,
}

impl<B: Backend> Vfs<B> {
    pub fn new(backend: B) -> Self {
        let mut connection_ids = BitVec::from_elem(65536, false);
        connection_ids.set(0, true); // file id can not be 0

        Self {
            backend,
            connection_ids,
            files: HashMap::new(),
        }
    }

    pub fn process_request(&mut self, req: VfsRequest) -> VfsResponse {
        let VfsRequest { header, body } = req;

        debug!(target: "vfs", "received request {body:?} on connection {}", header.servers_conn_id);

        let response = match body {
            VfsRequestBody::GetStatus(body) => self.get_status(&header, body),
            VfsRequestBody::Open(body) => self.open(&header, body),
            VfsRequestBody::Read(body) => self.read(&header, body),
            VfsRequestBody::ReadDesc(body) => self.read_desc(&header, body),
            VfsRequestBody::ReadDirPage(body) => self.read_dir_page(&header, body),
            VfsRequestBody::Write(body) => self.write(&header, body),
            VfsRequestBody::WriteDesc(body) => self.write_desc(&header, body),
            VfsRequestBody::SetStatus(body) => self.set_status(&header, body),
            VfsRequestBody::Seek(body) => self.seek(&header, body),
            VfsRequestBody::Attach(body) => self.attach(&header, body),
            VfsRequestBody::Detach => self.detach(&header),
            VfsRequestBody::Close => self.close(&header),
            VfsRequestBody::Flush => self.flush(&header),
            VfsRequestBody::Unknown(body) => self.unknown(&header, body),
        };

        debug!(target: "vfs", "response with {response:?}");

        response
    }

    #[inline]
    fn get_file<'f>(
        header: &VfsRequestHeader,
        files: &'f mut Files<B>,
    ) -> Result<&'f mut File<B::Handle>, u16> {
        NonZeroU16::new(header.servers_conn_id)
            .as_ref()
            .and_then(|f| files.get_mut(f))
            .ok_or(VFS_ERROR_BAD_CONNECTION)
    }

    #[inline]
    fn get_handle<'f>(file: &'f mut File<B::Handle>) -> Result<&'f mut B::Handle, u16> {
        file.handle.as_mut().ok_or(VFS_ERROR_FILE_NOT_OPEN)
    }

    fn get_status(&mut self, header: &VfsRequestHeader, _body: VfsReadRequest) -> VfsResponse {
        match self._get_status(header) {
            Ok((open, backend_status)) => VfsResponse::GetStatus(VfsGetStatusResponse {
                header: response_header(VfsRequestCode::GetStatus, header, status::OK),
                open,
                access: backend_status.access.into(),
                seek: backend_status.seek,
                file_position: backend_status.file_position,
                file_length: backend_status.file_length,
                num_pages: backend_status.num_pages,
                num_pages_alloc: backend_status.num_pages_alloc,
            }),
            Err(error) => VfsResponse::GetStatus(VfsGetStatusResponse {
                header: response_header(VfsRequestCode::GetStatus, header, error),
                open: false,
                access: VfsAccessMode::Read,
                seek: false,
                file_position: 0,
                file_length: 0,
                num_pages: 0,
                num_pages_alloc: 0,
            }),
        }
    }

    fn _get_status(&mut self, header: &VfsRequestHeader) -> Result<(bool, FileStatus), u16> {
        let file = Self::get_file(header, &mut self.files)?;
        let handle = Self::get_handle(file)?;
        let file_status = self.backend.get_status(handle)?;
        Ok((file.open, file_status))
    }

    fn open(&mut self, header: &VfsRequestHeader, _body: VfsOpenRequest) -> VfsResponse {
        let status = self._open(header).error_code();
        simple_response(VfsRequestCode::Open, header, status)
    }

    fn _open(&mut self, header: &VfsRequestHeader) -> Result<(), u16> {
        let file = Self::get_file(header, &mut self.files)?;
        if file.open {
            return Err(VFS_ERROR_ALREADY_OPEN);
        }

        let path = GRiDPath::try_from(&file.path).map_err(|_| VFS_ERROR_BAD_PARAMETER)?;
        let handle = self.backend.open(path, file.mode, file.access)?;

        file.handle = Some(handle);
        file.open = true;

        Ok(())
    }

    fn read(&mut self, header: &VfsRequestHeader, body: VfsReadRequest) -> VfsResponse {
        let read_result = self._read(header, body);

        let status = read_result.error_code();
        let data = read_result.unwrap_or_default();

        VfsResponse::Read(VfsReadResponse {
            header: response_header(VfsRequestCode::Read, header, status),
            data,
        })
    }

    fn _read(&mut self, header: &VfsRequestHeader, body: VfsReadRequest) -> Result<Vec<u8>, u16> {
        let file = Self::get_file(header, &mut self.files)?;
        if !file.open {
            return Err(VFS_ERROR_FILE_NOT_OPEN);
        }

        self.backend
            .read(Self::get_handle(file)?, body.bounded_length())
    }

    fn read_desc(&mut self, header: &VfsRequestHeader, body: VfsReadRequest) -> VfsResponse {
        let read_result = self._read_desc(header, body);

        let status = read_result.error_code();
        let data = read_result.unwrap_or_default();

        VfsResponse::Read(VfsReadResponse {
            header: response_header(VfsRequestCode::ReadDesc, header, status),
            data,
        })
    }

    fn _read_desc(
        &mut self,
        header: &VfsRequestHeader,
        body: VfsReadRequest,
    ) -> Result<Vec<u8>, u16> {
        let file = Self::get_file(header, &mut self.files)?;
        if !file.open {
            return Err(VFS_ERROR_FILE_NOT_OPEN);
        }

        let length = usize::from(body.data_length).min(VFS_DESCRIPTOR_LENGTH);
        self.backend.read_desc(Self::get_handle(file)?, length)
    }

    fn read_dir_page(&mut self, header: &VfsRequestHeader, body: VfsReadRequest) -> VfsResponse {
        let read_result = self._read_dir_page(header, body);

        let status = read_result.error_code();
        let entries = read_result.unwrap_or_default();

        VfsResponse::ReadDirPage(VfsReadDirPageResponse {
            header: response_header(VfsRequestCode::ReadDirPage, header, status),
            entries,
        })
    }

    fn _read_dir_page(
        &mut self,
        header: &VfsRequestHeader,
        body: VfsReadRequest,
    ) -> Result<Vec<VfsShortDirEntry>, u16> {
        let file = Self::get_file(header, &mut self.files)?;
        if !file.open {
            return Err(VFS_ERROR_FILE_NOT_OPEN);
        }

        let max_entries = usize::from(body.data_length).min(VFS_MAX_DIRECTORY_OBJECTS_PER_PAGE);
        let entries = self.backend.read_dir(
            Self::get_handle(file)?,
            max_entries,
            ReadDirection::Forward,
            ObjectMode::Directory,
        )?;

        Ok(entries
            .into_iter()
            .map(|entry| VfsShortDirEntry {
                name: entry.name.to_vec(),
            })
            .collect())
    }

    fn write(&mut self, header: &VfsRequestHeader, body: VfsWriteRequest<'_>) -> VfsResponse {
        let status = self._write(header, body).error_code();
        simple_response(VfsRequestCode::Write, header, status)
    }

    fn _write(&mut self, header: &VfsRequestHeader, body: VfsWriteRequest<'_>) -> Result<(), u16> {
        if body.data.len() > VFS_MAX_WRITE_LENGTH {
            return Err(VFS_ERROR_BAD_PARAMETER);
        }

        let file = Self::get_file(header, &mut self.files)?;
        if !file.open {
            return Err(VFS_ERROR_FILE_NOT_OPEN);
        }

        self.backend.write(Self::get_handle(file)?, body.data)
    }

    fn write_desc(&mut self, header: &VfsRequestHeader, body: VfsWriteRequest<'_>) -> VfsResponse {
        let status = self._write_desc(header, body).error_code();
        simple_response(VfsRequestCode::WriteDesc, header, status)
    }

    fn _write_desc(
        &mut self,
        header: &VfsRequestHeader,
        body: VfsWriteRequest<'_>,
    ) -> Result<(), u16> {
        if body.data.len() != VFS_DESCRIPTOR_LENGTH {
            return Err(VFS_ERROR_BAD_PARAMETER);
        }

        let file = Self::get_file(header, &mut self.files)?;
        if !file.open {
            return Err(VFS_ERROR_FILE_NOT_OPEN);
        }

        self.backend.write_desc(Self::get_handle(file)?, body.data)
    }

    fn set_status(
        &mut self,
        header: &VfsRequestHeader,
        body: VfsSetStatusRequest<'_>,
    ) -> VfsResponse {
        let status = self._set_status(header, body).error_code();
        simple_response(VfsRequestCode::SetStatus, header, status)
    }

    fn _set_status(
        &mut self,
        header: &VfsRequestHeader,
        body: VfsSetStatusRequest,
    ) -> Result<(), u16> {
        // FIXME(vklachkov): set status called before open

        // let file = Self::get_file(header, &mut self.files)?;

        // let actions = body
        //     .actions
        //     .into_iter()
        //     .map(StatusAction::from)
        //     .collect::<Vec<_>>();

        // self.backend.set_status(Self::get_handle(file)?, &actions)

        Ok(())
    }

    fn seek(&mut self, header: &VfsRequestHeader, body: VfsSeekRequest) -> VfsResponse {
        let status = self._seek(header, body).error_code();
        simple_response(VfsRequestCode::Seek, header, status)
    }

    fn _seek(&mut self, header: &VfsRequestHeader, body: VfsSeekRequest) -> Result<(), u16> {
        let file = Self::get_file(header, &mut self.files)?;
        if !file.open {
            return Err(VFS_ERROR_FILE_NOT_OPEN);
        }

        self.backend
            .seek(Self::get_handle(file)?, body.mode.into(), body.position)
    }

    fn attach(&mut self, header: &VfsRequestHeader, body: VfsAttachRequest<'_>) -> VfsResponse {
        match self._attach(header, body) {
            Ok(conn_id) => VfsResponse::Simple(VfsResponseHeader {
                response: VFS_RESPONSE_BIT | VfsRequestCode::Attach.to_u16().unwrap(),
                servers_conn_id: conn_id.get(),
                requestors_conn_id: header.requestors_conn_id,
                status: status::OK,
            }),
            Err(err) => VfsResponse::Simple(VfsResponseHeader {
                response: VFS_RESPONSE_BIT | VfsRequestCode::Attach.to_u16().unwrap(),
                servers_conn_id: 0,
                requestors_conn_id: header.requestors_conn_id,
                status: err,
            }),
        }
    }

    fn _attach(
        &mut self,
        _header: &VfsRequestHeader,
        body: VfsAttachRequest<'_>,
    ) -> Result<NonZeroU16, u16> {
        let file_id = self.get_free_file_id().ok_or(VFS_ERROR_DEVICE_FULL)?;

        let mode = body.mode.into();
        let access = body.access.into();

        self.backend.is_attachable(&body.path, mode, access)?;

        self.files.insert(
            file_id,
            VfsFileDescriptor {
                handle: None,
                path: body.path.as_bytes().to_vec(),
                mode,
                access,
                open: false,
            },
        );

        self.connection_ids.set(file_id.get().into(), true);

        Ok(file_id)
    }

    fn get_free_file_id(&mut self) -> Option<NonZeroU16> {
        let index = self
            .connection_ids
            .iter()
            .position(|allocated| !allocated)?;

        NonZeroU16::new(index as u16)
    }

    fn detach(&mut self, header: &VfsRequestHeader) -> VfsResponse {
        let status = self._detach(header).error_code();
        simple_response(VfsRequestCode::Detach, header, status)
    }

    fn _detach(&mut self, header: &VfsRequestHeader) -> Result<(), u16> {
        NonZeroU16::new(header.servers_conn_id)
            .as_ref()
            .and_then(|f| self.files.remove(f))
            .ok_or(VFS_ERROR_BAD_CONNECTION)?;

        self.connection_ids
            .set(header.servers_conn_id.into(), false);

        Ok(())
    }

    fn close(&mut self, header: &VfsRequestHeader) -> VfsResponse {
        let status = self._close(header).error_code();
        simple_response(VfsRequestCode::Close, header, status)
    }

    fn _close(&mut self, header: &VfsRequestHeader) -> Result<(), u16> {
        let file = Self::get_file(header, &mut self.files)?;
        if !file.open {
            return Ok(());
        }

        self.backend.close(Self::get_handle(file)?)?;
        file.open = false;

        Ok(())
    }

    fn flush(&mut self, header: &VfsRequestHeader) -> VfsResponse {
        let status = self._flush(header).error_code();
        simple_response(VfsRequestCode::Flush, header, status)
    }

    fn _flush(&mut self, header: &VfsRequestHeader) -> Result<(), u16> {
        let file = Self::get_file(header, &mut self.files)?;
        if !file.open {
            return Err(VFS_ERROR_FILE_NOT_OPEN);
        }

        self.backend.flush(Self::get_handle(file)?)
    }

    fn unknown(&mut self, header: &VfsRequestHeader, body: &[u8]) -> VfsResponse {
        warn!(
            target: "vfs",
            "unsupported request {:#06x} with {} body bytes",
            header.request,
            body.len()
        );

        VfsResponse::Simple(VfsResponseHeader {
            response: VFS_RESPONSE_BIT | header.request,
            servers_conn_id: header.servers_conn_id,
            requestors_conn_id: header.requestors_conn_id,
            status: VFS_ERROR_NOT_SUPPORTED,
        })
    }
}

impl From<AccessMode> for VfsAccessMode {
    fn from(value: AccessMode) -> Self {
        match value {
            AccessMode::Read => Self::Read,
            AccessMode::Write => Self::Write,
            AccessMode::Update => Self::Update,
            AccessMode::UpdateDescriptor => Self::UpdateDescriptor,
            AccessMode::ShortDirectory => Self::ShortDirectory,
            AccessMode::LongDirectory => Self::LongDirectory,
        }
    }
}

impl From<VfsSetStatusAction<'_>> for StatusAction {
    fn from(value: VfsSetStatusAction<'_>) -> Self {
        match value {
            VfsSetStatusAction::SetDirection { direction } => Self::SetDirection(direction.into()),
            VfsSetStatusAction::SetWildcard { pattern } => {
                Self::SetWildcard(pattern.to_owned().into())
            }
            VfsSetStatusAction::SetObjectMode { mode } => Self::SetObjectMode(mode.into()),
            _ => Self::Unsupported,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vfs::{AttachMode, DirEntry, FileStatus, GRiDPath, SeekMode};

    #[derive(Default)]
    struct MockBackend {
        descriptor_reads: usize,
        descriptor_writes: usize,
    }

    struct MockHandle {
        physical_open: bool,
    }

    impl Backend for MockBackend {
        type Handle = MockHandle;

        fn is_attachable(
            &mut self,
            _path: &GRiDPath,
            _mode: AttachMode,
            _access: AccessMode,
        ) -> Result<(), u16> {
            Ok(())
        }

        fn open(
            &mut self,
            _path: &GRiDPath,
            _mode: AttachMode,
            _access: AccessMode,
        ) -> Result<Self::Handle, u16> {
            Ok(MockHandle {
                physical_open: true,
            })
        }

        fn close(&mut self, handle: &mut Self::Handle) -> Result<(), u16> {
            handle.physical_open = false;
            Ok(())
        }

        fn read(&mut self, _handle: &mut Self::Handle, _length: usize) -> Result<Vec<u8>, u16> {
            Ok(Vec::new())
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

        fn read_desc(&mut self, _handle: &mut Self::Handle, length: usize) -> Result<Vec<u8>, u16> {
            self.descriptor_reads += 1;
            Ok(vec![0; length])
        }

        fn write_desc(
            &mut self,
            _handle: &mut Self::Handle,
            _descriptor: &[u8],
        ) -> Result<(), u16> {
            self.descriptor_writes += 1;
            Ok(())
        }

        fn get_status(&mut self, _handle: &mut Self::Handle) -> Result<FileStatus, u16> {
            Ok(FileStatus {
                access: AccessMode::Update,
                seek: true,
                file_position: 0,
                file_length: 0,
                num_pages: 0,
                num_pages_alloc: 0,
            })
        }

        fn set_status(
            &mut self,
            _handle: &mut Self::Handle,
            _actions: &[StatusAction],
        ) -> Result<(), u16> {
            Ok(())
        }

        fn read_dir(
            &mut self,
            _handle: &mut Self::Handle,
            _max_entries: usize,
            _direction: ReadDirection,
            _object_mode: ObjectMode,
        ) -> Result<Vec<DirEntry>, u16> {
            Ok(Vec::new())
        }
    }

    fn header(code: VfsRequestCode, connection_id: u16) -> VfsRequestHeader {
        VfsRequestHeader {
            request: code.to_u16().unwrap(),
            requestors_conn_id: 7,
            servers_conn_id: connection_id,
        }
    }

    fn attach(vfs: &mut Vfs<MockBackend>) -> u16 {
        let path = GRiDPath::try_from(b"`server:Mail`test").unwrap();
        let response = vfs.process_request(VfsRequest {
            header: header(VfsRequestCode::Attach, 0),
            body: VfsRequestBody::Attach(VfsAttachRequest {
                mode: VfsAttachMode::UpdateFile,
                access: VfsAccessMode::Update,
                password: [0; VFS_PASSWORD_SPACE],
                path,
            }),
        });
        let VfsResponse::Simple(response) = response else {
            panic!("attach must return a simple response");
        };
        assert_eq!(response.status, status::OK);
        response.servers_conn_id
    }

    fn open(vfs: &mut Vfs<MockBackend>, connection_id: u16) {
        let response = vfs.process_request(VfsRequest {
            header: header(VfsRequestCode::Open, connection_id),
            body: VfsRequestBody::Open(VfsOpenRequest { num_buf: 1 }),
        });
        let VfsResponse::Simple(response) = response else {
            panic!("open must return a simple response");
        };
        assert_eq!(response.status, status::OK);
    }

    #[test]
    fn attach_allocates_a_closed_handle() {
        let mut vfs = Vfs::new(MockBackend::default());
        let connection_id = attach(&mut vfs);
        let file = vfs
            .files
            .get(&NonZeroU16::new(connection_id).unwrap())
            .unwrap();

        assert!(!file.open);
        assert!(file.handle.is_none());
        assert_eq!(vfs.connection_ids.get(connection_id as usize), Some(true));
    }

    #[test]
    fn detached_connection_id_is_reused() {
        let mut vfs = Vfs::new(MockBackend::default());
        let first_connection_id = attach(&mut vfs);
        let second_connection_id = attach(&mut vfs);
        vfs.process_request(VfsRequest {
            header: header(VfsRequestCode::Detach, first_connection_id),
            body: VfsRequestBody::Detach,
        });

        assert_eq!(attach(&mut vfs), first_connection_id);
        assert_ne!(second_connection_id, first_connection_id);
    }

    #[test]
    fn open_changes_logical_and_physical_state() {
        let mut vfs = Vfs::new(MockBackend::default());
        let connection_id = attach(&mut vfs);
        open(&mut vfs, connection_id);
        let file = vfs
            .files
            .get(&NonZeroU16::new(connection_id).unwrap())
            .unwrap();

        assert!(file.open);
        assert!(file.handle.as_ref().unwrap().physical_open);
    }

    #[test]
    fn close_releases_the_file_but_keeps_the_handle() {
        let mut vfs = Vfs::new(MockBackend::default());
        let connection_id = attach(&mut vfs);
        open(&mut vfs, connection_id);
        vfs.process_request(VfsRequest {
            header: header(VfsRequestCode::Close, connection_id),
            body: VfsRequestBody::Close,
        });
        let file = vfs
            .files
            .get(&NonZeroU16::new(connection_id).unwrap())
            .unwrap();

        assert!(!file.open);
        assert!(!file.handle.as_ref().unwrap().physical_open);
    }

    #[test]
    fn detach_removes_the_handle() {
        let mut vfs = Vfs::new(MockBackend::default());
        let connection_id = attach(&mut vfs);
        vfs.process_request(VfsRequest {
            header: header(VfsRequestCode::Detach, connection_id),
            body: VfsRequestBody::Detach,
        });

        assert!(
            !vfs.files
                .contains_key(&NonZeroU16::new(connection_id).unwrap())
        );
        assert_eq!(vfs.connection_ids.get(connection_id as usize), Some(false));
    }

    #[test]
    fn descriptor_operations_require_an_open_handle() {
        let mut vfs = Vfs::new(MockBackend::default());
        let connection_id = attach(&mut vfs);
        let read = vfs.process_request(VfsRequest {
            header: header(VfsRequestCode::ReadDesc, connection_id),
            body: VfsRequestBody::ReadDesc(VfsReadRequest {
                data_length: VFS_DESCRIPTOR_LENGTH as u16,
            }),
        });
        let VfsResponse::Read(read) = read else {
            panic!("read descriptor must return a read response");
        };
        assert_eq!(read.header.status, VFS_ERROR_FILE_NOT_OPEN);
        assert_eq!(vfs.backend.descriptor_reads, 0);

        let descriptor = vec![0; VFS_DESCRIPTOR_LENGTH];
        let write = vfs.process_request(VfsRequest {
            header: header(VfsRequestCode::WriteDesc, connection_id),
            body: VfsRequestBody::WriteDesc(VfsWriteRequest { data: &descriptor }),
        });
        let VfsResponse::Simple(write) = write else {
            panic!("write descriptor must return a simple response");
        };
        assert_eq!(write.status, VFS_ERROR_FILE_NOT_OPEN);
        assert_eq!(vfs.backend.descriptor_writes, 0);

        open(&mut vfs, connection_id);
        vfs.process_request(VfsRequest {
            header: header(VfsRequestCode::ReadDesc, connection_id),
            body: VfsRequestBody::ReadDesc(VfsReadRequest {
                data_length: VFS_DESCRIPTOR_LENGTH as u16,
            }),
        });
        vfs.process_request(VfsRequest {
            header: header(VfsRequestCode::WriteDesc, connection_id),
            body: VfsRequestBody::WriteDesc(VfsWriteRequest { data: &descriptor }),
        });

        assert_eq!(vfs.backend.descriptor_reads, 1);
        assert_eq!(vfs.backend.descriptor_writes, 1);
    }
}
