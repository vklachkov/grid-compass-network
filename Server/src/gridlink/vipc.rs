use std::io;

use bstr::BStr;

use super::{
    error::FrameError,
    path::Path,
    utils::{CursorExt, ReadExt},
};

#[derive(Clone, Copy, Debug, strum::FromRepr)]
#[repr(u16)]
enum MessageClassType {
    Vfs = 83,
    Mail = 0x7444,
}

#[derive(Clone, Debug)]
pub struct IncomingMessage<'a> {
    pub note: u16,                     // note
    pub body: IncomingMessageBody<'a>, // class + payload
}

#[derive(Clone, Debug)]
pub enum IncomingMessageBody<'a> {
    Vfs(VfsRequest<'a>),
    Mail(&'a [u8]),
    Unsupported(UnknownRequest<'a>),
}

#[derive(Clone, Debug)]
pub struct VfsRequest<'a> {
    pub header: VfsRequestHeader, // VfsRequestCommonPart
    pub body: VfsRequestBody<'a>,
}

#[derive(Clone, Copy, Debug)]
pub struct VfsRequestHeader {
    pub request: u16,            // vfsRequest
    pub requestors_conn_id: u16, // requestorsConnID
    pub servers_conn_id: u16,    // serversConnID
}

#[derive(Clone, Copy, Debug, strum::FromRepr)]
#[repr(u16)]
pub enum VfsRequestCode {
    GetStatus = 1,    // ddGetStatus
    Open = 2,         // ddOpen
    Close = 3,        // ddClose
    Read = 4,         // ddRead
    Write = 5,        // ddWrite
    Seek = 6,         // ddSeek
    Attach = 8,       // ddAttach
    Detach = 9,       // ddDetach
    ReadDesc = 12,    // ddReadDesc
    WriteDesc = 13,   // ddWriteDesc
    SetStatus = 20,   // ddSetStatus
    ReadDirPage = 29, // ddReadDirPage
}

#[derive(Clone, Debug)]
pub enum VfsRequestBody<'a> {
    // ReadReqType
    GetStatus(VfsReadRequest),

    // OpenReqType
    Open(VfsOpenRequest),

    // ReadReqType
    Read(VfsReadRequest),

    // ReadReqType
    ReadDesc(VfsReadRequest),

    // ReadReqType
    ReadDirPage(VfsReadRequest),

    // WriteReqType
    Write(VfsWriteRequest<'a>),

    // WriteReqType
    WriteDesc(VfsWriteRequest<'a>),

    // WriteReqType
    SetStatus(VfsSetStatusRequest<'a>),

    // SeekReqType
    Seek(VfsSeekRequest),

    // AttachReqType
    Attach(VfsAttachRequest<'a>),

    Detach,

    Close,

    // raw
    Unknown(&'a [u8]),
}

#[derive(Clone, Debug)]
pub struct VfsAttachRequest<'a> {
    pub mode: u8,           // mode
    pub access: u8,         // access
    pub password: [u8; 17], // password
    pub path: Path<'a>,
}

#[derive(Clone, Copy, Debug)]
pub struct VfsOpenRequest {
    pub num_buf: u8, // numBuf
}

#[derive(Clone, Copy, Debug)]
pub struct VfsReadRequest {
    pub data_length: u16, // vfsDatalength
}

#[derive(Clone, Copy, Debug)]
pub struct VfsSeekRequest {
    pub mode: u8,      // mode
    pub position: u32, // position
}

#[derive(Clone, Debug)]
pub struct VfsWriteRequest<'a> {
    pub data: &'a [u8], // buffer
}

#[derive(Clone, Debug)]
pub struct VfsSetStatusRequest<'a> {
    pub actions: Vec<VfsSetStatusAction<'a>>,
}

#[derive(Clone, Copy, Debug, strum::FromRepr)]
#[repr(u8)]
pub enum VfsSetStatusType {
    SetDirection = 255,
    SetWildcard = 254,
    SetObjectMode = 253,
    SetGpibAddress = 252,
    SetDeviceMask = 251,
    SetNoteValue = 250,
    SetNumBuffers = 249,
    SetNameAttributes = 248,
    SetConsoleMode = 247,
    SetAccessRights = 246,
    SetSecureMode = 245,
    SetIpcActivityTimeout = 244,
    GetGenericDeviceName = 243,
    GetDeviceName = 242,
}

#[derive(Clone, Debug)]
pub enum VfsSetStatusAction<'a> {
    SetDirection {
        raw: &'a [u8],
    },
    SetWildcard {
        pattern: &'a BStr,
    },
    SetObjectMode {
        mode: VfsObjectMode,
    },
    SetGpibAddress {
        raw: &'a [u8],
    },
    SetDeviceMask {
        raw: &'a [u8],
    },
    SetNoteValue {
        raw: &'a [u8],
    },
    SetNumBuffers {
        raw: &'a [u8],
    },
    SetNameAttributes {
        raw: &'a [u8],
    },
    SetConsoleMode {
        raw: &'a [u8],
    },
    SetAccessRights {
        raw: &'a [u8],
    },
    SetSecureMode {
        raw: &'a [u8],
    },
    SetIpcActivityTimeout {
        raw: &'a [u8],
    },
    GetGenericDeviceName {
        raw: &'a [u8],
    },
    GetDeviceName {
        raw: &'a [u8],
    },
    Unknown {
        ty: u8,
        raw: &'a [u8],
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, strum::FromRepr)]
#[repr(u8)]
pub enum VfsObjectMode {
    Byte = 0,
    Directory = 1,
    CompleteDirectory = 2,
}

#[derive(Clone, Copy, Debug)]
pub struct UnknownRequest<'a> {
    pub class: u16,
    pub payload: &'a [u8],
}

