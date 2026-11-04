//! Typed calls for plugins that use custom-models.
//!
//! A plugin keeps its models in its own folder and registers them when it loads; a server then has
//! exactly the models of the plugins installed on it.
//!
//! ```ignore
//! use custom_models_api::{Model, register};
//!
//! const BUCKET: Model<'static> = Model {
//!     name: "villager_bucket",
//!     names: &[("en", "Bucket of Villager"), ("tr", "Köylü Kovası")],
//!     base: None,
//!     stack: Some(1),
//!     files: &[("texture.png", include_bytes!("../models/villager_bucket.png"))],
//! };
//!
//! // in on_load, with "custom-models" in the metadata's dependencies:
//! let bucket = register(&BUCKET)?; // bucket.id == "villager-bucket.villager_bucket"
//! ```
//!
//! Every call answers `Err` with a kind the caller can act on: `NotInstalled` (custom-models is
//! not on this server), `Unavailable` (the model is not in the server's resource pack yet, so a
//! player could not see it), `Full` (no room in the inventory), `Conflict` (another plugin has this
//! block or name), `Unknown`, `Invalid`, `Limit`.
//!
//! The look of a server is named once too and comes out for Java and Bedrock: [`retexture`] a
//! vanilla block, [`sound`], and a language [`line`].
//! A plugin with a valuable item treats anything but `Ok` as "give nothing".

use strawberry_plugin_api::{ItemStack, Player, persistent_data::PersistentDataHolder};

/// The plugin these calls talk to.
pub const PLUGIN: &str = "custom-models";

/// What went wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// custom-models is not on this server.
    NotInstalled,
    /// Not possible on this server yet (the model is not in its resource pack).
    Unavailable,
    /// A bad request (a name, a count, a file).
    Invalid,
    /// No such model or player.
    Unknown,
    /// A size or count limit.
    Limit,
    /// No room in the player's inventory; nothing was given.
    Full,
    /// Another plugin already has this block or name.
    Conflict,
    /// An answer this version does not know.
    Other,
}

/// An error with the reason custom-models gave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub kind: ErrorKind,
    pub detail: String,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.detail)
    }
}

impl std::error::Error for Error {}

fn invalid(detail: impl Into<String>) -> Error {
    Error {
        kind: ErrorKind::Invalid,
        detail: detail.into(),
    }
}

/// A model of your plugin.
#[derive(Debug, Clone, Copy)]
pub struct Model<'a> {
    /// `[a-z0-9_]`, 1 to 48 characters. Its id becomes `<your plugin>.<name>`.
    pub name: &'a str,
    /// The item's name per language code (`en` required; `fr`, `es`, `tr`).
    pub names: &'a [(&'a str, &'a str)],
    /// The base item (`minecraft:paper` when `None`).
    pub base: Option<&'a str>,
    /// The stack size, 1 to 99 (64 when `None`).
    pub stack: Option<u8>,
    /// `texture.png` for a flat item, or `model.json` (a Blockbench "Java Block/Item" export) with
    /// its textures as `textures/<name>.png`.
    pub files: &'a [(&'a str, &'a [u8])],
}

/// The bytes of a `model` request for `model`.
pub fn model_request(model: &Model<'_>) -> Result<Vec<u8>, Error> {
    let line_safe = |s: &str| !s.contains(['\n', '\r', '=']);
    if !line_safe(model.name) {
        return Err(invalid("a model name has no line breaks or '='"));
    }
    let mut out = format!("model {}\n", model.name);
    for (lang, text) in model.names {
        if !line_safe(lang) || text.contains(['\n', '\r']) {
            return Err(invalid("a name has no line breaks"));
        }
        out.push_str(&format!("name.{lang}={text}\n"));
    }
    if let Some(base) = model.base {
        if !line_safe(base) {
            return Err(invalid("a base item has no line breaks"));
        }
        out.push_str(&format!("base={base}\n"));
    }
    if let Some(stack) = model.stack {
        out.push_str(&format!("stack={stack}\n"));
    }
    let mut out = out.into_bytes();
    out.push(b'\n');
    for (path, bytes) in model.files {
        if path.contains(['\n', '\r', ' ']) {
            return Err(invalid("a file path has no spaces or line breaks"));
        }
        out.extend_from_slice(format!("{path} {}\n", bytes.len()).as_bytes());
        out.extend_from_slice(bytes);
    }
    Ok(out)
}

