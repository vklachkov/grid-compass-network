use std::{collections::HashMap, io, mem::size_of, num::NonZeroU16};

use bstr::BStr;
use log::{debug, warn};

use std::io::Write;

use super::protocol::status;
use crate::shared::{
    FrameError,
    io::{CursorExt, ReadExt, WriteExt, read_small_slice, u8_len, with_u16_len},
};

const VFS_RESPONSE_BIT: u16 = 0x8000;
const VFS_PASSWORD_LEN: usize = 17;
const VFS_DESCRIPTOR_LEN: usize = 198;
const VFS_DESCRIPTOR_PROPERTY_LENGTH_OFFSET: usize = 164;
const DIRECTORY_ENTRY_PREAMBLE_LEN: u32 = 9;
const MAX_MAIL_OBJECT_SIZE: usize = 16 * 1024 * 1024;

const VFS_ERROR_NOT_SUPPORTED: u16 = 35; // eNotSupport
const VFS_ERROR_DEVICE_FULL: u16 = 41; // eDeviceFull
const VFS_ERROR_FILE_NOT_OPEN: u16 = 205; // eFileNotOpen
const VFS_ERROR_BAD_CONNECTION: u16 = 221; // eBadConn
const VFS_ERROR_BAD_PARAMETER: u16 = 225; // eParam

const RESOURCES: &[&str] = &["Hard Disk~FS~"];
const HARD_DISK: &[&str] = &[
    "Folder 1~Subject~",
    "Folder 3~Subject~",
    "Folder 2~Subject~",
];
const HARD_DISK_FILES: &[&str] = &["Demo file~Text~"];
const READ_STUB: &[u8] = b"Read stub";

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

#[derive(Clone, Copy, Debug)]
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
    Unknown(&'a [u8]),
}

/// Only the path steers the server so far; the rest of the request is decoded
/// because it reaches the operator through the `Debug` line the dispatcher logs.
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct VfsAttachRequest<'a> {
    pub mode: VfsAttachMode,
    pub access: VfsAccessMode,
    pub password: [u8; VFS_PASSWORD_LEN],
    pub path: Path<'a>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

#[derive(Clone, Copy, Debug)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    pub error: u16,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum VfsSeekMode {
    Backward = 1,
    Absolute = 2,
    Forward = 3,
    FromEnd = 4,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Path<'a> {
    pub server: &'a BStr,
    pub components: Vec<&'a BStr>,
}

#[derive(Clone, Debug)]
pub enum VfsResponse {
    Simple(VfsResponseHeader),
    GetStatus(VfsGetStatusResponse),
    ReadDirPage(VfsReadDirPageResponse),
    Read(VfsReadResponse),
}

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

impl VfsAttachMode {
    fn from_repr(value: u8) -> Option<Self> {
        Some(match value {
            1 => Self::OldFile,
            2 => Self::UpdateFile,
            3 => Self::NewFile,
            _ => return None,
        })
    }
}

impl VfsRequestCode {
    pub fn from_repr(value: u16) -> Option<Self> {
        Some(match value {
            0 => Self::Initialize,
            1 => Self::GetStatus,
            2 => Self::Open,
            3 => Self::Close,
            4 => Self::Read,
            5 => Self::Write,
            6 => Self::Seek,
            7 => Self::Truncate,
            8 => Self::Attach,
            9 => Self::Detach,
            10 => Self::Rename,
            11 => Self::Delete,
            12 => Self::ReadDesc,
            13 => Self::WriteDesc,
            14 => Self::Flush,
            15 => Self::WaitSrq,
            16 => Self::SelfTest,
            17 => Self::Format,
            20 => Self::SetStatus,
            21 => Self::Deactivate,
            22 => Self::TrackFormat,
            23 => Self::ControllerTest,
            24 => Self::RamTest,
            25 => Self::DriveTest,
            26 => Self::Program,
            27 => Self::WriteProtect,
            28 => Self::BufferCommand,
            29 => Self::ReadDirPage,
            _ => return None,
        })
    }
}

impl VfsSetStatusType {
    pub fn from_repr(value: u8) -> Option<Self> {
        Some(match value {
            255 => Self::SetDirection,
            254 => Self::SetWildcard,
            253 => Self::SetObjectMode,
            252 => Self::SetGpibAddress,
            251 => Self::SetDeviceMask,
            250 => Self::SetNoteValue,
            249 => Self::SetNumBuffers,
            248 => Self::SetNameAttributes,
            247 => Self::SetConsoleMode,
            246 => Self::SetAccessRights,
            245 => Self::SetSecureMode,
            244 => Self::SetIpcActivityTimeout,
            243 => Self::GetGenericDeviceName,
            242 => Self::GetDeviceName,
            _ => return None,
        })
    }
}

impl VfsObjectMode {
    pub fn from_repr(value: u8) -> Option<Self> {
        Some(match value {
            0 => Self::Byte,
            1 => Self::Directory,
            2 => Self::CompleteDirectory,
            _ => return None,
        })
    }
}