#[derive(Clone, Debug)]
pub struct OutgoingMessage {
    pub note: u16,                 // note
    pub body: OutgoingMessageBody, // class + payload
}

#[derive(Clone, Debug)]
pub enum OutgoingMessageBody {
    Mail {
        payload: Vec<u8>,
    },

    // SimpleRespType
    VfsSimple(VfsResponseHeader),

    // StatusType
    VfsGetStatus(VfsGetStatusResponse),

    // ReadRespType with DirectoryEntryType buffer
    VfsReadDirPage(VfsReadDirPageResponse),

    // ReadRespType
    VfsRead(VfsReadResponse),
}

#[derive(Clone, Copy, Debug)]
pub struct VfsResponseHeader {
    pub response: u16,           // vfsResponse
    pub servers_conn_id: u16,    // serversConnID
    pub requestors_conn_id: u16, // requestorsConnID
    pub error: u16,              // Error
}

#[derive(Clone, Debug)]
pub struct VfsGetStatusResponse {
    pub header: VfsResponseHeader, // VfsRespCommonPart
    pub open: bool,                // open
    pub access: VfsAccessMode,     // access
    pub seek: bool,                // seek
    pub file_position: u32,        // filePosition
    pub file_length: u32,          // fileLength
    pub num_pages: u16,            // numPages
    pub num_pages_alloc: u16,      // numPagesAlloc
}

#[derive(Clone, Debug)]
pub struct VfsReadDirPageResponse {
    pub header: VfsResponseHeader,      // VfsRespCommonPart
    pub entries: Vec<VfsShortDirEntry>, // vfsDatalength + DirectoryEntryType buffer
}

#[derive(Clone, Debug)]
pub struct VfsReadResponse {
    pub header: VfsResponseHeader, // VfsRespCommonPart
    pub data: Vec<u8>,             // vfsDatalength + buffer
}

#[derive(Clone, Debug)]
pub struct VfsShortDirEntry {
    pub name: Vec<u8>, // DirectoryEntryType.name
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum VfsAccessMode {
    Read = 1,
    Write = 2,
    Update = 3,
    UpdateDescriptor = 4,
    ShortDirectory = 5,
    LongDirectory = 6,
}

impl<'a> IncomingMessage<'a> {
    pub fn try_from_slice(data: &'a [u8]) -> Result<Self, FrameError> {
        let mut cursor = io::Cursor::new(data);

        let class = cursor.read_u16()?;
        let note = cursor.read_u16()?;
        let data_length = cursor.read_u16()? as usize;
        let payload = cursor.read_slice(data_length)?;

        Self::ensure_empty(&cursor, "VIPC message")?;

        let body = match MessageClassType::from_repr(class) {
            Some(MessageClassType::Vfs) => {
                IncomingMessageBody::Vfs(Self::read_vfs_request(payload)?)
            }
            Some(MessageClassType::Mail) => IncomingMessageBody::Mail(payload),
            None => {
                IncomingMessageBody::Unsupported(UnknownRequest { class, payload }) //
            }
        };

        Ok(Self { note, body })
    }

