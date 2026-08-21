mod protocol;

pub use protocol::VfsRequest;
use protocol::*;

use std::{collections::HashMap, mem::size_of, num::NonZeroU16};

use bstr::ByteSlice;
use log::{debug, warn};
use num_traits::ToPrimitive;

use super::protocol::status;
use crate::vfs::{Backend, ObjectMode, Path as BackendPath, ReadDirection};

pub(crate) struct Vfs<B: Backend> {
    backend: B,
    connection_id: NonZeroU16,
    files: HashMap<NonZeroU16, VfsFileDescriptor<B::Handle>>,
}

struct VfsFileDescriptor<H> {
    handle: H,
    open: bool,
}

impl<B: Backend> Vfs<B> {
    pub(crate) fn new(backend: B) -> Self {
        Self {
            backend,
            connection_id: NonZeroU16::MIN,
            files: HashMap::new(),
        }
    }

    pub fn process_request(&mut self, req: VfsRequest) -> VfsResponse {
        let VfsRequest { header, body } = req;

        debug!(target: "vfs", "received request {body:?} on connection {}", header.servers_conn_id);

        match body {
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
        }
    }

    fn get_status(&mut self, header: &VfsRequestHeader, _body: VfsReadRequest) -> VfsResponse {
        VfsResponse::GetStatus(VfsGetStatusResponse {
            header: response_header(VfsRequestCode::GetStatus, header, status::OK),
            open: true,
            access: VfsAccessMode::Read,
            seek: true,
            file_position: 0,
            file_length: 0,
            num_pages: 0,
            num_pages_alloc: 0,
        })
    }

    fn open(&mut self, header: &VfsRequestHeader, _body: VfsOpenRequest) -> VfsResponse {
        let error = match self.file_mut(header.servers_conn_id) {
            Some(file) => {
                file.open = true;
                status::OK
            }
            None => VFS_ERROR_BAD_CONNECTION,
        };

        simple_response(VfsRequestCode::Open, header, error)
    }

    fn read(&mut self, header: &VfsRequestHeader, body: VfsReadRequest) -> VfsResponse {
        let result = NonZeroU16::new(header.servers_conn_id)
            .and_then(|conn_id| self.files.get_mut(&conn_id))
            .ok_or(VFS_ERROR_BAD_CONNECTION)
            .and_then(|file| {
                if !file.open {
                    Err(VFS_ERROR_FILE_NOT_OPEN)
                } else {
                    self.backend.read(&mut file.handle, body.bounded_length())
                }
            });
        let (error, data) = match result {
            Ok(data) => (status::OK, data),
            Err(error) => (error, Vec::new()),
        };

        VfsResponse::Read(VfsReadResponse {
            header: response_header(VfsRequestCode::Read, header, error),
            data,
        })
    }

    fn read_desc(&mut self, header: &VfsRequestHeader, body: VfsReadRequest) -> VfsResponse {
        let data_length = usize::from(body.data_length).min(VFS_DESCRIPTOR_LENGTH);
        let mut data = vec![0; data_length];

        if let Some(property_length) = data
            .get_mut(VFS_DESCRIPTOR_PROPERTY_LENGTH_OFFSET..)
            .and_then(|data| data.get_mut(..size_of::<u32>()))
        {
            property_length.copy_from_slice(&0_u32.to_le_bytes());
        }

        VfsResponse::Read(VfsReadResponse {
            header: response_header(VfsRequestCode::ReadDesc, header, status::OK),
            data,
        })
    }

    fn read_dir_page(&mut self, header: &VfsRequestHeader, body: VfsReadRequest) -> VfsResponse {
        let max_entries = usize::from(body.data_length).min(VFS_MAX_DIRECTORY_OBJECTS_PER_PAGE);
        let result = NonZeroU16::new(header.servers_conn_id)
            .and_then(|conn_id| self.files.get_mut(&conn_id))
            .ok_or(VFS_ERROR_BAD_CONNECTION)
            .and_then(|file| {
                self.backend
                    .read_dir(
                        &mut file.handle,
                        max_entries,
                        ReadDirection::Forward,
                        ObjectMode::Directory,
                    )
                    .map(|entries| {
                        entries
                            .into_iter()
                            .map(|entry| VfsShortDirEntry { name: entry.name })
                            .collect()
                    })
            });
        let (error, entries) = match result {
            Ok(entries) => (status::OK, entries),
            Err(error) => (error, Vec::new()),
        };

        VfsResponse::ReadDirPage(VfsReadDirPageResponse {
            header: response_header(VfsRequestCode::ReadDirPage, header, error),
            entries,
        })
    }

    fn write(&mut self, header: &VfsRequestHeader, body: VfsWriteRequest<'_>) -> VfsResponse {
        let error = match NonZeroU16::new(header.servers_conn_id)
            .and_then(|conn_id| self.files.get_mut(&conn_id))
        {
            None => VFS_ERROR_BAD_CONNECTION,
            Some(_) if body.data.len() > VFS_MAX_WRITE_LENGTH => VFS_ERROR_BAD_PARAMETER,
            Some(file) if !file.open => VFS_ERROR_FILE_NOT_OPEN,
            Some(file) => self
                .backend
                .write(&mut file.handle, body.data)
                .err()
                .unwrap_or(status::OK),
        };

        if error != status::OK {
            warn!(target: "vfs", "refused a write with error {error}");
        }

        simple_response(VfsRequestCode::Write, header, error)
    }

