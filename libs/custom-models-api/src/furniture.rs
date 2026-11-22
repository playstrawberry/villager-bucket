//! Furniture: one of your models placed in the world with its item, with hitboxes, seats, barrier
//! and light blocks and named looks.
//!
//! ```ignore
//! use custom_models_api::{Furniture, HitBox, Place, furniture, furniture_state};
//!
//! // in on_load, after registering the models it names:
//! furniture(&Furniture::new("chair", "chair").seats(&[[0.0, 0.4, 0.0]]))?;
//! furniture(
//!     &Furniture::new("lamp", "lamp")
//!         .places(&[Place::Floor, Place::Ceiling])
//!         .solid(&[[0, 0, 0]])
//!         .light(14, [0, 1, 0])
//!         .states(&[("off", "lamp"), ("on", "lamp_lit")])
//!         .notify(),
//! )?;
//!
//! // in handle_ipc_message:
//! if let Some(n) = custom_models_api::furniture_notice(&from, &payload) {
//!     if n.kind == FurnitureNoticeKind::Click && n.furniture == "lamp" {
//!         let next = if n.state == "on" { "off" } else { "on" };
//!         let _ = furniture_state(&n.piece, next);
//!     }
//!     return Ok(custom_models_api::ALLOW.to_vec());
//! }
//! ```
//!
//! A piece with [`Furniture::storage`] opens a chest window when it is used, and what lies in it
//! stays with the piece (and drops when the piece breaks). One player has it open at a time.
//!
//! A piece with [`Furniture::trades`] opens the server's trading window when it is used: a trade
//! machine. The server does the trade (it takes the costs and gives the result); there is no
//! stock and no restock, so every offer can be traded as often as the player pays for it.
//!
//! ```ignore
//! use custom_models_api::{Furniture, Trade, TradeItem};
//!
//! furniture(&Furniture::new("shop", "shop").trades(&[
//!     Trade::new(TradeItem::vanilla("minecraft:emerald", 3), TradeItem::model("coin", 1)),
//!     Trade::new(TradeItem::vanilla("minecraft:iron_ingot", 8), TradeItem::vanilla("minecraft:diamond", 1))
//!         .and(TradeItem::model("coin", 1)),
//! ]))?;
//! ```
//!
//! A piece has one use action: storage, trades and seats do not go together.
//!
//! A piece is told to its script only with [`Furniture::notify`]: place, break, click, sit and
//! stand, before it happens. Place and sit happen only when you answer [`ALLOW`]; break and stand
//! happen unless you answer [`DENY`], so a script that fails or is gone never makes a piece
//! unbreakable or keeps a player in a seat. A stand because the player left, died or was moved,
//! or because the piece broke, is told with `forced` and cannot be refused.

#[cfg(doc)]
use super::{ALLOW, DENY};
use super::{Error, PLUGIN, Registered, ask, field, invalid};

/// Where a piece may go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    Floor,
    Wall,
    Ceiling,
}

impl Place {
    const fn word(self) -> &'static str {
        match self {
            Self::Floor => "floor",
            Self::Wall => "wall",
            Self::Ceiling => "ceiling",
        }
    }
}

/// The steps a piece on the floor or the ceiling turns in, to face the player who places it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Turn {
    Four,
    Eight,
    Sixteen,
    Free,
}

impl Turn {
    const fn word(self) -> &'static str {
        match self {
            Self::Four => "4",
            Self::Eight => "8",
            Self::Sixteen => "16",
            Self::Free => "free",
        }
    }
}

/// An extra hitbox: an offset in blocks in the piece's frame (`+z` is its front), a width and a
/// height (0.25 to 4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitBox {
    pub offset: [f32; 3],
    pub width: f32,
    pub height: f32,
}