    fn read_vfs_request(data: &'a [u8]) -> Result<VfsRequest<'a>, FrameError> {
        let mut cursor = io::Cursor::new(data);

        let header = Self::read_vfs_request_header(&mut cursor)?;

        let body = match VfsRequestCode::from_repr(header.request) {
            Some(VfsRequestCode::Attach) => {
                Self::read_vfs_attach_request(&mut cursor)? //
            }
            Some(VfsRequestCode::Open) => {
                Self::read_vfs_open_request(&mut cursor)? //
            }
            Some(VfsRequestCode::GetStatus) => {
                Self::read_vfs_read_request(&mut cursor).map(VfsRequestBody::GetStatus)?
            }
            Some(VfsRequestCode::Read) => {
                Self::read_vfs_read_request(&mut cursor).map(VfsRequestBody::Read)?
            }
            Some(VfsRequestCode::ReadDesc) => {
                Self::read_vfs_read_request(&mut cursor).map(VfsRequestBody::ReadDesc)?
            }
            Some(VfsRequestCode::ReadDirPage) => {
                Self::read_vfs_read_request(&mut cursor).map(VfsRequestBody::ReadDirPage)?
            }
            Some(VfsRequestCode::Seek) => {
                Self::read_vfs_seek_request(&mut cursor)? //
            }
            Some(VfsRequestCode::Write) => {
                Self::read_vfs_write_request(&mut cursor).map(VfsRequestBody::Write)?
            }
            Some(VfsRequestCode::WriteDesc) => {
                Self::read_vfs_write_request(&mut cursor).map(VfsRequestBody::WriteDesc)?
            }
            Some(VfsRequestCode::SetStatus) => {
                Self::read_vfs_set_status_request(&mut cursor)? //
            }
            Some(VfsRequestCode::Detach) => {
                Self::read_detach_vfs_request(&cursor)? //
            }
            Some(VfsRequestCode::Close) => {
                Self::read_close_vfs_request(&cursor)? //
            }
            None => {
                VfsRequestBody::Unknown(cursor.read_remainder()) //
            }
        };

        Ok(VfsRequest { header, body })
    }

    fn read_vfs_request_header(
        cursor: &mut io::Cursor<&[u8]>,
    ) -> Result<VfsRequestHeader, FrameError> {
        Ok(VfsRequestHeader {
            request: cursor.read_u16()?,
            requestors_conn_id: cursor.read_u16()?,
            servers_conn_id: cursor.read_u16()?,
        })
    }

    fn read_vfs_attach_request(
        cursor: &mut io::Cursor<&'a [u8]>,
    ) -> Result<VfsRequestBody<'a>, FrameError> {
        let mode = cursor.read_u8()?;
        let access = cursor.read_u8()?;
        let password = cursor.read_array()?;
        let path = Self::read_small_slice(cursor).and_then(Path::try_from_slice)?;

        Self::ensure_empty(cursor, "VFS attach payload")?;

        Ok(VfsRequestBody::Attach(VfsAttachRequest {
            mode,
            access,
            password,
            path,
        }))
    }

    fn read_vfs_open_request(
        cursor: &mut io::Cursor<&[u8]>,
    ) -> Result<VfsRequestBody<'a>, FrameError> {
        let num_buf = cursor.read_u8()?;

        Self::ensure_empty(cursor, "VFS open payload")?;

        Ok(VfsRequestBody::Open(VfsOpenRequest { num_buf }))
    }

    fn read_vfs_read_request(cursor: &mut io::Cursor<&[u8]>) -> Result<VfsReadRequest, FrameError> {
        let data_length = cursor.read_u16()?;

        Self::ensure_empty(cursor, "VFS read payload")?;

        Ok(VfsReadRequest { data_length })
    }

    fn read_vfs_seek_request(
        cursor: &mut io::Cursor<&[u8]>,
    ) -> Result<VfsRequestBody<'a>, FrameError> {
        let mode = cursor.read_u8()?;
        let position = cursor.read_u32()?;

        Self::ensure_empty(cursor, "VFS seek payload")?;

