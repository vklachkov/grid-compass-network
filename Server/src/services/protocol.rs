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
