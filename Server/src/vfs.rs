use std::{collections::HashMap, num::NonZeroU16};

use crate::gridlink::vipc::*;

const RESOURCES: &[&str] = &["Hard Disk~FS~"];
const HARD_DISK: &[&str] = &[
    "Folder 1~Subject~",
    "Folder 3~Subject~",
    "Folder 2~Subject~",
];
const HARD_DISK_FILES: &[&str] = &["Demo file~Text~"];

const MAX_MAIL_OBJECT_SIZE: usize = 16 * 1024 * 1024;
const VFS_OK: u16 = 0;
const VFS_ERROR_FILE_NOT_OPEN: u16 = 205;
const VFS_ERROR_BAD_CONNECTION: u16 = 221;
const VFS_ERROR_BAD_PARAMETER: u16 = 225;
const VFS_ERROR_DEVICE_FULL: u16 = 41;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VfsResource {
    Resources,
    HardDisk,
    HardDiskFiles,
    MailObject,
    Unknown,
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

    pub fn process_request(&mut self, req: VfsRequest) -> OutgoingMessageBody {
        match req.body {
            VfsRequestBody::GetStatus(body) => self.get_status(&req.header, body),
            VfsRequestBody::Open(body) => self.open(&req.header, body),
            VfsRequestBody::Read(body) => self.read(&req.header, body),
            VfsRequestBody::ReadDesc(body) => self.read_desc(&req.header, body),
            VfsRequestBody::ReadDirPage(body) => self.read_dir_page(&req.header, body),
            VfsRequestBody::Write(body) => self.write(&req.header, body),
            VfsRequestBody::WriteDesc(body) => self.write_desc(&req.header, body),
            VfsRequestBody::SetStatus(body) => self.set_status(&req.header, body),
            VfsRequestBody::Seek(body) => self.seek(&req.header, body),
            VfsRequestBody::Attach(body) => self.attach(&req.header, body),
            VfsRequestBody::Detach => self.detach(&req.header),
            VfsRequestBody::Close => self.close(&req.header),
            VfsRequestBody::Unknown(body) => self.unknown(body),
        }
    }

    fn get_status(
        &mut self,
        header: &VfsRequestHeader,
        _body: VfsReadRequest,
    ) -> OutgoingMessageBody {
        OutgoingMessageBody::VfsGetStatus(VfsGetStatusResponse {
            header: VfsResponseHeader {
                response: 0x8000 | VfsRequestCode::GetStatus as u16,
                servers_conn_id: header.servers_conn_id,
                requestors_conn_id: header.requestors_conn_id,
                error: 0, // Ok
            },
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

    fn open(&mut self, header: &VfsRequestHeader, _body: VfsOpenRequest) -> OutgoingMessageBody {
        if let Some(file) =
            NonZeroU16::new(header.servers_conn_id).and_then(|conn_id| self.files.get_mut(&conn_id))
        {
            file.open = true;
        }

        OutgoingMessageBody::VfsSimple(VfsResponseHeader {
            response: 0x8000 | VfsRequestCode::Open as u16,
            servers_conn_id: header.servers_conn_id,
            requestors_conn_id: header.requestors_conn_id,
            error: 0, // Ok
        })
    }

    fn read(&mut self, header: &VfsRequestHeader, _body: VfsReadRequest) -> OutgoingMessageBody {
        // TODO

        OutgoingMessageBody::VfsRead(VfsReadResponse {
            header: VfsResponseHeader {
                response: 0x8000 | VfsRequestCode::Read as u16,
                servers_conn_id: header.servers_conn_id,
                requestors_conn_id: header.requestors_conn_id,
                error: 0, // Ok
            },
            data: b"Read stub".to_vec(),
        })
    }

    fn read_desc(
        &mut self,
        header: &VfsRequestHeader,
        _body: VfsReadRequest,
    ) -> OutgoingMessageBody {
        // TODO

        OutgoingMessageBody::VfsRead(VfsReadResponse {
            header: VfsResponseHeader {
                response: 0x8000 | VfsRequestCode::ReadDesc as u16,
                servers_conn_id: header.servers_conn_id,
                requestors_conn_id: header.requestors_conn_id,
                error: 0, // Ok
            },
            data: Vec::new(),
        })
    }

    fn read_dir_page(
        &mut self,
        header: &VfsRequestHeader,
        body: VfsReadRequest,
    ) -> OutgoingMessageBody {
        let entries = NonZeroU16::new(header.servers_conn_id)
            .and_then(|conn_id| self.files.get_mut(&conn_id))
            .map(|file| file.read_dir_page(body.data_length as usize))
            .unwrap_or_default();

        OutgoingMessageBody::VfsReadDirPage(VfsReadDirPageResponse {
            header: VfsResponseHeader {
                response: 0x8000 | VfsRequestCode::ReadDirPage as u16,
                servers_conn_id: header.servers_conn_id,
                requestors_conn_id: header.requestors_conn_id,
                error: 0, // Ok
            },
            entries,
        })
    }

    fn write(
        &mut self,
        header: &VfsRequestHeader,
        body: VfsWriteRequest<'_>,
    ) -> OutgoingMessageBody {
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
                    VFS_OK
                }
                _ => {
                    file.write_failed = true;
                    VFS_ERROR_DEVICE_FULL
                }
            },
        };

        OutgoingMessageBody::VfsSimple(VfsResponseHeader {
            response: 0x8000 | VfsRequestCode::Write as u16,
            servers_conn_id: header.servers_conn_id,
            requestors_conn_id: header.requestors_conn_id,
            error,
        })
    }

    fn write_desc(
        &mut self,
        header: &VfsRequestHeader,
        _body: VfsWriteRequest<'_>,
    ) -> OutgoingMessageBody {
        // TODO

        OutgoingMessageBody::VfsSimple(VfsResponseHeader {
            response: 0x8000 | VfsRequestCode::WriteDesc as u16,
            servers_conn_id: header.servers_conn_id,
            requestors_conn_id: header.requestors_conn_id,
            error: 0, // Ok
        })
    }

    fn set_status(
        &mut self,
        header: &VfsRequestHeader,
        _body: VfsSetStatusRequest<'_>,
    ) -> OutgoingMessageBody {
        // TODO

        OutgoingMessageBody::VfsSimple(VfsResponseHeader {
            response: 0x8000 | VfsRequestCode::SetStatus as u16,
            servers_conn_id: header.servers_conn_id,
            requestors_conn_id: header.requestors_conn_id,
            error: 0, // Ok
        })
    }

    fn seek(&mut self, header: &VfsRequestHeader, body: VfsSeekRequest) -> OutgoingMessageBody {
        let error = match NonZeroU16::new(header.servers_conn_id)
            .and_then(|conn_id| self.files.get_mut(&conn_id))
        {
            None => VFS_ERROR_BAD_CONNECTION,
            Some(file) if !file.open => VFS_ERROR_FILE_NOT_OPEN,
            Some(file) => {
                let offset = usize::try_from(body.position).ok();
                let position = match body.mode {
                    1 => offset.and_then(|offset| file.position.checked_sub(offset)),
                    2 => offset,
                    3 => offset.and_then(|offset| file.position.checked_add(offset)),
                    4 => offset.and_then(|offset| file.data.len().checked_sub(offset)),
                    _ => None,
                };

                match position.filter(|position| *position <= MAX_MAIL_OBJECT_SIZE) {
                    Some(position) => {
                        file.position = position;
                        VFS_OK
                    }
                    None => VFS_ERROR_BAD_PARAMETER,
                }
            }
        };

        OutgoingMessageBody::VfsSimple(VfsResponseHeader {
            response: 0x8000 | VfsRequestCode::Seek as u16,
            servers_conn_id: header.servers_conn_id,
            requestors_conn_id: header.requestors_conn_id,
            error,
        })
    }

    fn attach(
        &mut self,
        header: &VfsRequestHeader,
        body: VfsAttachRequest<'_>,
    ) -> OutgoingMessageBody {
        let conn_id = self.connection_id;

        if self.files.contains_key(&conn_id) {
            unimplemented!();
        }

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

        // no wrapping_add() for NonZero types.
        self.connection_id = conn_id.checked_add(1).unwrap_or(NonZeroU16::MIN);

        OutgoingMessageBody::VfsSimple(VfsResponseHeader {
            response: 0x8000 | VfsRequestCode::Attach as u16,
            servers_conn_id: conn_id.get(),
            requestors_conn_id: header.requestors_conn_id,
            error: 0, // Ok
        })
    }

    fn detach(&mut self, header: &VfsRequestHeader) -> OutgoingMessageBody {
        if let Some(file) =
            NonZeroU16::new(header.servers_conn_id).and_then(|conn_id| self.files.remove(&conn_id))
            && file.resource == VfsResource::MailObject
            && !file.write_failed
            && !file.data.is_empty()
        {
            self.finalized_mail = Some(file.data);
        }

        OutgoingMessageBody::VfsSimple(VfsResponseHeader {
            response: 0x8000 | VfsRequestCode::Detach as u16,
            servers_conn_id: header.servers_conn_id,
            requestors_conn_id: header.requestors_conn_id,
            error: 0, // Ok
        })
    }

    fn close(&mut self, header: &VfsRequestHeader) -> OutgoingMessageBody {
        // TODO

        OutgoingMessageBody::VfsSimple(VfsResponseHeader {
            response: 0x8000 | VfsRequestCode::Close as u16,
            servers_conn_id: header.servers_conn_id,
            requestors_conn_id: header.requestors_conn_id,
            error: 0, // Ok
        })
    }

    fn unknown(&mut self, _body: &[u8]) -> OutgoingMessageBody {
        panic!("unsupported VFS request")
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
    use super::*;
    use crate::gridlink::Path;

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
                mode: 4,
                access: VfsAccessMode::Write as u8,
                password: [0; 17],
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
        let OutgoingMessageBody::VfsSimple(response) = response else {
            panic!("write returned a non-simple response");
        };
        assert_eq!(response.error, VFS_ERROR_BAD_CONNECTION);

        let path =
            Path::try_from_slice(b"`vklachkov server:Mail`Mail`84/08/10 19:01:54.3~Mail~").unwrap();
        vfs.process_request(VfsRequest {
            header: header(VfsRequestCode::Attach, 0),
            body: VfsRequestBody::Attach(VfsAttachRequest {
                mode: 4,
                access: VfsAccessMode::Write as u8,
                password: [0; 17],
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
        let OutgoingMessageBody::VfsSimple(response) = response else {
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
    fn seek_updates_the_next_write_position() {
        let path =
            Path::try_from_slice(b"`vklachkov server:Mail`Mail`84/08/10 19:01:54.3~Mail~").unwrap();
        let mut vfs = Vfs::new();
        vfs.process_request(VfsRequest {
            header: header(VfsRequestCode::Attach, 0),
            body: VfsRequestBody::Attach(VfsAttachRequest {
                mode: 4,
                access: VfsAccessMode::Write as u8,
                password: [0; 17],
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
                mode: 1,
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
