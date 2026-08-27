type Result<T, E = u16> = ::core::result::Result<T, E>;

pub trait ErrorCodeExt {
    fn error_code(&self) -> u16;
}

impl<T> ErrorCodeExt for Result<T, u16> {
    #[inline]
    fn error_code(&self) -> u16 {
        if let Err(err) = self { *err } else { 0 }
    }
}