/// An item of a trade, with how many (1 to 99, within the item's stack size).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeItem<'a> {
    /// A vanilla item by its id with namespace: `minecraft:emerald`.
    Vanilla(&'a str, u8),
    /// One of your own models, by its name.
    Model(&'a str, u8),
}

impl<'a> TradeItem<'a> {
    #[must_use]
    pub const fn vanilla(id: &'a str, count: u8) -> Self {
        Self::Vanilla(id, count)
    }

    #[must_use]
    pub const fn model(name: &'a str, count: u8) -> Self {
        Self::Model(name, count)
    }

    fn text(self) -> Result<String, Error> {
        let (id, count) = match self {
            Self::Vanilla(id, count) => {
                let ok = id.strip_prefix("minecraft:").is_some_and(|n| {
                    !n.is_empty()
                        && n.len() <= 53
                        && n.chars().all(|c| {
                            c.is_ascii_lowercase()
                                || c.is_ascii_digit()
                                || matches!(c, '_' | '.' | '-' | '/')
                        })
                });
                if !ok {
                    return Err(invalid(
                        "a vanilla item of a trade is `minecraft:<name>`, lowercase".to_owned(),
                    ));
                }
                (id, count)
            }
            Self::Model(name_, count) => {
                name(name_, "a trade's model")?;
                (name_, count)
            }
        };
        if !(1..=99).contains(&count) {
            return Err(invalid("a trade's count is 1 to 99".to_owned()));
        }
        Ok(format!("{id} {count}"))
    }
}

/// One trade of a trade machine: a cost (and perhaps a second one) for a result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Trade<'a> {
    pub cost: TradeItem<'a>,
    pub second_cost: Option<TradeItem<'a>>,
    pub result: TradeItem<'a>,
}

impl<'a> Trade<'a> {
    /// `cost` for `result`.
    #[must_use]
    pub const fn new(cost: TradeItem<'a>, result: TradeItem<'a>) -> Self {
        Self {
            cost,
            second_cost: None,
            result,
        }
    }

    /// A second cost, paid as well.
    #[must_use]
    pub const fn and(mut self, second: TradeItem<'a>) -> Self {
        self.second_cost = Some(second);
        self
    }

    fn line(&self) -> Result<String, Error> {
        let mut out = self.cost.text()?;
        if let Some(second) = self.second_cost {
            out.push_str(" + ");
            out.push_str(&second.text()?);
        }
        out.push_str(" -> ");
        out.push_str(&self.result.text()?);
        Ok(out)
    }
}

/// Trades per piece.
pub const MAX_TRADES: usize = 16;

/// A piece of furniture of your script. Its id is `<your script>.<name>`; the same name again
/// replaces it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Furniture<'a> {
    /// `[a-z0-9_]`, 1 to 48 characters.
    pub name: &'a str,
    /// One of your models, by its name: its item places the piece and drops from it.
    pub model: &'a str,
    /// Floor only when empty.
    pub places: &'a [Place],
    pub turn: Turn,
    /// 0.25 to 4.
    pub scale: f32,
    /// The look's offset in blocks, -4 to 4 each.
    pub offset: [f32; 3],
    /// The hitbox at the piece's foot: width and height, 0.25 to 4.
    pub size: (f32, f32),
    /// Up to 3 more hitboxes.
    pub boxes: &'a [HitBox],
    /// Cells (-3 to 3 from the piece's own) that get barrier blocks; not solid when empty.
    pub solid: &'a [[i8; 3]],
    /// A light block: its level (1 to 15) and its cell, which is not a solid cell.
    pub light: Option<(u8, [i8; 3])>,
    /// Up to 4 seats, offsets in blocks.
    pub seats: &'a [[f32; 3]],
    /// Up to 8 named looks, each one of your models; the first is the look a new piece has.
    pub states: &'a [(&'a str, &'a str)],
    /// The rows (1 to 6) of the chest window a use of the piece opens; none when `None`. A piece
    /// with storage has no seats.
    pub storage: Option<u8>,
    /// Up to 16 trades: a use opens the trading window. A piece with trades has no storage and
    /// no seats.
    pub trades: &'a [Trade<'a>],
    pub drop: bool,
    pub notify: bool,
    /// One of your sounds, by its name; vanilla's wooden sound when `None`.
    pub place_sound: Option<&'a str>,
    pub break_sound: Option<&'a str>,
    /// Up to 4 particle emitters of the piece (`Emit`): they give off particles while a player is
    /// near, from the piece's look.
    pub emit: &'a [crate::Emit<'a>],
}