/// The fields of an answer, or its error.
pub fn parse_answer(answer: &[u8]) -> Result<Vec<String>, Error> {
    let text = String::from_utf8_lossy(answer);
    let mut fields = text.split('\t').map(str::to_owned);
    match fields.next().as_deref() {
        Some("ok") => Ok(fields.collect()),
        _ => Err(Error {
            kind: ErrorKind::Other,
            detail: text.into_owned(),
        }),
    }
}

/// The kind of an error text from custom-models.
#[must_use]
pub fn error_from(text: &str) -> Error {
    let kind = match text.split_once(':').map(|(code, _)| code) {
        Some("unavailable") => ErrorKind::Unavailable,
        Some("invalid") => ErrorKind::Invalid,
        Some("unknown") => ErrorKind::Unknown,
        Some("limit") => ErrorKind::Limit,
        Some("full") => ErrorKind::Full,
        Some("conflict") => ErrorKind::Conflict,
        _ => ErrorKind::Other,
    };
    Error {
        kind,
        detail: text.to_owned(),
    }
}

/// Sends one request to custom-models.
fn ask(message: &[u8]) -> Result<Vec<String>, Error> {
    match strawberry_plugin_api::ipc::send_ipc_message(PLUGIN, message) {
        Ok(Ok(answer)) => parse_answer(&answer),
        Ok(Err(text)) => Err(error_from(&text)),
        Err(()) => Err(Error {
            kind: ErrorKind::NotInstalled,
            detail: "custom-models is not on this server".to_owned(),
        }),
    }
}

fn field(fields: &[String], i: usize) -> Result<&str, Error> {
    fields.get(i).map(String::as_str).ok_or_else(|| Error {
        kind: ErrorKind::Other,
        detail: "a short answer from custom-models".to_owned(),
    })
}

/// A registered model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registered {
    /// The id to use in every other call: `<your plugin>.<name>`.
    pub id: String,
    /// Whether players already get it (in the server's resource pack).
    pub ready: bool,
}

/// Registers (or replaces) one of your models. Call it in `on_load`.
pub fn register(model: &Model<'_>) -> Result<Registered, Error> {
    let fields = ask(&model_request(model)?)?;
    Ok(Registered {
        id: field(&fields, 0)?.to_owned(),
        ready: field(&fields, 1)? == "ready",
    })
}

/// Which image goes on which face of a retextured block.
#[derive(Debug, Clone, Copy)]
pub enum Faces<'a> {
    /// One image on every face.
    All(&'a [u8]),
    /// The top, the bottom and the four sides.
    Sides {
        up: &'a [u8],
        down: &'a [u8],
        side: &'a [u8],
    },
}

/// A text in the four languages every text ships in.
#[derive(Debug, Clone, Copy)]
pub struct Texts<'a> {
    pub en: &'a str,
    pub fr: &'a str,
    pub es: &'a str,
    pub tr: &'a str,
}

impl Texts<'_> {
    fn settings(&self, prefix: &str, out: &mut String) -> Result<(), Error> {
        for (language, text) in [
            ("en", self.en),
            ("fr", self.fr),
            ("es", self.es),
            ("tr", self.tr),
        ] {
            if text.contains(['\n', '\r']) {
                return Err(invalid("a text has no line breaks"));
            }
            out.push_str(&format!("{prefix}.{language}={text}\n"));
        }
        Ok(())
    }
}

/// A sound of your plugin.
#[derive(Debug, Clone, Copy)]
pub struct Sound<'a> {
    /// `[a-z0-9_]`.
    pub name: &'a str,
    /// An Ogg Vorbis file.
    pub ogg: &'a [u8],
    /// For long loops: played while it loads instead of decoded into memory.
    pub stream: bool,
    /// `ambient`, `block`, `bottle`, `hostile`, `music`, `neutral`, `player`, `record`, `ui` or
    /// `weather`.
    pub category: &'a str,
    /// What Java's subtitles show.
    pub subtitle: Option<Texts<'a>>,
    /// `false`: Bedrock plays it at the same loudness wherever the player walks.
    pub positional: bool,
    /// Bedrock's distance limit in blocks.
    pub max_distance: Option<u32>,
}

/// The names of a registered sound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredSound {
    pub id: String,
    /// The event a Java player's client plays.
    pub java: String,
    /// The event a Bedrock player's client plays.
    pub bedrock: String,
}

fn word_safe(word: &str, what: &str) -> Result<(), Error> {
    if word.is_empty() || word.contains(['\n', '\r', ' ', '=']) {
        return Err(invalid(format!("{what} is one word")));
    }
    Ok(())
}

fn with_files(settings: String, files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = settings.into_bytes();
    out.push(b'\n');
    for (path, bytes) in files {
        out.extend_from_slice(format!("{path} {}\n", bytes.len()).as_bytes());
        out.extend_from_slice(bytes);
    }
    out
}

