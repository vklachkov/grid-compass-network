mod protocol;

pub use protocol::VfsRequest;

use std::{collections::HashMap, mem::size_of, num::NonZeroU16};

use log::{debug, warn};

use super::protocol::status;

use protocol::{
    HARD_DISK, HARD_DISK_FILES, MAX_MAIL_OBJECT_SIZE, READ_STUB, RESOURCES, VFS_DESCRIPTOR_LEN,
    VFS_DESCRIPTOR_PROPERTY_LENGTH_OFFSET, VFS_ERROR_BAD_CONNECTION, VFS_ERROR_BAD_PARAMETER,
    VFS_ERROR_DEVICE_FULL, VFS_ERROR_FILE_NOT_OPEN, VFS_ERROR_NOT_SUPPORTED, VFS_RESPONSE_BIT,
    VfsAccessMode, VfsAttachRequest, VfsGetStatusResponse, VfsOpenRequest, VfsReadDirPageResponse,
    VfsReadRequest, VfsReadResponse, VfsRequestBody, VfsRequestCode, VfsRequestHeader, VfsResource,
    VfsResponse, VfsResponseHeader, VfsSeekMode, VfsSeekRequest, VfsSetStatusRequest,
    VfsShortDirEntry, VfsWriteRequest, response_header, simple_response,
};

pub struct Vfs {
    connection_id: NonZeroU16,
    files: HashMap<NonZeroU16, VfsFileDescriptor>,
    finalized_mail: Option<Vec<u8>>,
}

struct VfsFileDescriptor {
    resource: VfsResource,
    read_dir_page_offset: usize,
    data: Vec<u8>,
    position: usize,
    open: bool,
    write_failed: bool,
}

impl Vfs {
    pub fn new() -> Self {
        Self {
            connection_id: NonZeroU16::MIN,
            files: HashMap::new(),
            finalized_mail: None,
        }
    }