impl<'a> Furniture<'a> {
    /// A floor piece of `model`, turning in 8 steps, with a 1 by 1 hitbox, that drops its item.
    #[must_use]
    pub const fn new(name: &'a str, model: &'a str) -> Self {
        Self {
            name,
            model,
            places: &[],
            turn: Turn::Eight,
            scale: 1.0,
            offset: [0.0; 3],
            size: (1.0, 1.0),
            boxes: &[],
            solid: &[],
            light: None,
            seats: &[],
            states: &[],
            storage: None,
            trades: &[],
            drop: true,
            notify: false,
            place_sound: None,
            break_sound: None,
            emit: &[],
        }
    }

    #[must_use]
    pub const fn places(mut self, places: &'a [Place]) -> Self {
        self.places = places;
        self
    }

    #[must_use]
    pub const fn turn(mut self, turn: Turn) -> Self {
        self.turn = turn;
        self
    }

    #[must_use]
    pub const fn scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }

    #[must_use]
    pub const fn offset(mut self, offset: [f32; 3]) -> Self {
        self.offset = offset;
        self
    }

    #[must_use]
    pub const fn size(mut self, width: f32, height: f32) -> Self {
        self.size = (width, height);
        self
    }

    #[must_use]
    pub const fn boxes(mut self, boxes: &'a [HitBox]) -> Self {
        self.boxes = boxes;
        self
    }

    #[must_use]
    pub const fn solid(mut self, cells: &'a [[i8; 3]]) -> Self {
        self.solid = cells;
        self
    }

    #[must_use]
    pub const fn light(mut self, level: u8, at: [i8; 3]) -> Self {
        self.light = Some((level, at));
        self
    }

    #[must_use]
    pub const fn seats(mut self, seats: &'a [[f32; 3]]) -> Self {
        self.seats = seats;
        self
    }

    #[must_use]
    pub const fn states(mut self, states: &'a [(&'a str, &'a str)]) -> Self {
        self.states = states;
        self
    }

    /// A use of the piece opens a chest window of `rows` rows (1 to 6); what lies in it stays
    /// with the piece, across unloads and restarts, and drops when the piece breaks.
    #[must_use]
    pub const fn storage(mut self, rows: u8) -> Self {
        self.storage = Some(rows);
        self
    }

    /// A use of the piece opens the server's trading window with these trades (up to 16): a
    /// trade machine. The server does the trading; there is no stock.
    #[must_use]
    pub const fn trades(mut self, trades: &'a [Trade<'a>]) -> Self {
        self.trades = trades;
        self
    }

    /// The broken piece drops nothing (its contents still drop).
    #[must_use]
    pub const fn no_drop(mut self) -> Self {
        self.drop = false;
        self
    }

    /// Your script is told about place, break, click, sit and stand (see the module).
    #[must_use]
    pub const fn notify(mut self) -> Self {
        self.notify = true;
        self
    }

    /// Particle emitters of the piece, at most 4 (see `Emit`).
    #[must_use]
    pub const fn emit(mut self, emit: &'a [crate::Emit<'a>]) -> Self {
        self.emit = emit;
        self
    }

    #[must_use]
    pub const fn sounds(mut self, place: Option<&'a str>, broken: Option<&'a str>) -> Self {
        self.place_sound = place;
        self.break_sound = broken;
        self
    }
}

/// One name of yours: `[a-z0-9_]`, 1 to 48 characters.
pub(crate) fn name(text: &str, what: &str) -> Result<(), Error> {
    let ok = !text.is_empty()
        && text.len() <= 48
        && text
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if ok {
        Ok(())
    } else {
        Err(invalid(format!("{what} is [a-z0-9_], 1 to 48 characters")))
    }
}

