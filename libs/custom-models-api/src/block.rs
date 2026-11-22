//! Custom blocks: a solid full block with its own look, placed with its item.
//!
//! ```ignore
//! use custom_models_api::{Block, BlockDrop, BlockFeel, BlockLook, BlockShape, BlockTool, block};
//!
//! const RUBY_ORE: Block<'static> = Block {
//!     name: "ruby_ore",
//!     names: &[("en", "Ruby Ore"), ("tr", "Yakut Cevheri")],
//!     stack: None,
//!     drop: BlockDrop::Itself,
//!     notify: false,
//!     look: BlockLook::All(include_bytes!("../blocks/ruby_ore.png")),
//!     feel: BlockFeel {
//!         hardness: Some(3.0),
//!         tool: Some(BlockTool::Pickaxe),
//!         requires_tool: Some(true),
//!         ..BlockFeel::default()
//!     },
//!     shape: BlockShape::Cube,
//!     states: &[],
//!     facing: false,
//! };
//!
//! // in on_load, with "custom-models" in the metadata's dependencies:
//! let ore = block(&RUBY_ORE)?; // ore.id == "<your plugin>.ruby_ore", the block and its item
//! ```
//!
//! The block's item places it as any block item does; a broken block drops its item (or nothing
//! with [`BlockDrop::Nothing`]), once the break is final, and an explosion drops the same, with
//! the explosion's decay as for any block; a `drops` rule from the block
//! ([`crate::Drops::my_block`]) adds to that or replaces it, and its experience drops for a
//! player's break. Pick block gives the block's item. A Bedrock player sees a note block. Register
//! your blocks in every `on_load`: a block keeps its place in the world, and the same name gets
//! the same block back.
//!
//! **Feel** ([`BlockFeel`]): every line you set is checked (out of range refuses the request),
//! every line you leave out is a note block's (a passable block's: string's). Hitting, stepping on
//! and breaking sound like wood, the place sound is yours or a vanilla block's, and the block
//! gives no light. A block with no lines at all keeps the server's default.
//!
//! **Shape and states**: [`BlockShape::Passable`] is a block without collision (a plant, a
//! crop) and uses the 96 passable states of the server, shared by every plugin; a request that
//! cannot get its states answers [`crate::ErrorKind::Full`]. `states` are the looks of state 1
//! and up (the block's `look` is state 0, the item's look); `facing` makes four states (so `states`
//! has three looks) and turns the block to face the player who places it: state 0 has its front
//! to the north, 1 east, 2 south, 3 west. Change a state with [`block_state`]: only a block
//! of yours that is already there.
//!
//! With `notify`, custom-models asks your plugin before a player places, breaks or right-clicks
//! one of these blocks: [`block_notice`] reads the message in your `handle_ipc_message`, and you
//! answer [`ALLOW`] or [`DENY`]. No answer (or a failure) refuses a place and allows a break or a
//! click. Answer at once, and do not call custom-models while you answer.

use super::{Error, Registered, ask, field, invalid, with_files};

/// The look of a block.
#[derive(Debug, Clone, Copy)]
pub enum BlockLook<'a> {
    /// One picture on every face (square, a power of two up to 512 pixels).
    All(&'a [u8]),
    /// The top, the bottom and the four sides.
    Sides {
        top: &'a [u8],
        bottom: &'a [u8],
        side: &'a [u8],
    },
    /// A Blockbench "Java Block/Item" model and its textures by name; a full cube looks right,
    /// since the block collides and hides its neighbours' faces as one.
    Model {
        json: &'a [u8],
        textures: &'a [(&'a str, &'a [u8])],
    },
}

/// What a broken block drops on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockDrop {
    /// One of its own item (silk touch and fortune change nothing).
    Itself,
    /// Nothing; a `drops` rule may give something.
    Nothing,
}

/// The family of vanilla states a block is drawn on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockShape {
    /// A full solid cube.
    Cube,
    /// No collision, any model inside its cell: plants, crops, rugs. 96 states per server for
    /// every plugin together.
    Passable,
}

/// The tool that mines a block at full speed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockTool {
    Pickaxe,
    Axe,
    Shovel,
    Hoe,
    /// No tool is faster than a hand.
    None,
}

