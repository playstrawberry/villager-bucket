//! Pictures in text: a small picture your players write as `:name:` in chat and in an anvil's
//! name (signs and books once the server takes them).
//!
//! ```ignore
//! use custom_models_api::{Picture, picture};
//!
//! // in on_load:
//! let coin = picture(&Picture::new("coin", include_bytes!("../pictures/coin.png")).permission("shop.vip"))?;
//! // coin.id == "shop.coin"; players type :coin: (or :shop.coin: when another plugin has a coin)
//! ```
//!
//! The picture is an RGBA PNG of at most 64 x 64 pixels, drawn `height` pixels high on the text
//! line (5 to 32; 9 by default, one chat line). Its code point never changes once given, so a
//! renamed item keeps its picture across restarts. A player needs the surface's permission
//! (`custom-models:pictures.chat`, `custom-models:pictures.anvil`; everybody by default) and the
//! picture's own `permission`, if you give one (register that node yourself). A player whose client
//! has not loaded the server's pack, and a Bedrock player, read the text that was typed. Limits:
//! 256 pictures per plugin, 2048 per server; 16 pictures in one chat message, 4 in a name.

use super::{Error, ask, field, invalid, with_files};

/// The tallest picture on a text line, in pixels.
pub const MAX_HEIGHT: u8 = 32;
/// The smallest.
pub const MIN_HEIGHT: u8 = 5;

/// A picture of your plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Picture<'a> {
    /// `[a-z0-9_]`, 1 to 32 characters. Its id becomes `<your plugin>.<name>`.
    pub name: &'a str,
    /// An RGBA PNG, at most 64 x 64 pixels and 32 KiB.
    pub png: &'a [u8],
    /// Its height on the text line, 5 to 32 pixels; `None`: 9.
    pub height: Option<u8>,
    /// How far it reaches above the baseline, 0 to its height; `None`: centred on the letters.
    pub ascent: Option<u8>,
    /// The node a player also needs to use it; `None`: the surface's node is enough.
    pub permission: Option<&'a str>,
}

impl<'a> Picture<'a> {
    #[must_use]
    pub const fn new(name: &'a str, png: &'a [u8]) -> Self {
        Self {
            name,
            png,
            height: None,
            ascent: None,
            permission: None,
        }
    }

    #[must_use]
    pub const fn height(mut self, height: u8) -> Self {
        self.height = Some(height);
        self
    }

    #[must_use]
    pub const fn ascent(mut self, ascent: u8) -> Self {
        self.ascent = Some(ascent);
        self
    }

    #[must_use]
    pub const fn permission(mut self, node: &'a str) -> Self {
        self.permission = Some(node);
        self
    }
}

/// A registered picture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredPicture {
    /// `<your plugin>.<name>`: `:<id>:` always names it.
    pub id: String,
    /// The character that draws it in the font `custom-models:pictures`.
    pub code: char,
    /// Whether players already get it (in the server's resource pack).
    pub ready: bool,
}

/// The bytes of a `picture` request.
pub fn picture_request(picture: &Picture<'_>) -> Result<Vec<u8>, Error> {
    let name = picture.name;
    if name.is_empty()
        || name.len() > 32
        || !name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    {
        return Err(invalid("a picture's name is [a-z0-9_], 1 to 32 characters"));
    }
    let mut settings = format!("picture {name}\n");
    if let Some(height) = picture.height {
        if !(MIN_HEIGHT..=MAX_HEIGHT).contains(&height) {
            return Err(invalid("a picture is 5 to 32 pixels high"));
        }
        settings.push_str(&format!("height={height}\n"));
    }
    if let Some(ascent) = picture.ascent {
        settings.push_str(&format!("ascent={ascent}\n"));
    }
    if let Some(node) = picture.permission {
        if node.is_empty() || node.contains(['\n', '\r', '=', ' ']) {
            return Err(invalid("a permission is one word"));
        }
        settings.push_str(&format!("permission={node}\n"));
    }
    Ok(with_files(settings, &[("picture.png", picture.png)]))
}

/// Registers (or replaces) one of your pictures. Call it in `on_load`, never at tick time: the
/// request carries the picture.
pub fn picture(picture: &Picture<'_>) -> Result<RegisteredPicture, Error> {
    let fields = ask(&picture_request(picture)?)?;
    let code = u32::from_str_radix(field(&fields, 1)?, 16)
        .ok()
        .and_then(char::from_u32)
        .ok_or_else(|| Error {
            kind: super::ErrorKind::Other,
            detail: "custom-models answered no code point".to_owned(),
        })?;
    Ok(RegisteredPicture {
        id: field(&fields, 0)?.to_owned(),
        code,
        ready: field(&fields, 2)? == "ready",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ErrorKind;

    #[test]
    fn a_picture_request_carries_its_settings_and_its_one_file() {
        let png = b"\x89PNG";
        let bytes = picture_request(
            &Picture::new("coin", png)
                .height(16)
                .ascent(12)
                .permission("shop.vip"),
        )
        .unwrap();
        let mut expected =
            b"picture coin\nheight=16\nascent=12\npermission=shop.vip\n\npicture.png 4\n".to_vec();
        expected.extend_from_slice(png);
        assert_eq!(bytes, expected);
        let plain = picture_request(&Picture::new("coin", png)).unwrap();
        assert!(plain.starts_with(b"picture coin\n\npicture.png 4\n"));
    }

    #[test]
    fn a_bad_picture_is_refused_before_it_is_sent() {
        for bad in [
            Picture::new("Coin", b"x"),
            Picture::new("", b"x"),
            Picture::new("coin", b"x").height(4),
            Picture::new("coin", b"x").height(33),
            Picture::new("coin", b"x").permission("a b"),
            Picture::new("coin", b"x").permission("a\nheight=1"),
        ] {
            assert_eq!(picture_request(&bad).unwrap_err().kind, ErrorKind::Invalid);
        }
    }
}