/// A number as the request writes it: at most three decimals, finite.
pub(crate) fn number(value: f32, what: &str) -> Result<String, Error> {
    if !value.is_finite() {
        return Err(invalid(format!("{what} is a finite number")));
    }
    let text = format!("{:.3}", (f64::from(value) * 1000.0).round() / 1000.0);
    let text = text.trim_end_matches('0').trim_end_matches('.');
    Ok(match text {
        "" | "-" | "-0" => "0".to_owned(),
        t => t.to_owned(),
    })
}

fn numbers(values: &[f32], what: &str) -> Result<String, Error> {
    Ok(values
        .iter()
        .map(|v| number(*v, what))
        .collect::<Result<Vec<_>, _>>()?
        .join(" "))
}

fn cells(cells: &[[i8; 3]]) -> String {
    cells
        .iter()
        .map(|c| format!("{} {} {}", c[0], c[1], c[2]))
        .collect::<Vec<_>>()
        .join(",")
}

/// The bytes of a `furniture` request for `f`.
pub fn furniture_request(f: &Furniture<'_>) -> Result<Vec<u8>, Error> {
    name(f.name, "a furniture name")?;
    name(f.model, "a model")?;
    let mut out = format!("furniture {}\nmodel={}\n", f.name, f.model);
    if !f.places.is_empty() {
        let words: Vec<&str> = f.places.iter().map(|p| p.word()).collect();
        out.push_str(&format!("place={}\n", words.join(",")));
    }
    if f.turn != Turn::Eight {
        out.push_str(&format!("turn={}\n", f.turn.word()));
    }
    out.push_str(&format!("scale={}\n", number(f.scale, "scale")?));
    out.push_str(&format!("offset={}\n", numbers(&f.offset, "offset")?));
    out.push_str(&format!(
        "size={}\n",
        numbers(&[f.size.0, f.size.1], "size")?
    ));
    for (n, b) in f.boxes.iter().enumerate() {
        let [x, y, z] = b.offset;
        out.push_str(&format!(
            "box.{}={}\n",
            n + 1,
            numbers(&[x, y, z, b.width, b.height], "a box")?
        ));
    }
    if !f.solid.is_empty() {
        out.push_str(&format!("solid=true\ncells={}\n", cells(f.solid)));
    }
    if let Some((level, at)) = f.light {
        out.push_str(&format!("light={level}\nlight.at={}\n", cells(&[at])));
    }
    for (n, s) in f.seats.iter().enumerate() {
        out.push_str(&format!("seat.{}={}\n", n + 1, numbers(s, "a seat")?));
    }
    for (state, model) in f.states {
        name(state, "a state")?;
        name(model, "a state's model")?;
        out.push_str(&format!("state.{state}={model}\n"));
    }
    if let Some(rows) = f.storage {
        if !(1..=6).contains(&rows) {
            return Err(invalid("storage is 1 to 6 rows".to_owned()));
        }
        out.push_str(&format!("storage={rows}\n"));
    }
    if f.trades.len() > MAX_TRADES {
        return Err(invalid(format!("a piece has at most {MAX_TRADES} trades")));
    }
    for (n, t) in f.trades.iter().enumerate() {
        out.push_str(&format!("trade.{}={}\n", n + 1, t.line()?));
    }
    if !f.drop {
        out.push_str("drop=false\n");
    }
    if f.notify {
        out.push_str("notify=true\n");
    }
    for (key, sound) in [
        ("sound.place", f.place_sound),
        ("sound.break", f.break_sound),
    ] {
        if let Some(sound) = sound {
            name(sound, "a sound")?;
            out.push_str(&format!("{key}={sound}\n"));
        }
    }
    if f.emit.len() > crate::emitter::MAX_PER_PIECE {
        return Err(invalid("a piece has at most 4 emitters"));
    }
    for (n, e) in f.emit.iter().enumerate() {
        out.push_str(&format!("emit.{}={}\n", n + 1, e.spec()?));
    }
    Ok(out.into_bytes())
}