/// The lowest level of the tool that gets a block's drops (a pickaxe's).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockTier {
    Any,
    Stone,
    Iron,
    Diamond,
}

/// The sound a block makes when it is placed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockSound<'a> {
    /// One of your own sounds (its `sound` request's name).
    Own(&'a str),
    /// A vanilla block's place sound, `minecraft:stone`.
    Like(&'a str),
}

/// How a block feels. A `None` is left out of the request: a note block's value (a passable
/// block's: string's). All `None` (the default) sends no line, and the server keeps its default
/// feel.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BlockFeel<'a> {
    /// Destroy time as vanilla's, 0 to 1000 (stone 1.5, obsidian 50), or -1: it cannot be broken
    /// outside creative. A passable block's is 0 or -1.
    pub hardness: Option<f32>,
    /// Explosion resistance as vanilla's, 0 to 3,600,000 (stone 6).
    pub resistance: Option<f32>,
    pub tool: Option<BlockTool>,
    /// A pickaxe's tool level; needs [`BlockTool::Pickaxe`].
    pub tier: Option<BlockTier>,
    /// Only the right tool (and level) gets the drops; needs a tool.
    pub requires_tool: Option<bool>,
    pub sound: Option<BlockSound<'a>>,
    /// Pistons move it.
    pub pushable: Option<bool>,
}

/// The look of one state after the first.
#[derive(Debug, Clone, Copy)]
pub enum StateLook<'a> {
    /// One picture on every face (square, a power of two up to 512 pixels).
    All(&'a [u8]),
    /// A Blockbench model; its textures by name (a name that several looks use is sent once, and
    /// must be the same picture).
    Model {
        json: &'a [u8],
        textures: &'a [(&'a str, &'a [u8])],
    },
}

/// A custom block of your plugin. Its id is `<your plugin>.<name>`, and the server takes at most 48
/// characters for it.
#[derive(Debug, Clone, Copy)]
pub struct Block<'a> {
    /// `[a-z0-9_]`.
    pub name: &'a str,
    /// The item's name by language (`en` required; `fr`, `es`, `tr`).
    pub names: &'a [(&'a str, &'a str)],
    /// The item's stack size, 1 to 99; 64 when `None`.
    pub stack: Option<u8>,
    pub drop: BlockDrop,
    /// Ask your plugin before a place, a break or a right click ([`block_notice`]).
    pub notify: bool,
    /// State 0: the item's look.
    pub look: BlockLook<'a>,
    pub feel: BlockFeel<'a>,
    pub shape: BlockShape,
    /// The looks of state 1 and up: `states.len() + 1` states, at most 32.
    pub states: &'a [StateLook<'a>],
    /// Four states, turned to face the player who places the block: `states` has three looks.
    pub facing: bool,
}

fn float_line(settings: &mut String, key: &str, value: Option<f32>) -> Result<(), Error> {
    if let Some(v) = value {
        if !v.is_finite() {
            return Err(invalid(format!("{key} is a number")));
        }
        settings.push_str(&format!("{key}={v}\n"));
    }
    Ok(())
}

fn bool_line(settings: &mut String, key: &str, value: Option<bool>) {
    if let Some(v) = value {
        settings.push_str(&format!("{key}={v}\n"));
    }
}

/// The settings lines of a block's feel.
fn feel_lines(settings: &mut String, feel: &BlockFeel<'_>) -> Result<(), Error> {
    float_line(settings, "hardness", feel.hardness)?;
    float_line(settings, "resistance", feel.resistance)?;
    if let Some(tool) = feel.tool {
        let word = match tool {
            BlockTool::Pickaxe => "pickaxe",
            BlockTool::Axe => "axe",
            BlockTool::Shovel => "shovel",
            BlockTool::Hoe => "hoe",
            BlockTool::None => "none",
        };
        settings.push_str(&format!("tool={word}\n"));
    }
    if let Some(tier) = feel.tier {
        let word = match tier {
            BlockTier::Any => "any",
            BlockTier::Stone => "stone",
            BlockTier::Iron => "iron",
            BlockTier::Diamond => "diamond",
        };
        settings.push_str(&format!("tier={word}\n"));
    }
    bool_line(settings, "requires_tool", feel.requires_tool);
    match feel.sound {
        Some(BlockSound::Own(name)) | Some(BlockSound::Like(name))
            if name.is_empty() || name.contains(['\n', '\r', '=', ' ']) =>
        {
            return Err(invalid("a sound is one word"));
        }
        Some(BlockSound::Own(name)) => settings.push_str(&format!("sound={name}\n")),
        Some(BlockSound::Like(block)) => settings.push_str(&format!("sounds_like={block}\n")),
        None => {}
    }
    bool_line(settings, "pushable", feel.pushable);
    Ok(())
}

