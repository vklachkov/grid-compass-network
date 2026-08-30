use std::{io, io::Write, mem::size_of};

use bstr::BStr;
use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::{FromPrimitive, ToPrimitive};

use crate::{
    shared::{
        FrameError,
        io::{CursorExt, ReadExt, WriteExt, read_small_slice, u8_len, with_u16_len},
    },
    vfs::{
        AccessMode, AttachMode, DIRECTORY_ENTRY_PREAMBLE_LEN, GRiDPath, ObjectMode, ReadDirection,
        SeekMode,
    },
};

pub(super) const VFS_RESPONSE_BIT: u16 = 0x8000;
pub(super) const VFS_MAX_MESSAGE_LENGTH: usize = 514;
pub(super) const VFS_PAGE_SIZE: usize = 504;
pub(super) const VFS_MAX_DIRECTORY_OBJECTS_PER_PAGE: usize = 30;
pub(super) const VFS_MAX_FILE_NAME_LENGTH: usize = 80;
pub(super) const VFS_MAX_PASSWORD_LENGTH: usize = 16;
pub(super) const VFS_PASSWORD_SPACE: usize = VFS_MAX_PASSWORD_LENGTH + 1;
pub(super) const VFS_MAX_DESCRIPTOR_LENGTH: usize = 200;
pub(super) const VFS_DESCRIPTOR_LENGTH: usize = VFS_MAX_DESCRIPTOR_LENGTH - size_of::<u16>();
pub(super) const VFS_MAX_READ_LENGTH: usize = VFS_PAGE_SIZE;
pub(super) const VFS_MAX_WRITE_LENGTH: usize = VFS_PAGE_SIZE;

pub(super) const VFS_ERROR_NOT_SUPPORTED: u16 = 35; // eNotSupport
pub(super) const VFS_ERROR_FILE_EXISTS: u16 = 32; // eFileExists
pub(super) const VFS_ERROR_DEVICE_FULL: u16 = 41; // eDeviceFull
pub(super) const VFS_ERROR_FILE_NOT_OPEN: u16 = 205; // eFileNotOpen
pub(super) const VFS_ERROR_BAD_CONNECTION: u16 = 221; // eBadConn
pub(super) const VFS_ERROR_ALREADY_OPEN: u16 = 222; // eOpen
pub(super) const VFS_ERROR_BAD_PARAMETER: u16 = 225; // eParam
pub(super) const VFS_ERROR_RESOURCE_UNAVAILABLE: u16 = 601; // eGCRscUnav

#[derive(Clone, Debug)]
pub struct VfsRequest<'a> {
    pub header: VfsRequestHeader,
    pub body: VfsRequestBody<'a>,
}

#[derive(Clone, Copy, Debug)]
pub struct VfsRequestHeader {
    pub request: u16,
    pub requestors_conn_id: u16,
    pub servers_conn_id: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, ToPrimitive)]
#[repr(u16)]
pub enum VfsRequestCode {
    Initialize = 0,
    GetStatus = 1,
    Open = 2,
    Close = 3,
    Read = 4,
    Write = 5,
    Seek = 6,
    Truncate = 7,
    Attach = 8,
    Detach = 9,
    Rename = 10,
    Delete = 11,
    ReadDesc = 12,
    WriteDesc = 13,
    Flush = 14,
    WaitSrq = 15,
    SelfTest = 16,
    Format = 17,
    SetStatus = 20,
    Deactivate = 21,
    TrackFormat = 22,
    ControllerTest = 23,
    RamTest = 24,
    DriveTest = 25,
    Program = 26,
    WriteProtect = 27,
    BufferCommand = 28,
    ReadDirPage = 29,
    SignOn = 30,
    SignOff = 31,
    Send = 32,
    RemoteCopy = 33,
    GetMoreStatus = 34,
    ReadRamBuffer = 35,
    WriteRamBuffer = 36,
}