/// Registers (or replaces) one of your pieces of furniture. Call it in `on_load`, after the
/// models and sounds it names. Pieces already in the world work again once it is registered.
pub fn furniture(f: &Furniture<'_>) -> Result<Registered, Error> {
    let fields = ask(&furniture_request(f)?)?;
    Ok(Registered {
        id: field(&fields, 0)?.to_owned(),
        ready: field(&fields, 1)? == "ready",
    })
}

/// The bytes of a `furniture-state` request.
pub fn furniture_state_request(piece: &str, state: &str) -> Result<Vec<u8>, Error> {
    if piece.is_empty() || piece.len() > 128 || piece.contains(char::is_whitespace) {
        return Err(invalid("a piece id is one word"));
    }
    name(state, "a state")?;
    Ok(format!("furniture-state {piece} {state}").into_bytes())
}

/// Switches your piece `piece` (the id its notices carry) to its look `state`: one change of
/// the item it shows, kept on the piece. Its chunk must be loaded.
pub fn furniture_state(piece: &str, state: &str) -> Result<(), Error> {
    ask(&furniture_state_request(piece, state)?).map(|_| ())
}

/// What a notice is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FurnitureNoticeKind {
    Place,
    Break,
    Click,
    Sit,
    Stand,
}

/// A notice custom-models sent your script about one of its pieces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FurnitureNotice {
    pub kind: FurnitureNoticeKind,
    /// The piece's id, for [`furniture_state`].
    pub piece: String,
    /// Your furniture's name.
    pub furniture: String,
    /// The player's uuid; `None` when no player did it (its block went).
    pub player: Option<String>,
    /// The piece's state; empty without states.
    pub state: String,
    /// It cannot be refused (a stand because the player left, died or was moved, or the piece
    /// broke).
    pub forced: bool,
}