/// The bytes of a `retexture` request.
pub fn retexture_request(block: &str, faces: &Faces<'_>) -> Result<Vec<u8>, Error> {
    word_safe(block, "a block")?;
    let settings = format!("retexture {block}\n");
    Ok(match *faces {
        Faces::All(png) => with_files(settings, &[("all.png", png)]),
        Faces::Sides { up, down, side } => with_files(
            settings,
            &[("up.png", up), ("down.png", down), ("side.png", side)],
        ),
    })
}

/// The bytes of a `sound` request.
pub fn sound_request(sound: &Sound<'_>) -> Result<Vec<u8>, Error> {
    word_safe(sound.name, "a sound's name")?;
    word_safe(sound.category, "a category")?;
    let mut settings = format!(
        "sound {}\ncategory={}\nstream={}\npositional={}\n",
        sound.name, sound.category, sound.stream, sound.positional
    );
    if let Some(distance) = sound.max_distance {
        settings.push_str(&format!("max_distance={distance}\n"));
    }
    if let Some(subtitle) = &sound.subtitle {
        subtitle.settings("subtitle", &mut settings)?;
    }
    Ok(with_files(settings, &[("sound.ogg", sound.ogg)]))
}

/// The bytes of a `line` request.
pub fn line_request(name: &str, texts: &Texts<'_>) -> Result<Vec<u8>, Error> {
    word_safe(name, "a line's name")?;
    let mut settings = format!("line {name}\n");
    texts.settings("text", &mut settings)?;
    Ok(with_files(settings, &[]))
}

/// Gives a vanilla block a new look on this server, on Java and Bedrock. Call it in `on_load`. One
/// role per block: when another plugin has this block the answer is `Conflict`. The id comes back;
/// the look reaches players with the server's resource pack, after the server owner approves it in
/// the server panel.
pub fn retexture(block: &str, faces: &Faces<'_>) -> Result<String, Error> {
    let fields = ask(&retexture_request(block, faces)?)?;
    Ok(field(&fields, 0)?.to_owned())
}

/// Registers a sound for Java and Bedrock. Call it in `on_load`.
pub fn sound(sound: &Sound<'_>) -> Result<RegisteredSound, Error> {
    let fields = ask(&sound_request(sound)?)?;
    Ok(RegisteredSound {
        id: field(&fields, 0)?.to_owned(),
        java: field(&fields, 1)?.to_owned(),
        bedrock: field(&fields, 2)?.to_owned(),
    })
}

/// Registers a language line for Java and Bedrock. Call it in `on_load`. The key comes back.
pub fn line(name: &str, texts: &Texts<'_>) -> Result<String, Error> {
    let fields = ask(&line_request(name, texts)?)?;
    Ok(field(&fields, 0)?.to_owned())
}

fn uuid_of(player: &Player) -> String {
    strawberry_plugin_api::uuid::to_string(player.get_id())
}

/// Gives `count` items of the model `id` in vanilla's order (stacks of the same item first, then
/// empty slots). Nothing at all when they do not all fit (`Full`). The first slot used.
pub fn give(player: &Player, id: &str, count: u8) -> Result<u32, Error> {
    let fields = ask(format!("give {} {id} {count}", uuid_of(player)).as_bytes())?;
    field(&fields, 0)?
        .parse()
        .map_err(|_| error_from("other: a bad slot in the answer"))
}

/// Puts `count` items of the model `id` into the player's main or off hand, replacing what was
/// there.
pub fn hand(player: &Player, off_hand: bool, id: &str, count: u8) -> Result<(), Error> {
    let which = if off_hand { "off" } else { "main" };
    ask(format!("hand {} {which} {id} {count}", uuid_of(player)).as_bytes()).map(|_| ())
}

/// The model's name in the language of `locale` (`tr_tr`, `fr_fr`; English otherwise).
pub fn name(id: &str, locale: &str) -> Result<String, Error> {
    let fields = ask(format!("name {id} {locale}").as_bytes())?;
    Ok(field(&fields, 0)?.to_owned())
}