    pub fn take_finalized_mail(&mut self) -> Option<Vec<u8>> {
        self.finalized_mail.take()
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
            VfsRequestBody::Unknown(body) => self.unknown(&header, body),
        }
    }

    fn get_status(&mut self, header: &VfsRequestHeader, _body: VfsReadRequest) -> VfsResponse {
        VfsResponse::GetStatus(VfsGetStatusResponse {
            header: response_header(VfsRequestCode::GetStatus as u16, header, status::OK),
            // FIXME: replace with real data
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
        if let Some(file) =
            NonZeroU16::new(header.servers_conn_id).and_then(|conn_id| self.files.get_mut(&conn_id))
        {
            file.open = true;
        }

        simple_response(VfsRequestCode::Open as u16, header, status::OK)
    }

    fn read(&mut self, header: &VfsRequestHeader, _body: VfsReadRequest) -> VfsResponse {
        // TODO

        VfsResponse::Read(VfsReadResponse {
            header: response_header(VfsRequestCode::Read as u16, header, status::OK),
            data: READ_STUB.to_vec(),
        })
    }

    fn read_desc(&mut self, header: &VfsRequestHeader, body: VfsReadRequest) -> VfsResponse {
        let data_length = usize::from(body.data_length).min(VFS_DESCRIPTOR_LEN);
        let mut data = vec![0; data_length];

        if let Some(property_length) = data
            .get_mut(VFS_DESCRIPTOR_PROPERTY_LENGTH_OFFSET..)
            .and_then(|data| data.get_mut(..size_of::<u32>()))
        {
            property_length.copy_from_slice(&0_u32.to_le_bytes());
        }

        VfsResponse::Read(VfsReadResponse {
            header: response_header(VfsRequestCode::ReadDesc as u16, header, status::OK),
            data,
        })
    }

    fn read_dir_page(&mut self, header: &VfsRequestHeader, body: VfsReadRequest) -> VfsResponse {
        let entries = NonZeroU16::new(header.servers_conn_id)
            .and_then(|conn_id| self.files.get_mut(&conn_id))
            .map(|file| file.read_dir_page(body.data_length as usize))
            .unwrap_or_default();

        VfsResponse::ReadDirPage(VfsReadDirPageResponse {
            header: response_header(VfsRequestCode::ReadDirPage as u16, header, status::OK),
            entries,
        })
    }

    fn write(&mut self, header: &VfsRequestHeader, body: VfsWriteRequest<'_>) -> VfsResponse {
        let error = match NonZeroU16::new(header.servers_conn_id)
            .and_then(|conn_id| self.files.get_mut(&conn_id))
        {
            None => VFS_ERROR_BAD_CONNECTION,
            Some(file) if !file.open => VFS_ERROR_FILE_NOT_OPEN,
            Some(file) if file.resource != VfsResource::MailObject => VFS_ERROR_BAD_PARAMETER,
            Some(file) => match file.position.checked_add(body.data.len()) {
                Some(end) if end <= MAX_MAIL_OBJECT_SIZE => {
                    if file.data.len() < end {
                        file.data.resize(end, 0);
                    }
                    file.data[file.position..end].copy_from_slice(body.data);
                    file.position = end;
                    status::OK
                }
                _ => {
                    file.write_failed = true;
                    VFS_ERROR_DEVICE_FULL
                }
            },
        };

        if error != status::OK {
            warn!(target: "vfs", "refused a write with error {error}");
        }

        simple_response(VfsRequestCode::Write as u16, header, error)
    }

    fn write_desc(&mut self, header: &VfsRequestHeader, _body: VfsWriteRequest<'_>) -> VfsResponse {
        // TODO

        simple_response(VfsRequestCode::WriteDesc as u16, header, status::OK)
    }

    fn set_status(
        &mut self,
        header: &VfsRequestHeader,
        _body: VfsSetStatusRequest<'_>,
    ) -> VfsResponse {
        // TODO

        simple_response(VfsRequestCode::SetStatus as u16, header, status::OK)
    }

    fn seek(&mut self, header: &VfsRequestHeader, body: VfsSeekRequest) -> VfsResponse {
        let error = match NonZeroU16::new(header.servers_conn_id)
            .and_then(|conn_id| self.files.get_mut(&conn_id))
        {
            None => VFS_ERROR_BAD_CONNECTION,
            Some(file) if !file.open => VFS_ERROR_FILE_NOT_OPEN,
            Some(file) => {
                let offset = usize::try_from(body.position).ok();
                let position = match body.mode {
                    VfsSeekMode::Backward => {
                        offset.and_then(|offset| file.position.checked_sub(offset))
                    }
                    VfsSeekMode::Absolute => offset,
                    VfsSeekMode::Forward => {
                        offset.and_then(|offset| file.position.checked_add(offset))
                    }
                    VfsSeekMode::FromEnd => {
                        offset.and_then(|offset| file.data.len().checked_sub(offset))
                    }
                };

                match position {
                    Some(position) => {
                        // FsSeek forwards absolute positions to the device without
                        // comparing them to the current file length. The write
                        // operation enforces the object-size limit separately.
                        file.position = position;
                        status::OK
                    }
                    None => VFS_ERROR_BAD_PARAMETER,
                }
            }
        };

        if error != status::OK {
            warn!(target: "vfs", "refused a seek with error {error}");
        }

        simple_response(VfsRequestCode::Seek as u16, header, error)
    }

    /// Hands out the next free connection id. Ids are only released on detach,
    /// so after 65535 attaches the counter wraps onto live entries: probing the
    /// whole range keeps that case an error instead of a collision.
    fn allocate_connection_id(&mut self) -> Option<NonZeroU16> {
        for _ in 0..u16::MAX {
            let conn_id = self.connection_id;

            // no wrapping_add() for NonZero types.
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
                response: VFS_RESPONSE_BIT | VfsRequestCode::Attach as u16,
                servers_conn_id: 0,
                requestors_conn_id: header.requestors_conn_id,
                error: VFS_ERROR_DEVICE_FULL,
            });
        };

        self.files.insert(
            conn_id,
            VfsFileDescriptor {
                resource: VfsResource::from_components(&body.path.components),
                read_dir_page_offset: 0,
                data: Vec::new(),
                position: 0,
                open: false,
                write_failed: false,
            },
        );

        let mut response = response_header(VfsRequestCode::Attach as u16, header, status::OK);
        response.servers_conn_id = conn_id.get();
        VfsResponse::Simple(response)
    }

    fn detach(&mut self, header: &VfsRequestHeader) -> VfsResponse {
        if let Some(file) =
            NonZeroU16::new(header.servers_conn_id).and_then(|conn_id| self.files.remove(&conn_id))
            && file.resource == VfsResource::MailObject
            && !file.write_failed
            && !file.data.is_empty()
        {
            self.finalized_mail = Some(file.data);
        }

        simple_response(VfsRequestCode::Detach as u16, header, status::OK)
    }

    fn close(&mut self, header: &VfsRequestHeader) -> VfsResponse {
        // TODO

        simple_response(VfsRequestCode::Close as u16, header, status::OK)
    }

    /// A request code the parser did not recognize. Answering with an error
    /// keeps a single unknown message from taking the server down: the client
    /// sees a failed request instead of a closed connection.
    fn unknown(&mut self, header: &VfsRequestHeader, body: &[u8]) -> VfsResponse {
        warn!(
            target: "vfs",
            "unsupported request {:#06x} with {} body bytes",
            header.request,
            body.len()
        );

        simple_response(header.request, header, VFS_ERROR_NOT_SUPPORTED)
    }
}