        Ok(VfsRequestBody::Seek(VfsSeekRequest { mode, position }))
    }

    fn read_vfs_write_request(
        cursor: &mut io::Cursor<&'a [u8]>,
    ) -> Result<VfsWriteRequest<'a>, FrameError> {
        let data_length = cursor.read_u16()? as usize;
        let data = cursor.read_slice(data_length)?;

        Self::ensure_empty(cursor, "VFS write payload")?;

        Ok(VfsWriteRequest { data })
    }

    fn read_vfs_set_status_request(
        cursor: &mut io::Cursor<&'a [u8]>,
    ) -> Result<VfsRequestBody<'a>, FrameError> {
        let data_length = cursor.read_u16()? as usize;
        let data = cursor.read_slice(data_length)?;

        Self::ensure_empty(cursor, "VFS set status payload")?;

        let mut data_cursor = io::Cursor::new(data);
        let mut actions = Vec::new();

        while data_cursor.position() < data.len() as u64 {
            let ty = data_cursor.read_u8()?;
            let length = data_cursor.read_u16()? as usize;
            let raw = data_cursor.read_slice(length)?;

            actions.push(VfsSetStatusAction::from_raw(ty, raw)?);
        }

        Self::ensure_empty(&data_cursor, "VFS set status data")?;

        Ok(VfsRequestBody::SetStatus(VfsSetStatusRequest { actions }))
    }

    fn read_detach_vfs_request(
        cursor: &io::Cursor<&[u8]>,
    ) -> Result<VfsRequestBody<'a>, FrameError> {
        Self::ensure_empty(cursor, "VFS detach payload")?;
        Ok(VfsRequestBody::Detach)
    }

    fn read_close_vfs_request(
        cursor: &io::Cursor<&[u8]>,
    ) -> Result<VfsRequestBody<'a>, FrameError> {
        Self::ensure_empty(cursor, "VFS close payload")?;
        Ok(VfsRequestBody::Close)
    }

    fn read_small_slice(cursor: &mut io::Cursor<&'a [u8]>) -> Result<&'a [u8], FrameError> {
        let length = cursor.read_u8()?;
        cursor.read_slice(length as usize).map_err(Into::into)
    }

    fn ensure_empty(cursor: &io::Cursor<&[u8]>, context: &str) -> Result<(), FrameError> {
        let remaining = cursor
            .get_ref()
            .len()
            .saturating_sub(cursor.position() as usize);

        if remaining == 0 {
            return Ok(());
        }

        Err(FrameError::Validation {
            reason: format!("{context}: {remaining} trailing bytes"),
        })
    }
}

impl OutgoingMessage {
    pub fn to_bytes(&self) -> Vec<u8> {
        let payload_length = self.payload_length();

        let mut data = Vec::with_capacity(6 + payload_length as usize);

        data.extend(self.class().to_le_bytes());
        data.extend(self.note.to_le_bytes());
        data.extend(payload_length.to_le_bytes());

        match &self.body {
            OutgoingMessageBody::Mail { payload, .. } => data.extend_from_slice(payload),
            OutgoingMessageBody::VfsSimple(response) => {
                Self::write_header(&mut data, response);
            }
            OutgoingMessageBody::VfsGetStatus(response) => {
                Self::write_header(&mut data, &response.header);
                Self::write_status(&mut data, response);
            }
            OutgoingMessageBody::VfsReadDirPage(response) => {
                Self::write_header(&mut data, &response.header);
                Self::write_short_dir_entries(&mut data, &response.entries);
            }
            OutgoingMessageBody::VfsRead(response) => {
                Self::write_header(&mut data, &response.header);
                data.extend((response.data.len() as u16).to_le_bytes());
                data.extend_from_slice(&response.data);
            }
        }

        data
    }

    fn class(&self) -> u16 {
        match &self.body {
            OutgoingMessageBody::Mail { .. } => MessageClassType::Mail as u16,
            OutgoingMessageBody::VfsSimple(_) => MessageClassType::Vfs as u16,
            OutgoingMessageBody::VfsGetStatus(_) => MessageClassType::Vfs as u16,
            OutgoingMessageBody::VfsReadDirPage(_) => MessageClassType::Vfs as u16,
            OutgoingMessageBody::VfsRead(_) => MessageClassType::Vfs as u16,
        }
    }