/// The notice in a message, when it is one: only a message from custom-models itself (`from`
/// as the server names the sender) is read, so no other script can fake one.
#[must_use]
pub fn furniture_notice(from: &str, payload: &[u8]) -> Option<FurnitureNotice> {
    if from != PLUGIN {
        return None;
    }
    let text = std::str::from_utf8(payload).ok()?;
    let f: Vec<&str> = text.split('\t').collect();
    let [head, kind, piece, furniture, player, state, forced] = f.as_slice() else {
        return None;
    };
    if *head != "furniture-notice" {
        return None;
    }
    let kind = match *kind {
        "place" => FurnitureNoticeKind::Place,
        "break" => FurnitureNoticeKind::Break,
        "click" => FurnitureNoticeKind::Click,
        "sit" => FurnitureNoticeKind::Sit,
        "stand" => FurnitureNoticeKind::Stand,
        _ => return None,
    };
    let forced = match *forced {
        "0" => false,
        "1" => true,
        _ => return None,
    };
    Some(FurnitureNotice {
        kind,
        piece: (*piece).to_owned(),
        furniture: (*furniture).to_owned(),
        player: (*player != "-").then(|| (*player).to_owned()),
        state: if *state == "-" {
            String::new()
        } else {
            (*state).to_owned()
        },
        forced,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_carries_every_setting() {
        let f = Furniture::new("sofa", "sofa")
            .places(&[Place::Floor, Place::Wall])
            .turn(Turn::Four)
            .scale(1.5)
            .offset([0.0, 0.25, -0.125])
            .size(2.0, 1.0)
            .boxes(&[HitBox {
                offset: [1.0, 0.0, 0.0],
                width: 1.0,
                height: 0.5,
            }])
            .solid(&[[0, 0, 0], [1, 0, 0]])
            .light(12, [0, 1, 0])
            .seats(&[[0.0, 0.4, 0.0], [1.0, 0.4, 0.0]])
            .states(&[("closed", "sofa"), ("open", "sofa_open")])
            .no_drop()
            .notify()
            .sounds(Some("creak"), None);
        assert_eq!(
            String::from_utf8(furniture_request(&f).unwrap()).unwrap(),
            "furniture sofa\nmodel=sofa\nplace=floor,wall\nturn=4\nscale=1.5\noffset=0 0.25 -0.125\nsize=2 1\nbox.1=1 0 0 1 0.5\nsolid=true\ncells=0 0 0,1 0 0\nlight=12\nlight.at=0 1 0\nseat.1=0 0.4 0\nseat.2=1 0.4 0\nstate.closed=sofa\nstate.open=sofa_open\ndrop=false\nnotify=true\nsound.place=creak\n"
        );
        assert_eq!(
            String::from_utf8(furniture_request(&Furniture::new("chair", "chair")).unwrap())
                .unwrap(),
            "furniture chair\nmodel=chair\nscale=1\noffset=0 0 0\nsize=1 1\n"
        );
    }

    #[test]
    fn storage_is_one_to_six_rows() {
        let text = |f: &Furniture<'_>| String::from_utf8(furniture_request(f).unwrap()).unwrap();
        assert!(text(&Furniture::new("crate", "crate").storage(3)).contains("\nstorage=3\n"));
        assert!(!text(&Furniture::new("crate", "crate")).contains("storage"));
        for rows in [1, 6] {
            assert!(furniture_request(&Furniture::new("crate", "crate").storage(rows)).is_ok());
        }
        for rows in [0, 7, 255] {
            assert_eq!(
                furniture_request(&Furniture::new("crate", "crate").storage(rows))
                    .unwrap_err()
                    .kind,
                crate::ErrorKind::Invalid,
                "{rows}"
            );
        }
    }

    #[test]
    fn trades_are_written_in_order() {
        let trades = [
            Trade::new(
                TradeItem::vanilla("minecraft:emerald", 3),
                TradeItem::model("coin", 1),
            ),
            Trade::new(
                TradeItem::vanilla("minecraft:iron_ingot", 8),
                TradeItem::vanilla("minecraft:diamond", 1),
            )
            .and(TradeItem::model("coin", 2)),
        ];
        let f = Furniture::new("shop", "shop").trades(&trades);
        let text = String::from_utf8(furniture_request(&f).unwrap()).unwrap();
        assert!(
            text.contains(
                "trade.1=minecraft:emerald 3 -> coin 1\ntrade.2=minecraft:iron_ingot 8 + coin 2 -> minecraft:diamond 1\n"
            ),
            "{text}"
        );
        assert!(!text.contains("storage"));
    }

    #[test]
    fn a_trade_cannot_carry_more_than_one_item_per_side() {
        let cases = [
            Trade::new(
                TradeItem::vanilla("minecraft:emerald\ntrade.2=x", 1),
                TradeItem::model("coin", 1),
            ),
            Trade::new(
                TradeItem::vanilla("emerald", 1),
                TradeItem::model("coin", 1),
            ),
            Trade::new(
                TradeItem::vanilla("minecraft:Emerald", 1),
                TradeItem::model("coin", 1),
            ),
            Trade::new(
                TradeItem::vanilla("minecraft:emerald", 0),
                TradeItem::model("coin", 1),
            ),
            Trade::new(
                TradeItem::vanilla("minecraft:emerald", 100),
                TradeItem::model("coin", 1),
            ),
            Trade::new(
                TradeItem::model("coin -> x", 1),
                TradeItem::model("coin", 1),
            ),
            Trade::new(TradeItem::model("Coin", 1), TradeItem::model("coin", 1)),
            Trade::new(
                TradeItem::vanilla("minecraft:emerald", 1),
                TradeItem::model("a b", 1),
            ),
            Trade::new(
                TradeItem::vanilla("minecraft:emerald", 1),
                TradeItem::model("coin", 1),
            )
            .and(TradeItem::model("coin + x", 1)),
        ];
        for t in &cases {
            let trades = [*t];
            let f = Furniture::new("shop", "shop").trades(&trades);
            assert_eq!(
                furniture_request(&f).unwrap_err().kind,
                crate::ErrorKind::Invalid,
                "{t:?}"
            );
        }
        let one = Trade::new(TradeItem::model("coin", 1), TradeItem::model("coin", 2));
        let many = vec![one; MAX_TRADES + 1];
        let f = Furniture::new("shop", "shop").trades(&many);
        assert!(furniture_request(&f).is_err());
        let sixteen = vec![one; MAX_TRADES];
        assert!(furniture_request(&Furniture::new("shop", "shop").trades(&sixteen)).is_ok());
    }

    #[test]
    fn line_breaks_and_odd_numbers_cannot_add_settings() {
        for f in [
            Furniture::new("a\nnotify=true", "m"),
            Furniture::new("a", "m\nsolid=true"),
            Furniture::new("a", "m").states(&[("on", "lamp\ndrop=false")]),
            Furniture::new("a", "m").states(&[("on=x", "lamp")]),
            Furniture::new("a", "m").sounds(Some("a b"), None),
            Furniture::new("a", "m").scale(f32::NAN),
            Furniture::new("a", "m").offset([f32::INFINITY, 0.0, 0.0]),
        ] {
            assert_eq!(
                furniture_request(&f).unwrap_err().kind,
                crate::ErrorKind::Invalid,
                "{f:?}"
            );
        }
        assert!(furniture_state_request("world/x y", "on").is_err());
        assert!(furniture_state_request("world/x", "on\nmore").is_err());
        assert_eq!(
            furniture_state_request("world/x", "on").unwrap(),
            b"furniture-state world/x on"
        );
    }

    #[test]
    fn numbers_keep_three_decimals_at_most() {
        assert_eq!(number(0.4, "x").unwrap(), "0.4");
        assert_eq!(number(-0.0, "x").unwrap(), "0");
        assert_eq!(number(1.23456, "x").unwrap(), "1.235");
        assert_eq!(number(-2.0, "x").unwrap(), "-2");
        assert_eq!(number(-0.0001, "x").unwrap(), "0");
    }

    #[test]
    fn only_custom_models_notices_are_read() {
        let text = b"furniture-notice\tclick\tworld/00000000-0000-4000-8000-000000000001\tlamp\t8667ba71-b85a-4004-af54-457a9734eed7\ton\t0";
        let n = furniture_notice("custom-models", text).unwrap();
        assert_eq!(n.kind, FurnitureNoticeKind::Click);
        assert_eq!(n.furniture, "lamp");
        assert_eq!(n.state, "on");
        assert!(n.player.is_some() && !n.forced);
        assert!(furniture_notice("evil-script", text).is_none());
        assert!(furniture_notice("custom-models", b"furniture-notice\tclick").is_none());
        assert!(
            furniture_notice("custom-models", b"furniture-notice\tdance\tp\tl\t-\t-\t0").is_none()
        );
        let stand =
            furniture_notice("custom-models", b"furniture-notice\tstand\tp\tl\t-\t-\t1").unwrap();
        assert!(stand.player.is_none() && stand.state.is_empty() && stand.forced);
    }

    #[test]
    fn emit_lines_follow_the_other_settings_and_are_bounded() {
        use crate::{Emit, EmitLook};
        let one = Emit {
            look: EmitLook::Plain("flame"),
            count: 2,
            every: 20,
            offset: [0.0, 1.0, 0.0],
            spread: 0.1,
        };
        let both = [one, one];
        let f = Furniture::new("lamp", "lamp").emit(&both);
        let text = String::from_utf8(furniture_request(&f).unwrap()).unwrap();
        assert!(
            text.ends_with("emit.1=flame 2 20 0 1 0 0.1\nemit.2=flame 2 20 0 1 0 0.1\n"),
            "{text}"
        );
        let many = [one; 5];
        assert!(furniture_request(&Furniture::new("lamp", "lamp").emit(&many)).is_err());
        let bad = [Emit { count: 0, ..one }];
        assert!(furniture_request(&Furniture::new("lamp", "lamp").emit(&bad)).is_err());
    }
}