/// The model id of `stack`, read on the stack itself (no request): `None` for any other item.
/// The id is visible to players and a creative-mode player can copy it like any item data, so
/// never treat it as proof of value on its own.
#[must_use]
pub fn model_id(stack: &ItemStack) -> Option<String> {
    stack.get_string(PLUGIN, "id")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_model_request_is_settings_an_empty_line_then_the_files() {
        let model = Model {
            name: "villager_bucket",
            names: &[("en", "Bucket of Villager"), ("tr", "Köylü Kovası")],
            base: None,
            stack: Some(1),
            files: &[("texture.png", b"PNGDATA")],
        };
        let bytes = model_request(&model).unwrap();
        assert_eq!(
            String::from_utf8(bytes).unwrap(),
            "model villager_bucket\nname.en=Bucket of Villager\nname.tr=Köylü Kovası\nstack=1\n\ntexture.png 7\nPNGDATA"
        );
    }

    #[test]
    fn line_breaks_cannot_smuggle_settings() {
        let bad_name = Model {
            name: "x\nbase=minecraft:diamond",
            names: &[("en", "X")],
            base: None,
            stack: None,
            files: &[],
        };
        assert_eq!(
            model_request(&bad_name).unwrap_err().kind,
            ErrorKind::Invalid
        );
        let bad_text = Model {
            name: "x",
            names: &[("en", "X\nstack=99")],
            ..bad_name
        };
        assert_eq!(
            model_request(&bad_text).unwrap_err().kind,
            ErrorKind::Invalid
        );
        let bad_path = Model {
            name: "x",
            names: &[("en", "X")],
            files: &[("texture.png 1\nx", b"")],
            ..bad_name
        };
        assert_eq!(
            model_request(&bad_path).unwrap_err().kind,
            ErrorKind::Invalid
        );
    }

    #[test]
    fn answers_and_errors_are_read() {
        assert_eq!(
            parse_answer(b"ok\tvillager-bucket.villager_bucket\tpending").unwrap(),
            ["villager-bucket.villager_bucket", "pending"]
        );
        assert_eq!(parse_answer(b"ok").unwrap(), Vec::<String>::new());
        assert_eq!(parse_answer(b"nope").unwrap_err().kind, ErrorKind::Other);
        for (text, kind) in [
            ("unavailable: not in the pack", ErrorKind::Unavailable),
            ("invalid: bad", ErrorKind::Invalid),
            ("unknown: no model", ErrorKind::Unknown),
            ("limit: too big", ErrorKind::Limit),
            ("full: no room", ErrorKind::Full),
            ("conflict: taken", ErrorKind::Conflict),
            ("strange", ErrorKind::Other),
        ] {
            assert_eq!(error_from(text).kind, kind, "{text}");
            assert_eq!(error_from(text).detail, text);
        }
    }

    const TEXTS: Texts<'static> = Texts {
        en: "Hum",
        fr: "F",
        es: "E",
        tr: "T",
    };

    #[test]
    fn retexture_requests_name_the_block_and_the_faces() {
        assert_eq!(
            retexture_request("yellow_wool", &Faces::All(b"PNG")).unwrap(),
            b"retexture yellow_wool\n\nall.png 3\nPNG"
        );
        assert_eq!(
            retexture_request(
                "yellow_wool",
                &Faces::Sides {
                    up: b"U",
                    down: b"D",
                    side: b"S"
                }
            )
            .unwrap(),
            b"retexture yellow_wool\n\nup.png 1\nUdown.png 1\nDside.png 1\nS"
        );
        assert_eq!(
            retexture_request("a b", &Faces::All(b"")).unwrap_err().kind,
            ErrorKind::Invalid
        );
    }

    #[test]
    fn sound_requests_carry_their_settings() {
        let request = sound_request(&Sound {
            name: "level0",
            ogg: b"OGG",
            stream: true,
            category: "ambient",
            subtitle: Some(TEXTS),
            positional: false,
            max_distance: Some(64),
        })
        .unwrap();
        assert_eq!(
            String::from_utf8(request).unwrap(),
            "sound level0\ncategory=ambient\nstream=true\npositional=false\nmax_distance=64\nsubtitle.en=Hum\nsubtitle.fr=F\nsubtitle.es=E\nsubtitle.tr=T\n\nsound.ogg 3\nOGG"
        );
    }

    #[test]
    fn line_requests_cannot_smuggle_settings() {
        assert_eq!(
            String::from_utf8(line_request("card", &TEXTS).unwrap()).unwrap(),
            "line card\ntext.en=Hum\ntext.fr=F\ntext.es=E\ntext.tr=T\n\n"
        );
        let bad = Texts {
            fr: "a\ntext.en=b",
            ..TEXTS
        };
        assert_eq!(
            line_request("card", &bad).unwrap_err().kind,
            ErrorKind::Invalid
        );
        assert_eq!(
            line_request("a=b", &TEXTS).unwrap_err().kind,
            ErrorKind::Invalid
        );
    }
}