    fn payload_length(&self) -> u16 {
        // TODO: Remove magic numbers.
        match &self.body {
            OutgoingMessageBody::Mail { payload, .. } => {
                u16::try_from(payload.len()).expect("VIPC payload exceeds u16 length")
            }
            OutgoingMessageBody::VfsSimple(_) => 8,
            OutgoingMessageBody::VfsGetStatus(_) => 25,
            OutgoingMessageBody::VfsReadDirPage(response) => {
                10 + Self::short_dir_entries_len(&response.entries)
            }
            OutgoingMessageBody::VfsRead(response) => 10 + response.data.len() as u16,
        }
    }

    fn write_header(dst: &mut Vec<u8>, header: &VfsResponseHeader) {
        dst.extend(header.response.to_le_bytes());
        dst.extend(header.servers_conn_id.to_le_bytes());
        dst.extend(header.requestors_conn_id.to_le_bytes());
        dst.extend(header.error.to_le_bytes());
    }

    fn write_status(dst: &mut Vec<u8>, response: &VfsGetStatusResponse) {
        dst.extend(15u16.to_le_bytes()); // vfsDataLength
        dst.push(response.open as u8);
        dst.push(response.access as u8);
        dst.push(response.seek as u8);
        dst.extend(response.file_position.to_le_bytes());
        dst.extend(response.file_length.to_le_bytes());
        dst.extend(response.num_pages.to_le_bytes());
        dst.extend(response.num_pages_alloc.to_le_bytes());
    }

    fn write_short_dir_entries(dst: &mut Vec<u8>, entries: &[VfsShortDirEntry]) {
        dst.extend(Self::short_dir_entries_len(entries).to_le_bytes());

        for entry in entries {
            Self::write_short_dir_entry(dst, entry);
        }
    }

    fn short_dir_entries_len(entries: &[VfsShortDirEntry]) -> u16 {
        entries
            .iter()
            .map(|entry| 9 + entry.name.len() as u16)
            .sum()
    }

    fn write_short_dir_entry(dst: &mut Vec<u8>, entry: &VfsShortDirEntry) {
        dst.extend([0; 4]);
        dst.extend((9 + entry.name.len() as u32).to_le_bytes());
        dst.push(entry.name.len() as u8);
        dst.extend_from_slice(&entry.name);
    }
}

impl<'a> VfsSetStatusAction<'a> {
    fn from_raw(ty: u8, raw: &'a [u8]) -> Result<Self, FrameError> {
        let Some(ty) = VfsSetStatusType::from_repr(ty) else {
            return Ok(Self::Unknown { ty, raw });
        };

        let action = match ty {
            VfsSetStatusType::SetDirection => Self::SetDirection { raw },
            VfsSetStatusType::SetWildcard => Self::SetWildcard {
                pattern: BStr::new(raw),
            },
            VfsSetStatusType::SetObjectMode => Self::parse_object_mode(raw)?,
            VfsSetStatusType::SetGpibAddress => Self::SetGpibAddress { raw },
            VfsSetStatusType::SetDeviceMask => Self::SetDeviceMask { raw },
            VfsSetStatusType::SetNoteValue => Self::SetNoteValue { raw },
            VfsSetStatusType::SetNumBuffers => Self::SetNumBuffers { raw },
            VfsSetStatusType::SetNameAttributes => Self::SetNameAttributes { raw },
            VfsSetStatusType::SetConsoleMode => Self::SetConsoleMode { raw },
            VfsSetStatusType::SetAccessRights => Self::SetAccessRights { raw },
            VfsSetStatusType::SetSecureMode => Self::SetSecureMode { raw },
            VfsSetStatusType::SetIpcActivityTimeout => Self::SetIpcActivityTimeout { raw },
            VfsSetStatusType::GetGenericDeviceName => Self::GetGenericDeviceName { raw },
            VfsSetStatusType::GetDeviceName => Self::GetDeviceName { raw },
        };

        Ok(action)
    }

    fn parse_object_mode(raw: &[u8]) -> Result<Self, FrameError> {
        if raw.len() != 1 {
            return Err(FrameError::Validation {
                reason: format!(
                    "invalid VFS set object mode length: expected 1, found {}",
                    raw.len()
                ),
            });
        }

        let raw_mode = raw[0];
        let Some(mode) = VfsObjectMode::from_repr(raw_mode) else {
            return Err(FrameError::Validation {
                reason: format!("invalid VFS object mode: {raw_mode}"),
            });
        };

        Ok(Self::SetObjectMode { mode })
    }
}
