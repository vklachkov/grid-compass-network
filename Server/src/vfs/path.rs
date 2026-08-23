use std::fmt;

use bstr::{BStr, ByteSlice};

#[repr(transparent)]
#[derive(PartialEq, Eq)]
pub struct GRiDPath([u8]);

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct GRiDPathComponents<'a> {
    pub device: Option<&'a BStr>,
    pub folder: Option<&'a BStr>,
    pub file: Option<&'a BStr>,
}

impl fmt::Debug for GRiDPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("GRiDPath")
            .field(&BStr::new(&self.0))
            .finish()
    }
}

impl fmt::Debug for GRiDPathComponents<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GRiDPathComponents")
            .field("device", &self.device)
            .field("folder", &self.folder)
            .field("file", &self.file)
            .finish()
    }
}

impl<'a> GRiDPathComponents<'a> {
    pub fn new(device: Option<&'a BStr>, folder: Option<&'a BStr>, file: Option<&'a BStr>) -> Self {
        Self {
            device,
            folder,
            file,
        }
    }
}

impl GRiDPath {
    const COMPONENT_SEPARATOR: u8 = b'`';
    const SERVER_SEPARATOR: u8 = b':';
    const PASSWORD_SEPARATOR: u8 = b'|';

    pub fn try_from(path: &[u8]) -> Result<&Self, &'static str> {
        Self::validate(path)?;

        // SAFETY: `GRiDPath` is a transparent wrapper around `[u8]`.
        Ok(unsafe { &*(path as *const [u8] as *const Self) })
    }

    fn validate(path: &[u8]) -> Result<(), &'static str> {
        let Some(path_body) = path.strip_prefix(&[Self::COMPONENT_SEPARATOR]) else {
            return Err("path must start with `");
        };

        let (path_body, password) = match path_body
            .iter()
            .position(|&byte| byte == Self::PASSWORD_SEPARATOR)
        {
            Some(separator) => (&path_body[..separator], Some(&path_body[separator + 1..])),
            None => (path_body, None),
        };

        if password.is_some_and(|password| {
            password.is_empty() || password.contains(&Self::PASSWORD_SEPARATOR)
        }) {
            return Err("path password must not be empty or contain |");
        }

        let Some((server, components)) = path_body.split_once_str(&[Self::SERVER_SEPARATOR]) else {
            return Err("path must contain a server name followed by :");
        };

        if server.is_empty() {
            return Err("path server name must not be empty");
        }
        if server.contains(&Self::COMPONENT_SEPARATOR) {
            return Err("path server name must not contain `");
        }

        if components.contains(&Self::SERVER_SEPARATOR) {
            return Err("path must not contain more than one server separator");
        }

        let components = components
            .strip_suffix(&[Self::COMPONENT_SEPARATOR])
            .unwrap_or(components);

        if components.is_empty() {
            return Err("path device must not be empty");
        }

        if components
            .split(|&byte| byte == Self::COMPONENT_SEPARATOR)
            .any(|component| component.is_empty())
        {
            return Err("path components must not be empty");
        }

        Ok(())
    }

    #[cfg(test)]
    pub fn server(&self) -> Option<&BStr> {
        let path = self.0.strip_prefix(&[Self::COMPONENT_SEPARATOR])?;
        let separator = path
            .iter()
            .position(|&byte| byte == Self::SERVER_SEPARATOR)?;
        Some(BStr::new(&path[..separator]))
    }

    pub fn components(&self) -> GRiDPathComponents<'_> {
        let components = self
            .0
            .splitn(2, |&byte| byte == Self::SERVER_SEPARATOR)
            .nth(1)
            .unwrap();

        let components = components
            .splitn(2, |&byte| byte == Self::PASSWORD_SEPARATOR)
            .next()
            .unwrap();

        let mut components = components
            .split(|&byte| byte == Self::COMPONENT_SEPARATOR)
            .filter(|component| !component.is_empty())
            .map(BStr::new);

        GRiDPathComponents::new(components.next(), components.next(), components.next())
    }

    #[cfg(test)]
    pub fn password(&self) -> Option<&BStr> {
        let path = &self.0;
        let separator = path
            .iter()
            .position(|&byte| byte == Self::PASSWORD_SEPARATOR)?;
        Some(BStr::new(&path[separator + 1..]))
    }
}

#[cfg(test)]
mod tests {
    use super::GRiDPath;
    use bstr::BStr;

    #[test]
    fn parses_grid_paths() {
        let cases: &[(&[u8], Option<&[u8]>, Option<&[u8]>, Option<&[u8]>)] = &[
            (b"`server:Device", None, None, None),
            (b"`server:Device`Mail", Some(b"Mail"), None, None),
            (b"`server:Device`Mail`", Some(b"Mail"), None, None),
            (
                b"`server:Device`Mail`File~Text~",
                Some(b"Mail"),
                Some(b"File~Text~"),
                None,
            ),
            (
                b"`server:Device`Folder~Subject~`File~Text~",
                Some(b"Folder~Subject~"),
                Some(b"File~Text~"),
                None,
            ),
            (
                b"`server:Device`Folder~Subject~`File~Text~|Password",
                Some(b"Folder~Subject~"),
                Some(b"File~Text~"),
                Some(b"Password"),
            ),
            (
                b"`server:Device`Folder`File~Text~`Version",
                Some(b"Folder"),
                Some(b"File~Text~"),
                None,
            ),
        ];

        for &(raw, folder, file_name, password) in cases {
            let path = GRiDPath::try_from(raw).unwrap();

            assert_eq!(&path.0, raw);
            let components = path.components();

            assert_eq!(path.server(), Some(BStr::new(b"server")));
            assert_eq!(components.device, Some(BStr::new(b"Device")));
            assert_eq!(components.folder, folder.map(BStr::new));
            assert_eq!(components.file, file_name.map(BStr::new));
            assert_eq!(path.password(), password.map(BStr::new));
        }
    }

    #[test]
    fn accepts_non_utf8_names() {
        let path = GRiDPath::try_from(b"`server:Device`\xffolder`file~Data~").unwrap();

        assert_eq!(path.components().folder, Some(BStr::new(b"\xffolder")));
    }

    #[test]
    fn rejects_malformed_grid_paths() {
        for raw in [
            b"".as_slice(),
            b"server:Device",
            b"`:Device",
            b"`server:",
            b"`server:Device``Mail",
            b"`server:Device|",
            b"`server:Device|Password|Extra",
            b"`server:Dev:ice",
            b"`ser`ver:Device",
        ] {
            assert!(GRiDPath::try_from(raw).is_err(), "accepted {raw:?}");
        }
    }
}