impl VfsFileDescriptor {
    fn read_dir_page(&mut self, max_entries: usize) -> Vec<VfsShortDirEntry> {
        let entries = match self.resource {
            VfsResource::Resources => RESOURCES,
            VfsResource::HardDisk => HARD_DISK,
            VfsResource::HardDiskFiles => HARD_DISK_FILES,
            VfsResource::MailObject | VfsResource::Unknown => return Vec::new(),
        };

        let mut page = Vec::new();

        for name in entries
            .iter()
            .skip(self.read_dir_page_offset)
            .take(max_entries)
        {
            page.push(VfsShortDirEntry {
                name: name.as_bytes().to_vec(),
            });
            self.read_dir_page_offset += 1;
        }

        page
    }
}

impl VfsResource {
    fn from_components(components: &[&bstr::BStr]) -> Self {
        if components.len() == 2
            && components[0] == b"Name Device"
            && components[1] == b"Resources~Subject~"
        {
            Self::Resources
        } else if components.len() == 1 && components[0] == b"Hard Disk" {
            Self::HardDisk
        } else if components.len() == 2
            && components[0] == b"Hard Disk"
            && components[1] == b"Folder 3~Subject~"
        {
            Self::HardDiskFiles
        } else if components.len() >= 3 && components[0] == b"Mail" && components[1] == b"Mail" {
            Self::MailObject
        } else {
            Self::Unknown
        }
    }
}

#[cfg(test)]
mod tests {
    use super::protocol::{Path, VFS_PASSWORD_LEN, VfsAttachMode};
    use super::*;

    fn header(request: VfsRequestCode, server: u16) -> VfsRequestHeader {
        VfsRequestHeader {
            request: request as u16,
            requestors_conn_id: 0x7e00,
            servers_conn_id: server,
        }
    }

    #[test]
    fn writable_mail_object_is_finalized_on_detach() {
        let path =
            Path::try_from_slice(b"`vklachkov server:Mail`Mail`84/08/10 19:01:54.3~Mail~").unwrap();
        let mut vfs = Vfs::new();
        vfs.process_request(VfsRequest {
            header: header(VfsRequestCode::Attach, 0),
            body: VfsRequestBody::Attach(VfsAttachRequest {
                mode: VfsAttachMode::NewFile,
                access: VfsAccessMode::Write,
                password: [0; VFS_PASSWORD_LEN],
                path,
            }),
        });
        vfs.process_request(VfsRequest {
            header: header(VfsRequestCode::Open, 1),
            body: VfsRequestBody::Open(VfsOpenRequest { num_buf: 1 }),
        });
        vfs.process_request(VfsRequest {
            header: header(VfsRequestCode::Write, 1),
            body: VfsRequestBody::Write(VfsWriteRequest { data: b"first " }),
        });
        vfs.process_request(VfsRequest {
            header: header(VfsRequestCode::Write, 1),
            body: VfsRequestBody::Write(VfsWriteRequest { data: b"second" }),
        });

        assert_eq!(vfs.take_finalized_mail(), None);
        vfs.process_request(VfsRequest {
            header: header(VfsRequestCode::Detach, 1),
            body: VfsRequestBody::Detach,
        });
        assert_eq!(vfs.take_finalized_mail(), Some(b"first second".to_vec()));
        assert_eq!(vfs.take_finalized_mail(), None);
    }

    #[test]
    fn write_errors_do_not_commit_partial_mail() {
        let mut vfs = Vfs::new();
        let response = vfs.write(
            &header(VfsRequestCode::Write, 999),
            VfsWriteRequest { data: b"lost" },
        );
        let VfsResponse::Simple(response) = response else {
            panic!("write returned a non-simple response");
        };
        assert_eq!(response.error, VFS_ERROR_BAD_CONNECTION);

        let path =
            Path::try_from_slice(b"`vklachkov server:Mail`Mail`84/08/10 19:01:54.3~Mail~").unwrap();
        vfs.process_request(VfsRequest {
            header: header(VfsRequestCode::Attach, 0),
            body: VfsRequestBody::Attach(VfsAttachRequest {
                mode: VfsAttachMode::NewFile,
                access: VfsAccessMode::Write,
                password: [0; VFS_PASSWORD_LEN],
                path,
            }),
        });
        vfs.process_request(VfsRequest {
            header: header(VfsRequestCode::Open, 1),
            body: VfsRequestBody::Open(VfsOpenRequest { num_buf: 1 }),
        });
        vfs.files
            .get_mut(&NonZeroU16::new(1).unwrap())
            .unwrap()
            .position = MAX_MAIL_OBJECT_SIZE;
        let response = vfs.write(
            &header(VfsRequestCode::Write, 1),
            VfsWriteRequest { data: b"too much" },
        );
        let VfsResponse::Simple(response) = response else {
            panic!("write returned a non-simple response");
        };
        assert_eq!(response.error, VFS_ERROR_DEVICE_FULL);

        vfs.process_request(VfsRequest {
            header: header(VfsRequestCode::Detach, 1),
            body: VfsRequestBody::Detach,
        });
        assert_eq!(vfs.take_finalized_mail(), None);
    }