/// A word a request line can carry: not empty, no line break, `=` or space.
fn line_safe(s: &str) -> bool {
    !s.is_empty() && !s.contains(['\n', '\r', '=', ' '])
}

// A function and not a closure: the names and bytes live as long as the block, not as long as
// one call.
fn add_textures<'a>(
    held: &mut Vec<(&'a str, &'a [u8])>,
    list: &'a [(&'a str, &'a [u8])],
) -> Result<(), Error> {
    for (name, bytes) in list {
        if !line_safe(name) || name.contains('/') {
            return Err(invalid("a texture name is one word"));
        }
        match held.iter().find(|(t, _)| t == name) {
            Some((_, kept)) if kept != bytes => {
                return Err(invalid(format!(
                    "the texture {name} is two different pictures"
                )));
            }
            Some(_) => {}
            None => held.push((*name, *bytes)),
        }
    }
    Ok(())
}

/// The bytes of a `block` request for `block`.
pub fn block_request(block: &Block<'_>) -> Result<Vec<u8>, Error> {
    if !line_safe(block.name) {
        return Err(invalid("a block name is one word"));
    }
    let mut settings = format!("block {}\n", block.name);
    for (language, text) in block.names {
        if !line_safe(language) || text.contains(['\n', '\r']) {
            return Err(invalid("a name has no line breaks"));
        }
        settings.push_str(&format!("name.{language}={text}\n"));
    }
    if let Some(stack) = block.stack {
        settings.push_str(&format!("stack={stack}\n"));
    }
    if block.drop == BlockDrop::Nothing {
        settings.push_str("drop=none\n");
    }
    if block.notify {
        settings.push_str("notify=true\n");
    }
    feel_lines(&mut settings, &block.feel)?;
    if block.shape == BlockShape::Passable {
        settings.push_str("shape=passable\n");
    }
    if block.facing {
        if block.states.len() != 3 {
            return Err(invalid(
                "a block that faces has four states: three more looks",
            ));
        }
        settings.push_str("facing=horizontal\n");
    } else if !block.states.is_empty() {
        if block.states.len() >= 32 {
            return Err(invalid("a block has at most 32 states"));
        }
        settings.push_str(&format!("states={}\n", block.states.len() + 1));
    }
    // The files: the first look, the textures every model uses once, then the other states.
    let mut files: Vec<(String, &[u8])> = Vec::new();
    let mut held: Vec<(&str, &[u8])> = Vec::new();
    match block.look {
        BlockLook::All(png) => files.push(("all.png".to_owned(), png)),
        BlockLook::Sides { top, bottom, side } => {
            files.push(("top.png".to_owned(), top));
            files.push(("bottom.png".to_owned(), bottom));
            files.push(("side.png".to_owned(), side));
        }
        BlockLook::Model { json, textures } => {
            files.push(("model.json".to_owned(), json));
            add_textures(&mut held, textures)?;
        }
    }
    for look in block.states {
        if let StateLook::Model { textures, .. } = look {
            add_textures(&mut held, textures)?;
        }
    }
    for (name, bytes) in &held {
        files.push((format!("textures/{name}.png"), *bytes));
    }
    for (k, look) in block.states.iter().enumerate() {
        let k = k + 1;
        match look {
            StateLook::All(png) => files.push((format!("state.{k}.png"), *png)),
            StateLook::Model { json, .. } => files.push((format!("state.{k}.json"), *json)),
        }
    }
    let files: Vec<(&str, &[u8])> = files.iter().map(|(p, b)| (p.as_str(), *b)).collect();
    // `with_files` adds the empty line that ends the settings.
    Ok(with_files(settings, &files))
}

