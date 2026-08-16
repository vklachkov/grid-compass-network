pub mod status {
    pub const OK: u16 = 0;
    pub const PROPERTY_MISSING: u16 = 1001; // ePropListValueMissing
    pub const AUTHORIZATION_FILE: u16 = 1004; // eAuthorizationFile
}

pub mod property {
    pub const COMPANY: u8 = 0x07; // companyProp
    pub const GROUP: u8 = 0x08; // groupProp
    pub const USER: u8 = 0x09; // userProp
    pub const PASSWORD: u8 = 0x0a; // userPasswordProp
}

/// The application framing mail and broadcast share inside a VIPC message:
/// a four byte transport header, then `<marker><u16 length><tag + value>`.
pub mod app {
    pub const TAG_TERMINATOR: u8 = b'z';
    /// Set in the transport header while a request continues in a later frame.
    pub const MORE: u8 = 1;
    pub const TRANSPORT_HEADER_LEN: usize = 4;
}