    #[test]
    fn read_descriptor_initializes_the_property_length_used_by_clients() {
        let mut vfs = Vfs::new();
        let response = vfs.read_desc(
            &header(VfsRequestCode::ReadDesc, 1),
            VfsReadRequest {
                data_length: VFS_DESCRIPTOR_LEN as u16,
            },
        );
        let VfsResponse::Read(response) = response else {
            panic!("read descriptor returned a non-read response");
        };

        assert_eq!(response.data.len(), VFS_DESCRIPTOR_LEN);
        assert_eq!(
            &response.data[VFS_DESCRIPTOR_PROPERTY_LENGTH_OFFSET
                ..VFS_DESCRIPTOR_PROPERTY_LENGTH_OFFSET + size_of::<u32>()],
            &0_u32.to_le_bytes()
        );
    }

    #[test]
    fn absolute_seek_may_position_beyond_end_of_file() {
        let path =
            Path::try_from_slice(b"`vklachkov server:Mail`Mail`84/08/10 19:01:54.3~Mail~").unwrap();
        let mut vfs = Vfs::new();
        vfs.process_request(VfsRequest {
            header: header(VfsRequestCode::Attach, 0),
            body: VfsRequestBody::Attach(VfsAttachRequest {
                mode: VfsAttachMode::NewFile,
                access: VfsAccessMode::Write,
                password: [0; VFS_PASSWORD_LEN],
                path: path.clone(),
            }),
        });
        vfs.process_request(VfsRequest {
            header: header(VfsRequestCode::Open, 1),
            body: VfsRequestBody::Open(VfsOpenRequest { num_buf: 1 }),
        });

        let response = vfs.seek(
            &header(VfsRequestCode::Seek, 1),
            VfsSeekRequest {
                mode: VfsSeekMode::Absolute,
                position: u32::MAX,
            },
        );
        let VfsResponse::Simple(response) = response else {
            panic!("seek returned a non-simple response");
        };
        assert_eq!(response.error, status::OK);
        assert_eq!(
            vfs.files
                .get(&NonZeroU16::new(1).unwrap())
                .unwrap()
                .position,
            u32::MAX as usize
        );
    }

    #[test]
    fn seek_updates_the_next_write_position() {
        let path =
            Path::try_from_slice(b"`vklachkov server:Mail`Mail`84/08/10 19:01:54.3~Mail~").unwrap();
        let mut vfs = Vfs::new();
        vfs.process_request(VfsRequest {
            header: header(VfsRequestCode::Attach, 0),
            body: VfsRequestBody::Attach(VfsAttachRequest {
                mode: VfsAttachMode::NewFile,
                access: VfsAccessMode::Write,
                password: [0; VFS_PASSWORD_LEN],
                path,
            }),
        });
        vfs.process_request(VfsRequest {
            header: header(VfsRequestCode::Open, 1),
            body: VfsRequestBody::Open(VfsOpenRequest { num_buf: 1 }),
        });
        vfs.process_request(VfsRequest {
            header: header(VfsRequestCode::Write, 1),
            body: VfsRequestBody::Write(VfsWriteRequest { data: b"abcdef" }),
        });
        vfs.process_request(VfsRequest {
            header: header(VfsRequestCode::Seek, 1),
            body: VfsRequestBody::Seek(VfsSeekRequest {
                mode: VfsSeekMode::Backward,
                position: 3,
            }),
        });
        vfs.process_request(VfsRequest {
            header: header(VfsRequestCode::Write, 1),
            body: VfsRequestBody::Write(VfsWriteRequest { data: b"XYZ" }),
        });
        vfs.process_request(VfsRequest {
            header: header(VfsRequestCode::Detach, 1),
            body: VfsRequestBody::Detach,
        });

        assert_eq!(vfs.take_finalized_mail(), Some(b"abcXYZ".to_vec()));
    }
}
