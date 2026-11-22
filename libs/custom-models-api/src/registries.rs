//! Music discs with a song of their own, paintings and armour trims.
//!
//! ```ignore
//! use custom_models_api::{Disc, Look, Texts, disc, sound};
//!
//! // in on_load, after the song's sound is registered with `sound`:
//! let look = Look { names: &[("en", "Mellow Disc")], stack: None, files: &[("texture.png", TEXTURE)] };
//! let mellow = disc(&Disc {
//!     name: "mellow", sound: "mellow_song", seconds: 95.5, comparator: 4,
//!     description: Texts { en: "Mellow", fr: "Doux", es: "Suave", tr: "Yumuşak" },
//!     item: look,
//! })?;
//! // mellow.item == Some("shop.mellow"), mellow.entry == "custom-models:shop_mellow"
//! ```
//!
//! The server's registries for these entries are fixed when it starts: register them in
//! `on_load`, every start, with the same values (a changed value is `Conflict`). After the
//! server took players a registration is `Unavailable`. Every key is yours (`<your plugin>_<name>`
//! in custom-models' namespace). Limits: 8 entries of each kind per plugin, 64 per server. Java
//! only: a Bedrock player sees a vanilla painting, no trim, and a disc that plays nothing.

use super::{Error, Texts, ask, field, invalid, with_files};

impl Texts<'_> {
    fn lines(&self, key: &str, out: &mut String) -> Result<(), Error> {
        for (language, text) in [
            ("en", self.en),
            ("fr", self.fr),
            ("es", self.es),
            ("tr", self.tr),
        ] {
            if text.is_empty() || text.contains(['\n', '\r']) {
                return Err(invalid(format!("{key}.{language} is one line of text")));
            }
            out.push_str(&format!("{key}.{language}={text}\n"));
        }
        Ok(())
    }
}

/// The item of a disc, a painting or a trim material, as a `model` request makes an item: names
/// (`en` always), the stack size and its look (`texture.png`, or `model.json` with its textures).
#[derive(Debug, Clone, Copy)]
pub struct Look<'a> {
    pub names: &'a [(&'a str, &'a str)],
    pub stack: Option<u8>,
    pub files: &'a [(&'a str, &'a [u8])],
}

impl Look<'_> {
    fn lines(&self, out: &mut String) -> Result<(), Error> {
        for (language, text) in self.names {
            if !["en", "fr", "es", "tr"].contains(language) || text.contains(['\n', '\r']) {
                return Err(invalid("a name is en, fr, es or tr and one line of text"));
            }
            out.push_str(&format!("name.{language}={text}\n"));
        }
        if let Some(stack) = self.stack {
            out.push_str(&format!("stack={stack}\n"));
        }
        Ok(())
    }
}

/// What a registration answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The item's model id (`<your plugin>.<name>`); none for a trim pattern, which has no item.
    pub item: Option<String>,
    /// The entry's id, `custom-models:<your plugin>_<name>`: what an item's component names.
    pub entry: String,
    /// Whether it is registered on the server (and the item in its pack).
    pub ready: bool,
}

fn name_ok(name: &str) -> Result<(), Error> {
    if name.is_empty()
        || name.len() > 48
        || !name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    {
        return Err(invalid("a name is [a-z0-9_], 1 to 48 characters"));
    }
    Ok(())
}

fn sent(request: &[u8]) -> Result<Entry, Error> {
    let fields = ask(request)?;
    Ok(Entry {
        item: Some(field(&fields, 0)?)
            .filter(|id| *id != "-")
            .map(str::to_owned),
        entry: field(&fields, 1)?.to_owned(),
        ready: field(&fields, 2)? == "ready",
    })
}

fn files_of<'a>(
    extra: &[(&'a str, &'a [u8])],
    look: Option<&Look<'a>>,
) -> Vec<(&'a str, &'a [u8])> {
    let mut files = extra.to_vec();
    if let Some(look) = look {
        files.extend_from_slice(look.files);
    }
    files
}

/// A music disc.
#[derive(Debug, Clone, Copy)]
pub struct Disc<'a> {
    pub name: &'a str,
    /// The name of one of your own sounds (`sound`): the song.
    pub sound: &'a str,
    /// How long it plays, more than 0 and at most 3600 seconds.
    pub seconds: f64,
    /// What a comparator reads from a jukebox playing it, 0 to 15.
    pub comparator: u8,
    /// What the disc says it is (the tooltip's line).
    pub description: Texts<'a>,
    pub item: Look<'a>,
}