/// Registers (or replaces) one of your blocks and its item. Call it in `on_load`.
pub fn block(block: &Block<'_>) -> Result<Registered, Error> {
    let fields = ask(&block_request(block)?)?;
    Ok(Registered {
        id: field(&fields, 0)?.to_owned(),
        ready: field(&fields, 1)? == "ready",
    })
}

/// The bytes of a `block-state` request.
pub fn block_state_request(
    block: &str,
    world: &str,
    pos: (i32, i32, i32),
    state: u16,
) -> Result<Vec<u8>, Error> {
    let word = |s: &str| !s.is_empty() && !s.contains(char::is_whitespace);
    if !word(block) || block.contains('=') {
        return Err(invalid("a block name is one word"));
    }
    if !word(world) {
        return Err(invalid("a world name is one word"));
    }
    Ok(format!(
        "block-state {block} {world} {} {} {} {state}",
        pos.0, pos.1, pos.2
    )
    .into_bytes())
}

/// Changes the state of your block `block` (its name, not its id) at `pos` in `world`: only a block
/// of yours that is already there, to a state it has; its chunk must be loaded. A crop grows with
/// it, a block turns.
pub fn block_state(
    block: &str,
    world: &str,
    pos: (i32, i32, i32),
    state: u16,
) -> Result<(), Error> {
    ask(&block_state_request(block, world, pos, state)?).map(|_| ())
}

/// The answer that lets it happen.
pub const ALLOW: &[u8] = b"allow";
/// The answer that refuses it.
pub const DENY: &[u8] = b"deny";

/// What a notice is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeKind {
    Place,
    Break,
    /// A right click on the block.
    Use,
}

/// A notice about one of your blocks (`notify`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockNotice {
    pub kind: NoticeKind,
    /// The block's id, `<your plugin>.<name>`.
    pub block: String,
    /// The player's uuid, when a player did it.
    pub player: Option<String>,
    pub world: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// The face clicked on a use: `up`, `down`, `north`, `south`, `west` or `east`.
    pub face: Option<String>,
}