#[derive(Clone, Debug)]
pub enum VfsRequestBody<'a> {
    GetStatus(VfsReadRequest),
    Open(VfsOpenRequest),
    Read(VfsReadRequest),
    ReadDesc(VfsReadRequest),
    ReadDirPage(VfsReadRequest),
    Write(VfsWriteRequest<'a>),
    WriteDesc(VfsWriteRequest<'a>),
    SetStatus(VfsSetStatusRequest<'a>),
    Seek(VfsSeekRequest),
    Attach(VfsAttachRequest<'a>),
    Detach,
    Close,
    Flush,
    Unknown(&'a [u8]),
}

/// Only the path steers the server so far; the rest of the request is decoded
/// because it reaches the operator through the `Debug` line the dispatcher logs.
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct VfsAttachRequest<'a> {
    pub mode: VfsAttachMode,
    pub access: VfsAccessMode,
    pub password: [u8; VFS_PASSWORD_SPACE],
    pub path: &'a GRiDPath,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, ToPrimitive)]
#[repr(u8)]
pub enum VfsAttachMode {
    OldFile = 1,
    UpdateFile = 2,
    NewFile = 3,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub struct VfsOpenRequest {
    pub num_buf: u8,
}

#[derive(Clone, Copy, Debug)]
pub struct VfsReadRequest {
    pub data_length: u16,
}

impl VfsReadRequest {
    pub fn bounded_length(self) -> usize {
        usize::from(self.data_length).min(VFS_MAX_READ_LENGTH)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct VfsSeekRequest {
    pub mode: VfsSeekMode,
    pub position: u32,
}

#[derive(Clone, Debug)]
pub struct VfsWriteRequest<'a> {
    pub data: &'a [u8],
}

/// Set-status is answered with a bare acknowledgement, so the decoded actions
/// only ever reach the log; they are kept because the shape of the request is
/// what the reverse engineering established.
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct VfsSetStatusRequest<'a> {
    pub actions: Vec<VfsSetStatusAction<'a>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, ToPrimitive)]
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

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub enum VfsSetStatusAction<'a> {
    SetDirection {
        direction: VfsReadDirection,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, ToPrimitive)]
#[repr(u8)]
pub enum VfsObjectMode {
    Byte = 0,
    Directory = 1,
    CompleteDirectory = 2,
}

#[derive(Clone, Copy, Debug)]
pub struct VfsResponseHeader {
    pub response: u16,
    pub servers_conn_id: u16,
    pub requestors_conn_id: u16,
    pub status: u16,
}

#[derive(Clone, Debug)]
pub struct VfsGetStatusResponse {
    pub header: VfsResponseHeader,
    pub open: bool,
    pub access: VfsAccessMode,
    pub seek: bool,
    pub file_position: u32,
    pub file_length: u32,
    pub num_pages: u16,
    pub num_pages_alloc: u16,
}

#[derive(Clone, Debug)]
pub struct VfsReadDirPageResponse {
    pub header: VfsResponseHeader,
    pub entries: Vec<VfsShortDirEntry>,
}

#[derive(Clone, Debug)]
pub struct VfsReadResponse {
    pub header: VfsResponseHeader,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct VfsShortDirEntry {
    pub name: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, ToPrimitive)]
#[repr(u8)]
pub enum VfsSeekMode {
    Backward = 1,
    Absolute = 2,
    Forward = 3,
    FromEnd = 4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, ToPrimitive)]
#[repr(u8)]
pub enum VfsReadDirection {
    Forward = 0,
    Backward = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, ToPrimitive)]
#[repr(u8)]
pub enum VfsAccessMode {
    Read = 1,
    Write = 2,
    Update = 3,
    UpdateDescriptor = 4,
    ShortDirectory = 5,
    LongDirectory = 6,
}

impl From<VfsAttachMode> for AttachMode {
    fn from(value: VfsAttachMode) -> Self {
        match value {
            VfsAttachMode::OldFile => Self::OldFile,
            VfsAttachMode::UpdateFile => Self::UpdateFile,
            VfsAttachMode::NewFile => Self::NewFile,
        }
    }
}

impl From<VfsAccessMode> for AccessMode {
    fn from(value: VfsAccessMode) -> Self {
        match value {
            VfsAccessMode::Read => Self::Read,
            VfsAccessMode::Write => Self::Write,
            VfsAccessMode::Update => Self::Update,
            VfsAccessMode::UpdateDescriptor => Self::UpdateDescriptor,
            VfsAccessMode::ShortDirectory => Self::ShortDirectory,
            VfsAccessMode::LongDirectory => Self::LongDirectory,
        }
    }
}

impl From<VfsSeekMode> for SeekMode {
    fn from(value: VfsSeekMode) -> Self {
        match value {
            VfsSeekMode::Backward => Self::Backward,
            VfsSeekMode::Absolute => Self::Absolute,
            VfsSeekMode::Forward => Self::Forward,
            VfsSeekMode::FromEnd => Self::FromEnd,
        }
    }
}

impl From<VfsReadDirection> for ReadDirection {
    fn from(value: VfsReadDirection) -> Self {
        match value {
            VfsReadDirection::Forward => Self::Forward,
            VfsReadDirection::Backward => Self::Backward,
        }
    }
}

impl From<VfsObjectMode> for ObjectMode {
    fn from(value: VfsObjectMode) -> Self {
        match value {
            VfsObjectMode::Byte => Self::Byte,
            VfsObjectMode::Directory => Self::Directory,
            VfsObjectMode::CompleteDirectory => Self::CompleteDirectory,
        }
    }
}

#[derive(Clone, Debug)]
pub enum VfsResponse {
    Simple(VfsResponseHeader),
    GetStatus(VfsGetStatusResponse),
    ReadDirPage(VfsReadDirPageResponse),
    Read(VfsReadResponse),
}

fn read_request(cursor: &mut io::Cursor<&[u8]>) -> Result<VfsReadRequest, FrameError> {
    let data_length = cursor.read_u16()?;
    ensure_empty(cursor, "VFS read payload")?;
    Ok(VfsReadRequest { data_length })
}

fn read_write_request<'a>(
    cursor: &mut io::Cursor<&'a [u8]>,
) -> Result<VfsWriteRequest<'a>, FrameError> {
    let data_length = cursor.read_u16()? as usize;
    let data = cursor.read_slice(data_length)?;
    ensure_empty(cursor, "VFS write payload")?;
    Ok(VfsWriteRequest { data })
}

fn read_single_byte(raw: &[u8], name: &str) -> Result<u8, FrameError> {
    raw.first()
        .copied()
        .filter(|_| raw.len() == 1)
        .ok_or_else(|| FrameError::Validation {
            reason: format!(
                "invalid VFS set {name} length: expected 1, found {}",
                raw.len()
            ),
        })
}

fn ensure_empty(cursor: &io::Cursor<&[u8]>, context: &str) -> Result<(), FrameError> {
    let remaining = cursor
        .get_ref()
        .len()
        .saturating_sub(cursor.position() as usize);
    if remaining == 0 {
        Ok(())
    } else {
        Err(FrameError::Validation {
            reason: format!("{context}: {remaining} trailing bytes"),
        })
    }
}

impl<'a> VfsRequest<'a> {
    pub fn try_from_slice(data: &'a [u8]) -> Result<Self, FrameError> {
        let mut cursor = io::Cursor::new(data);
        let header = VfsRequestHeader {
            request: cursor.read_u16()?,
            requestors_conn_id: cursor.read_u16()?,
            servers_conn_id: cursor.read_u16()?,
        };

        let body = match VfsRequestCode::from_u16(header.request) {
            Some(VfsRequestCode::Attach) => {
                let mode = VfsAttachMode::from_u8(cursor.read_u8()?).ok_or_else(|| {
                    FrameError::Validation {
                        reason: "invalid VFS attach mode".to_owned(),
                    }
                })?;
                let access = VfsAccessMode::from_u8(cursor.read_u8()?).ok_or_else(|| {
                    FrameError::Validation {
                        reason: "invalid VFS access mode".to_owned(),
                    }
                })?;
                let password = cursor.read_array()?;
                let path = read_small_slice(&mut cursor).and_then(|path| {
                    GRiDPath::try_from(path).map_err(|reason| FrameError::Validation {
                        reason: reason.to_owned(),
                    })
                })?;
                ensure_empty(&cursor, "VFS attach payload")?;
                VfsRequestBody::Attach(VfsAttachRequest {
                    mode,
                    access,
                    password,
                    path,
                })
            }
            Some(VfsRequestCode::Open) => {
                let num_buf = cursor.read_u8()?;
                ensure_empty(&cursor, "VFS open payload")?;
                VfsRequestBody::Open(VfsOpenRequest { num_buf })
            }
            Some(VfsRequestCode::GetStatus) => {
                VfsRequestBody::GetStatus(read_request(&mut cursor)?)
            }
            Some(VfsRequestCode::Read) => VfsRequestBody::Read(read_request(&mut cursor)?),
            Some(VfsRequestCode::ReadDesc) => VfsRequestBody::ReadDesc(read_request(&mut cursor)?),
            Some(VfsRequestCode::ReadDirPage) => {
                VfsRequestBody::ReadDirPage(read_request(&mut cursor)?)
            }
            Some(VfsRequestCode::Seek) => {
                let mode = VfsSeekMode::from_u8(cursor.read_u8()?).ok_or_else(|| {
                    FrameError::Validation {
                        reason: "invalid VFS seek mode".to_owned(),
                    }
                })?;
                let position = cursor.read_u32()?;
                ensure_empty(&cursor, "VFS seek payload")?;
                VfsRequestBody::Seek(VfsSeekRequest { mode, position })
            }
            Some(VfsRequestCode::Write) => VfsRequestBody::Write(read_write_request(&mut cursor)?),
            Some(VfsRequestCode::WriteDesc) => {
                VfsRequestBody::WriteDesc(read_write_request(&mut cursor)?)
            }
            Some(VfsRequestCode::SetStatus) => {
                let data_length = cursor.read_u16()? as usize;
                let data = cursor.read_slice(data_length)?;
                ensure_empty(&cursor, "VFS set status payload")?;
                let mut data_cursor = io::Cursor::new(data);
                let mut actions = Vec::new();
                while data_cursor.position() < data.len() as u64 {
                    let ty = data_cursor.read_u8()?;
                    let length = data_cursor.read_u16()? as usize;
                    let raw = data_cursor.read_slice(length)?;
                    actions.push(VfsSetStatusAction::from_raw(ty, raw)?);
                }
                VfsRequestBody::SetStatus(VfsSetStatusRequest { actions })
            }
            Some(VfsRequestCode::Detach) => {
                ensure_empty(&cursor, "VFS detach payload")?;
                VfsRequestBody::Detach
            }
            Some(VfsRequestCode::Close) => {
                ensure_empty(&cursor, "VFS close payload")?;
                VfsRequestBody::Close
            }
            Some(VfsRequestCode::Flush) => {
                ensure_empty(&cursor, "VFS flush payload")?;
                VfsRequestBody::Flush
            }
            Some(_) | None => VfsRequestBody::Unknown(cursor.read_remainder()),
        };

        Ok(Self { header, body })
    }
}

pub(super) fn response_header(
    request: VfsRequestCode,
    header: &VfsRequestHeader,
    status: u16,
) -> VfsResponseHeader {
    VfsResponseHeader {
        response: VFS_RESPONSE_BIT | request.to_u16().expect("valid VFS request code"),
        servers_conn_id: header.servers_conn_id,
        requestors_conn_id: header.requestors_conn_id,
        status,
    }
}

pub(super) fn simple_response(
    request: VfsRequestCode,
    header: &VfsRequestHeader,
    error: u16,
) -> VfsResponse {
    VfsResponse::Simple(response_header(request, header, error))
}

fn write_header(dst: &mut Vec<u8>, header: &VfsResponseHeader) -> Result<(), FrameError> {
    dst.write_u16(header.response)?;
    dst.write_u16(header.servers_conn_id)?;
    dst.write_u16(header.requestors_conn_id)?;
    dst.write_u16(header.status)?;
    Ok(())
}

impl VfsResponse {
    /// Appends the wire form of this response to `dst`.
    ///
    /// Every length prefix is patched in after its body has been written, so a
    /// prefix cannot disagree with the bytes it describes the way a separate
    /// counting pass over the same data could.
    pub fn write_into(&self, dst: &mut Vec<u8>) -> Result<(), FrameError> {
        let start = dst.len();

        match self {
            Self::Simple(response) => write_header(dst, response)?,
            Self::GetStatus(response) => {
                write_header(dst, &response.header)?;
                with_u16_len(dst, |dst| {
                    dst.write_u8(response.open as u8)?;
                    dst.write_u8(response.access.to_u8().expect("valid VFS access mode"))?;
                    dst.write_u8(response.seek as u8)?;
                    dst.write_u32(response.file_position)?;
                    dst.write_u32(response.file_length)?;
                    dst.write_u16(response.num_pages)?;
                    dst.write_u16(response.num_pages_alloc)?;
                    Ok(())
                })?;
            }
            Self::ReadDirPage(response) => {
                write_header(dst, &response.header)?;
                with_u16_len(dst, |dst| {
                    for entry in &response.entries {
                        if entry.name.len() > VFS_MAX_FILE_NAME_LENGTH {
                            return Err(FrameError::Validation {
                                reason: format!(
                                    "VFS directory entry name exceeds the maximum length of {VFS_MAX_FILE_NAME_LENGTH} bytes"
                                ),
                            });
                        }
                        let name_length = u8_len(entry.name.len(), "VFS directory entry name")?;
                        dst.write_array([0; 4])?;
                        dst.write_u32(
                            DIRECTORY_ENTRY_PREAMBLE_LEN as u32 + u32::from(name_length),
                        )?;
                        dst.write_u8(name_length)?;
                        dst.write_all(&entry.name)?;
                    }
                    Ok(())
                })?;
            }
            Self::Read(response) => {
                write_header(dst, &response.header)?;
                with_u16_len(dst, |dst| {
                    dst.write_all(&response.data)?;
                    Ok(())
                })?;
            }
        }

        if dst.len() - start > VFS_MAX_MESSAGE_LENGTH {
            dst.truncate(start);
            return Err(FrameError::Validation {
                reason: format!(
                    "VFS response exceeds the maximum length of {VFS_MAX_MESSAGE_LENGTH} bytes"
                ),
            });
        }

        Ok(())
    }
}

impl<'a> VfsSetStatusAction<'a> {
    fn from_raw(ty: u8, raw: &'a [u8]) -> Result<Self, FrameError> {
        let Some(ty) = VfsSetStatusType::from_u8(ty) else {
            return Ok(Self::Unknown { ty, raw });
        };
        match ty {
            VfsSetStatusType::SetDirection => {
                let direction = VfsReadDirection::from_u8(read_single_byte(raw, "direction")?)
                    .ok_or_else(|| FrameError::Validation {
                        reason: format!("invalid VFS read direction: {}", raw[0]),
                    })?;
                Ok(Self::SetDirection { direction })
            }
            VfsSetStatusType::SetWildcard => Ok(Self::SetWildcard {
                pattern: BStr::new(raw),
            }),
            VfsSetStatusType::SetObjectMode => {
                let raw_mode = read_single_byte(raw, "object mode")?;
                let mode =
                    VfsObjectMode::from_u8(raw_mode).ok_or_else(|| FrameError::Validation {
                        reason: format!("invalid VFS object mode: {raw_mode}"),
                    })?;
                Ok(Self::SetObjectMode { mode })
            }
            VfsSetStatusType::SetGpibAddress => Ok(Self::SetGpibAddress { raw }),
            VfsSetStatusType::SetDeviceMask => Ok(Self::SetDeviceMask { raw }),
            VfsSetStatusType::SetNoteValue => Ok(Self::SetNoteValue { raw }),
            VfsSetStatusType::SetNumBuffers => Ok(Self::SetNumBuffers { raw }),
            VfsSetStatusType::SetNameAttributes => Ok(Self::SetNameAttributes { raw }),
            VfsSetStatusType::SetConsoleMode => Ok(Self::SetConsoleMode { raw }),
            VfsSetStatusType::SetAccessRights => Ok(Self::SetAccessRights { raw }),
            VfsSetStatusType::SetSecureMode => Ok(Self::SetSecureMode { raw }),
            VfsSetStatusType::SetIpcActivityTimeout => Ok(Self::SetIpcActivityTimeout { raw }),
            VfsSetStatusType::GetGenericDeviceName => Ok(Self::GetGenericDeviceName { raw }),
            VfsSetStatusType::GetDeviceName => Ok(Self::GetDeviceName { raw }),
        }
    }
}