/// The bytes of a `disc` request.
pub fn disc_request(disc: &Disc<'_>) -> Result<Vec<u8>, Error> {
    name_ok(disc.name)?;
    name_ok(disc.sound)?;
    if !(disc.seconds > 0.0 && disc.seconds <= 3600.0) || disc.comparator > 15 {
        return Err(invalid(
            "a disc plays more than 0 and at most 3600 seconds, comparator 0 to 15",
        ));
    }
    let mut settings = format!(
        "disc {}\nsound={}\nlength={}\ncomparator={}\n",
        disc.name, disc.sound, disc.seconds, disc.comparator
    );
    disc.description.lines("description", &mut settings)?;
    disc.item.lines(&mut settings)?;
    Ok(with_files(settings, &files_of(&[], Some(&disc.item))))
}

/// Registers one of your discs (in `on_load`).
pub fn disc(disc: &Disc<'_>) -> Result<Entry, Error> {
    sent(&disc_request(disc)?)
}

/// A painting.
#[derive(Debug, Clone, Copy)]
pub struct Painting<'a> {
    pub name: &'a str,
    /// Its size in blocks, 1 to 16 each.
    pub width: u8,
    pub height: u8,
    /// Its picture, a PNG in the proportion of its width and height.
    pub picture: &'a [u8],
    pub title: Texts<'a>,
    pub author: Option<Texts<'a>>,
    pub item: Look<'a>,
}

/// The bytes of a `painting` request.
pub fn painting_request(painting: &Painting<'_>) -> Result<Vec<u8>, Error> {
    name_ok(painting.name)?;
    if !(1..=16).contains(&painting.width) || !(1..=16).contains(&painting.height) {
        return Err(invalid("a painting is 1 to 16 blocks wide and high"));
    }
    let mut settings = format!(
        "painting {}\nwidth={}\nheight={}\n",
        painting.name, painting.width, painting.height
    );
    painting.title.lines("title", &mut settings)?;
    if let Some(author) = &painting.author {
        author.lines("author", &mut settings)?;
    }
    painting.item.lines(&mut settings)?;
    Ok(with_files(
        settings,
        &files_of(&[("picture.png", painting.picture)], Some(&painting.item)),
    ))
}

/// Registers one of your paintings (in `on_load`).
pub fn painting(painting: &Painting<'_>) -> Result<Entry, Error> {
    sent(&painting_request(painting)?)
}

/// An armour trim pattern.
#[derive(Debug, Clone, Copy)]
pub struct TrimPattern<'a> {
    pub name: &'a str,
    /// Drawn over the armour as it is (vanilla's `decal`).
    pub decal: bool,
    /// The textures in grey tones: on the armour's body and on its leggings.
    pub pattern: &'a [u8],
    pub leggings: &'a [u8],
    pub description: Texts<'a>,
}

/// The bytes of a `trim` request.
pub fn trim_pattern_request(pattern: &TrimPattern<'_>) -> Result<Vec<u8>, Error> {
    name_ok(pattern.name)?;
    let mut settings = format!("trim {}\ndecal={}\n", pattern.name, pattern.decal);
    pattern.description.lines("description", &mut settings)?;
    Ok(with_files(
        settings,
        &[
            ("pattern.png", pattern.pattern),
            ("leggings.png", pattern.leggings),
        ],
    ))
}

/// Registers one of your trim patterns (in `on_load`). An armour item wears it with the setting
/// `trim=<pattern name>,<material name>` of a `model` request.
pub fn trim_pattern(pattern: &TrimPattern<'_>) -> Result<Entry, Error> {
    sent(&trim_pattern_request(pattern)?)
}

/// An armour trim material and its ingredient item.
#[derive(Debug, Clone, Copy)]
pub struct TrimMaterial<'a> {
    pub name: &'a str,
    /// The colour of its name in the tooltip, `0xRRGGBB`.
    pub color: u32,
    /// Its palette, a PNG of 8 by 1 pixels.
    pub palette: &'a [u8],
    pub description: Texts<'a>,
    /// The ingredient: a smithing table puts its material on armour.
    pub item: Look<'a>,
}

/// The bytes of a `trim-material` request.
pub fn trim_material_request(material: &TrimMaterial<'_>) -> Result<Vec<u8>, Error> {
    name_ok(material.name)?;
    if material.color > 0x00FF_FFFF {
        return Err(invalid("a colour is 0xRRGGBB"));
    }
    let mut settings = format!(
        "trim-material {}\ncolor={:06X}\n",
        material.name, material.color
    );
    material.description.lines("description", &mut settings)?;
    material.item.lines(&mut settings)?;
    Ok(with_files(
        settings,
        &files_of(&[("palette.png", material.palette)], Some(&material.item)),
    ))
}