/// Reads a notice from `handle_ipc_message`: `None` when the message is not one, or does not come
/// from custom-models (`from`, as the server names the sender).
#[must_use]
pub fn block_notice(from: &str, message: &[u8]) -> Option<BlockNotice> {
    if from != super::PLUGIN {
        return None;
    }
    let text = std::str::from_utf8(message).ok()?;
    let f: Vec<&str> = text.split('\t').collect();
    if f.len() != 9 || f[0] != "block-notice" {
        return None;
    }
    let kind = match f[1] {
        "place" => NoticeKind::Place,
        "break" => NoticeKind::Break,
        "use" => NoticeKind::Use,
        _ => return None,
    };
    let optional = |s: &str| (s != "-").then(|| s.to_owned());
    Some(BlockNotice {
        kind,
        block: f[2].to_owned(),
        player: optional(f[3]),
        world: f[4].to_owned(),
        x: f[5].parse().ok()?,
        y: f[6].parse().ok()?,
        z: f[7].parse().ok()?,
        face: optional(f[8]),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = b"png";

    fn text(b: &Block<'_>) -> String {
        String::from_utf8(block_request(b).unwrap()).unwrap()
    }

    #[test]
    fn block_requests_carry_their_settings_and_files() {
        let mut ore = Block {
            name: "ruby_ore",
            names: &[("en", "Ruby Ore"), ("tr", "Yakut Cevheri")],
            stack: None,
            drop: BlockDrop::Itself,
            notify: false,
            look: BlockLook::All(PNG),
            feel: BlockFeel::default(),
            shape: BlockShape::Cube,
            states: &[],
            facing: false,
        };
        assert_eq!(
            text(&ore),
            "block ruby_ore\nname.en=Ruby Ore\nname.tr=Yakut Cevheri\n\nall.png 3\npng"
        );
        ore.stack = Some(16);
        ore.drop = BlockDrop::Nothing;
        ore.notify = true;
        ore.look = BlockLook::Sides {
            top: PNG,
            bottom: PNG,
            side: PNG,
        };
        assert_eq!(
            text(&ore),
            "block ruby_ore\nname.en=Ruby Ore\nname.tr=Yakut Cevheri\nstack=16\ndrop=none\nnotify=true\n\ntop.png 3\npngbottom.png 3\npngside.png 3\npng"
        );
        ore.look = BlockLook::Model {
            json: b"{}",
            textures: &[("ore", PNG)],
        };
        assert!(text(&ore).ends_with("\n\nmodel.json 2\n{}textures/ore.png 3\npng"));
    }

    #[test]
    fn line_breaks_cannot_add_settings_to_a_block() {
        let ok = Block {
            name: "a",
            names: &[("en", "A")],
            stack: None,
            drop: BlockDrop::Itself,
            notify: false,
            look: BlockLook::All(PNG),
            feel: BlockFeel::default(),
            shape: BlockShape::Cube,
            states: &[],
            facing: false,
        };
        let bad = [
            Block {
                name: "a\nnotify=true",
                ..ok
            },
            Block { name: "a b", ..ok },
            Block {
                names: &[("en", "A\ndrop=none")],
                ..ok
            },
            Block {
                names: &[("en=x", "A")],
                ..ok
            },
            Block {
                look: BlockLook::Model {
                    json: b"{}",
                    textures: &[("../x", PNG)],
                },
                ..ok
            },
        ];
        for b in &bad {
            assert_eq!(
                block_request(b).unwrap_err().kind,
                crate::ErrorKind::Invalid,
                "{b:?}"
            );
        }
    }

    #[test]
    fn notices_are_read_only_from_custom_models() {
        let message = b"block-notice\tuse\tgems.ruby_ore\t8667ba71-b85a-4004-af54-457a9734eed7\tworld\t1\t-60\t3\tup";
        let notice = block_notice("custom-models", message).unwrap();
        assert_eq!(notice.kind, NoticeKind::Use);
        assert_eq!(notice.block, "gems.ruby_ore");
        assert_eq!((notice.x, notice.y, notice.z), (1, -60, 3));
        assert_eq!(notice.face.as_deref(), Some("up"));
        assert!(block_notice("someone-else", message).is_none());
        let place = block_notice(
            "custom-models",
            b"block-notice\tbreak\tgems.ruby_ore\t-\tworld\t1\t2\t3\t-",
        )
        .unwrap();
        assert_eq!(
            (place.kind, place.player, place.face),
            (NoticeKind::Break, None, None)
        );
        for bad in [
            &b"block-notice\tjump\tx\t-\tw\t1\t2\t3\t-"[..],
            b"block-notice\tplace\tx\t-\tw\t1\t2\t3",
            b"block-notice\tplace\tx\t-\tw\ta\t2\t3\t-",
            b"other\tplace\tx\t-\tw\t1\t2\t3\t-",
        ] {
            assert!(block_notice("custom-models", bad).is_none());
        }
    }

    fn plain<'a>() -> Block<'a> {
        Block {
            name: "crop",
            names: &[("en", "Crop")],
            stack: None,
            drop: BlockDrop::Itself,
            notify: false,
            look: BlockLook::All(PNG),
            feel: BlockFeel::default(),
            shape: BlockShape::Cube,
            states: &[],
            facing: false,
        }
    }

    #[test]
    fn a_blocks_feel_shape_and_states_are_lines_of_the_request() {
        // Nothing set: no line, the server's default feel.
        assert_eq!(text(&plain()), "block crop\nname.en=Crop\n\nall.png 3\npng");
        let marble = Block {
            feel: BlockFeel {
                hardness: Some(1.5),
                resistance: Some(6.0),
                tool: Some(BlockTool::Pickaxe),
                tier: Some(BlockTier::Stone),
                requires_tool: Some(true),
                sound: Some(BlockSound::Like("minecraft:stone")),
                pushable: Some(false),
            },
            ..plain()
        };
        assert_eq!(
            text(&marble),
            "block crop\nname.en=Crop\nhardness=1.5\nresistance=6\ntool=pickaxe\ntier=stone\nrequires_tool=true\nsounds_like=minecraft:stone\npushable=false\n\nall.png 3\npng"
        );
        let own = Block {
            feel: BlockFeel {
                tool: Some(BlockTool::None),
                sound: Some(BlockSound::Own("hum")),
                hardness: Some(-1.0),
                ..BlockFeel::default()
            },
            ..plain()
        };
        assert_eq!(
            text(&own),
            "block crop\nname.en=Crop\nhardness=-1\ntool=none\nsound=hum\n\nall.png 3\npng"
        );
        // Three more looks, and a texture two of them use is sent once.
        let looks = [
            StateLook::All(b"one"),
            StateLook::Model {
                json: b"{1}",
                textures: &[("stem", b"s"), ("leaf", b"l")],
            },
            StateLook::Model {
                json: b"{2}",
                textures: &[("stem", b"s")],
            },
        ];
        let crop = Block {
            shape: BlockShape::Passable,
            states: &looks,
            ..plain()
        };
        assert_eq!(
            text(&crop),
            "block crop\nname.en=Crop\nshape=passable\nstates=4\n\nall.png 3\npngtextures/stem.png 1\nstextures/leaf.png 1\nlstate.1.png 3\nonestate.2.json 3\n{1}state.3.json 3\n{2}"
        );
        let chair = Block {
            facing: true,
            states: &[
                StateLook::All(b"1"),
                StateLook::All(b"2"),
                StateLook::All(b"3"),
            ],
            ..plain()
        };
        assert!(text(&chair).contains("\nfacing=horizontal\n\n"));
        assert!(!text(&chair).contains("states="));
    }

    #[test]
    fn a_wrong_feel_shape_or_state_never_leaves_the_plugin() {
        let bad = [
            Block {
                feel: BlockFeel {
                    hardness: Some(f32::NAN),
                    ..BlockFeel::default()
                },
                ..plain()
            },
            Block {
                feel: BlockFeel {
                    resistance: Some(f32::INFINITY),
                    ..BlockFeel::default()
                },
                ..plain()
            },
            Block {
                feel: BlockFeel {
                    sound: Some(BlockSound::Own("hum\npushable=true")),
                    ..BlockFeel::default()
                },
                ..plain()
            },
            Block {
                feel: BlockFeel {
                    sound: Some(BlockSound::Like("")),
                    ..BlockFeel::default()
                },
                ..plain()
            },
            // A block that faces has four states.
            Block {
                facing: true,
                ..plain()
            },
            Block {
                facing: true,
                states: &[StateLook::All(b"1")],
                ..plain()
            },
            // Two pictures under one texture name.
            Block {
                states: &[
                    StateLook::Model {
                        json: b"{}",
                        textures: &[("stem", b"a")],
                    },
                    StateLook::Model {
                        json: b"{}",
                        textures: &[("stem", b"b")],
                    },
                ],
                ..plain()
            },
            Block {
                states: &[StateLook::Model {
                    json: b"{}",
                    textures: &[("../x", b"a")],
                }],
                ..plain()
            },
        ];
        for b in &bad {
            assert_eq!(
                block_request(b).unwrap_err().kind,
                crate::ErrorKind::Invalid,
                "{b:?}"
            );
        }
        // More than 32 states.
        let many = [StateLook::All(b"x"); 32];
        let b = Block {
            states: &many,
            ..plain()
        };
        assert_eq!(
            block_request(&b).unwrap_err().kind,
            crate::ErrorKind::Invalid
        );
    }

    #[test]
    fn a_state_change_is_one_line_of_words() {
        assert_eq!(
            block_state_request("chair", "world", (1, 64, -2), 3).unwrap(),
            b"block-state chair world 1 64 -2 3"
        );
        for (block, world) in [
            ("", "world"),
            ("a b", "world"),
            ("a\nstate", "world"),
            ("a=b", "world"),
            ("chair", ""),
            ("chair", "my world"),
            ("chair", "w\n1 2 3"),
        ] {
            assert_eq!(
                block_state_request(block, world, (0, 0, 0), 0)
                    .unwrap_err()
                    .kind,
                crate::ErrorKind::Invalid,
                "{block:?} {world:?}"
            );
        }
    }
}