    fn write_desc(&mut self, header: &VfsRequestHeader, _body: VfsWriteRequest<'_>) -> VfsResponse {
        simple_response(VfsRequestCode::WriteDesc, header, status::OK)
    }

    fn set_status(
        &mut self,
        header: &VfsRequestHeader,
        _body: VfsSetStatusRequest<'_>,
    ) -> VfsResponse {
        simple_response(VfsRequestCode::SetStatus, header, status::OK)
    }

    fn seek(&mut self, header: &VfsRequestHeader, body: VfsSeekRequest) -> VfsResponse {
        let error = match NonZeroU16::new(header.servers_conn_id)
            .and_then(|conn_id| self.files.get_mut(&conn_id))
        {
            None => VFS_ERROR_BAD_CONNECTION,
            Some(file) if !file.open => VFS_ERROR_FILE_NOT_OPEN,
            Some(file) => self
                .backend
                .seek(&mut file.handle, body.mode.into(), body.position)
                .err()
                .unwrap_or(status::OK),
        };

        if error != status::OK {
            warn!(target: "vfs", "refused a seek with error {error}");
        }

        simple_response(VfsRequestCode::Seek, header, error)
    }

    fn allocate_connection_id(&mut self) -> Option<NonZeroU16> {
        for _ in 0..u16::MAX {
            let conn_id = self.connection_id;
            self.connection_id = conn_id.checked_add(1).unwrap_or(NonZeroU16::MIN);

            if !self.files.contains_key(&conn_id) {
                return Some(conn_id);
            }
        }

        None
    }

    fn attach(&mut self, header: &VfsRequestHeader, body: VfsAttachRequest<'_>) -> VfsResponse {
        let Some(conn_id) = self.allocate_connection_id() else {
            warn!(target: "vfs", "refused attach, no free connection id");

            return VfsResponse::Simple(VfsResponseHeader {
                response: VFS_RESPONSE_BIT
                    | VfsRequestCode::Attach
                        .to_u16()
                        .expect("valid VFS request code"),
                servers_conn_id: 0,
                requestors_conn_id: header.requestors_conn_id,
                error: VFS_ERROR_DEVICE_FULL,
            });
        };

        let path = BackendPath {
            server: body.path.server.as_bytes().to_vec(),
            components: body
                .path
                .components
                .iter()
                .map(|component| component.as_bytes().to_vec())
                .collect(),
        };
        let handle = match self
            .backend
            .open(&path, body.mode.into(), body.access.into())
        {
            Ok(handle) => handle,
            Err(error) => return simple_response(VfsRequestCode::Attach, header, error),
        };

        self.files.insert(
            conn_id,
            VfsFileDescriptor {
                handle,
                open: false,
            },
        );

        let mut response = response_header(VfsRequestCode::Attach, header, status::OK);
        response.servers_conn_id = conn_id.get();
        VfsResponse::Simple(response)
    }

    fn detach(&mut self, header: &VfsRequestHeader) -> VfsResponse {
        let error = match NonZeroU16::new(header.servers_conn_id)
            .and_then(|conn_id| self.files.remove(&conn_id))
        {
            Some(mut file) => self
                .backend
                .close(&mut file.handle)
                .err()
                .unwrap_or(status::OK),
            None => VFS_ERROR_BAD_CONNECTION,
        };

        simple_response(VfsRequestCode::Detach, header, error)
    }

    fn close(&mut self, header: &VfsRequestHeader) -> VfsResponse {
        let error = match NonZeroU16::new(header.servers_conn_id)
            .and_then(|conn_id| self.files.get_mut(&conn_id))
        {
            Some(file) => match self.backend.close(&mut file.handle) {
                Ok(()) => {
                    file.open = false;
                    status::OK
                }
                Err(error) => error,
            },
            None => VFS_ERROR_BAD_CONNECTION,
        };

        simple_response(VfsRequestCode::Close, header, error)
    }

    fn flush(&mut self, header: &VfsRequestHeader) -> VfsResponse {
        let error = match NonZeroU16::new(header.servers_conn_id)
            .and_then(|conn_id| self.files.get_mut(&conn_id))
        {
            Some(file) => self
                .backend
                .flush(&mut file.handle)
                .err()
                .unwrap_or(status::OK),
            None => VFS_ERROR_BAD_CONNECTION,
        };

        simple_response(VfsRequestCode::Flush, header, error)
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
            error: VFS_ERROR_NOT_SUPPORTED,
        })
    }

    fn file_mut(&mut self, connection_id: u16) -> Option<&mut VfsFileDescriptor<B::Handle>> {
        NonZeroU16::new(connection_id).and_then(|conn_id| self.files.get_mut(&conn_id))
    }
}