/// Registers one of your trim materials (in `on_load`).
pub fn trim_material(material: &TrimMaterial<'_>) -> Result<Entry, Error> {
    sent(&trim_material_request(material)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ErrorKind;

    const TEXTS: Texts<'static> = Texts {
        en: "Mellow",
        fr: "Doux",
        es: "Suave",
        tr: "Yumusak",
    };
    const LOOK: Look<'static> = Look {
        names: &[("en", "Mellow Disc")],
        stack: None,
        files: &[("texture.png", b"PNG".as_slice())],
    };

    #[test]
    fn a_disc_request_carries_its_settings_texts_and_item_look() {
        let bytes = disc_request(&Disc {
            name: "mellow",
            sound: "song",
            seconds: 95.5,
            comparator: 4,
            description: TEXTS,
            item: LOOK,
        })
        .unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.starts_with("disc mellow\nsound=song\nlength=95.5\ncomparator=4\n"));
        assert!(text.contains("description.tr=Yumusak\n"));
        assert!(text.contains("name.en=Mellow Disc\n"));
        assert!(text.ends_with("\n\ntexture.png 3\nPNG"));
    }

    #[test]
    fn a_painting_carries_its_picture_before_the_look_and_an_optional_author() {
        let painting = Painting {
            name: "sunset",
            width: 2,
            height: 1,
            picture: b"PIC",
            title: TEXTS,
            author: Some(TEXTS),
            item: LOOK,
        };
        let text = String::from_utf8(painting_request(&painting).unwrap()).unwrap();
        assert!(text.contains("author.fr=Doux\n") && text.contains("title.es=Suave\n"));
        assert!(text.ends_with("\n\npicture.png 3\nPICtexture.png 3\nPNG"));
        let plain = painting_request(&Painting {
            author: None,
            ..painting
        })
        .unwrap();
        assert!(!String::from_utf8(plain).unwrap().contains("author."));
    }

    #[test]
    fn a_trim_has_no_item_and_a_material_names_its_colour_in_six_digits() {
        let pattern = TrimPattern {
            name: "spiral",
            decal: true,
            pattern: b"A",
            leggings: b"B",
            description: TEXTS,
        };
        let text = String::from_utf8(trim_pattern_request(&pattern).unwrap()).unwrap();
        assert!(text.starts_with("trim spiral\ndecal=true\n"));
        assert!(text.ends_with("\n\npattern.png 1\nAleggings.png 1\nB"));
        let material = TrimMaterial {
            name: "ruby",
            color: 0x0000_7F10,
            palette: b"P",
            description: TEXTS,
            item: LOOK,
        };
        let text = String::from_utf8(trim_material_request(&material).unwrap()).unwrap();
        assert!(text.starts_with("trim-material ruby\ncolor=007F10\n"));
        assert!(text.ends_with("palette.png 1\nPtexture.png 3\nPNG"));
    }

    #[test]
    fn a_bad_request_is_refused_before_it_is_sent() {
        let good = Disc {
            name: "mellow",
            sound: "song",
            seconds: 10.0,
            comparator: 4,
            description: TEXTS,
            item: LOOK,
        };
        let multi = Texts {
            en: "a\nsound=other",
            ..TEXTS
        };
        for bad in [
            Disc {
                name: "Mellow",
                ..good
            },
            Disc { sound: "", ..good },
            Disc {
                seconds: 0.0,
                ..good
            },
            Disc {
                seconds: f64::NAN,
                ..good
            },
            Disc {
                seconds: 3600.5,
                ..good
            },
            Disc {
                comparator: 16,
                ..good
            },
            Disc {
                description: multi,
                ..good
            },
        ] {
            assert_eq!(disc_request(&bad).unwrap_err().kind, ErrorKind::Invalid);
        }
        let painting = Painting {
            name: "p",
            width: 0,
            height: 1,
            picture: b"x",
            title: TEXTS,
            author: None,
            item: LOOK,
        };
        assert_eq!(
            painting_request(&painting).unwrap_err().kind,
            ErrorKind::Invalid
        );
        let material = TrimMaterial {
            name: "m",
            color: 0x0100_0000,
            palette: b"x",
            description: TEXTS,
            item: LOOK,
        };
        assert_eq!(
            trim_material_request(&material).unwrap_err().kind,
            ErrorKind::Invalid
        );
    }
}