impl VfsSeekMode {
    fn from_repr(value: u8) -> Option<Self> {
        Some(match value {
            1 => Self::Backward,
            2 => Self::Absolute,
            3 => Self::Forward,
            4 => Self::FromEnd,
            _ => return None,
        })
    }
}

impl VfsAccessMode {
    fn from_repr(value: u8) -> Option<Self> {
        Some(match value {
            1 => Self::Read,
            2 => Self::Write,
            3 => Self::Update,
            4 => Self::UpdateDescriptor,
            5 => Self::ShortDirectory,
            6 => Self::LongDirectory,
            _ => return None,
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

        let body = match VfsRequestCode::from_repr(header.request) {
            Some(VfsRequestCode::Attach) => {
                let mode = VfsAttachMode::from_repr(cursor.read_u8()?).ok_or_else(|| {
                    FrameError::Validation {
                        reason: "invalid VFS attach mode".to_owned(),
                    }
                })?;
                let access = VfsAccessMode::from_repr(cursor.read_u8()?).ok_or_else(|| {
                    FrameError::Validation {
                        reason: "invalid VFS access mode".to_owned(),
                    }
                })?;
                let password = cursor.read_array()?;
                let path = read_small_slice(&mut cursor).and_then(Path::try_from_slice)?;
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
                let mode = VfsSeekMode::from_repr(cursor.read_u8()?).ok_or_else(|| {
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
            Some(_) | None => VfsRequestBody::Unknown(cursor.read_remainder()),
        };

        Ok(Self { header, body })
    }
}

impl VfsResponse {
    /// Appends the wire form of this response to `dst`.
    ///
    /// Every length prefix is patched in after its body has been written, so a
    /// prefix cannot disagree with the bytes it describes the way a separate
    /// counting pass over the same data could.
    pub fn write_into(&self, dst: &mut Vec<u8>) -> Result<(), FrameError> {
        match self {
            Self::Simple(response) => write_header(dst, response)?,
            Self::GetStatus(response) => {
                write_header(dst, &response.header)?;
                with_u16_len(dst, |dst| {
                    dst.write_u8(response.open as u8)?;
                    dst.write_u8(response.access as u8)?;
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
                        let name_length = u8_len(entry.name.len(), "VFS directory entry name")?;
                        dst.write_array([0; 4])?;
                        dst.write_u32(DIRECTORY_ENTRY_PREAMBLE_LEN + u32::from(name_length))?;
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

        Ok(())
    }
}

impl<'a> Path<'a> {
    pub fn try_from_slice(data: &'a [u8]) -> Result<Self, FrameError> {
        let path = data
            .strip_prefix(b"`")
            .ok_or_else(|| FrameError::Validation {
                reason: "path must start with `".to_owned(),
            })?;
        let Some(separator) = path.iter().position(|&byte| byte == b':') else {
            return Err(FrameError::Validation {
                reason: "path must contain a server name followed by :".to_owned(),
            });
        };
        let server = &path[..separator];
        let resource = &path[separator + 1..];
        if server.is_empty() {
            return Err(FrameError::Validation {
                reason: "path server name must not be empty".to_owned(),
            });
        }
        let components = resource
            .split(|&byte| byte == b'`')
            .map(|component| {
                if component.is_empty() {
                    Err(FrameError::Validation {
                        reason: "path components must not be empty".to_owned(),
                    })
                } else {
                    Ok(BStr::new(component))
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            server: BStr::new(server),
            components,
        })
    }
}

impl<'a> VfsSetStatusAction<'a> {
    fn from_raw(ty: u8, raw: &'a [u8]) -> Result<Self, FrameError> {
        let Some(ty) = VfsSetStatusType::from_repr(ty) else {
            return Ok(Self::Unknown { ty, raw });
        };
        match ty {
            VfsSetStatusType::SetDirection => Ok(Self::SetDirection { raw }),
            VfsSetStatusType::SetWildcard => Ok(Self::SetWildcard {
                pattern: BStr::new(raw),
            }),
            VfsSetStatusType::SetObjectMode => {
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

fn response_header(request: u16, header: &VfsRequestHeader, error: u16) -> VfsResponseHeader {
    VfsResponseHeader {
        response: VFS_RESPONSE_BIT | request,
        servers_conn_id: header.servers_conn_id,
        requestors_conn_id: header.requestors_conn_id,
        error,
    }
}

fn simple_response(request: u16, header: &VfsRequestHeader, error: u16) -> VfsResponse {
    VfsResponse::Simple(response_header(request, header, error))
}

fn write_header(dst: &mut Vec<u8>, header: &VfsResponseHeader) -> Result<(), FrameError> {
    dst.write_u16(header.response)?;
    dst.write_u16(header.servers_conn_id)?;
    dst.write_u16(header.requestors_conn_id)?;
    dst.write_u16(header.error)?;
    Ok(())
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
