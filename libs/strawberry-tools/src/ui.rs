//! The interface tool (`user_interface`): show a menu, a dialog or a notice to a player, on Java
//! and Bedrock, and read what the player did.
//!
//! A **menu** is buttons with item icons in slots (a chest on Java, a button form on Bedrock); a
//! **dialog** is a title, text lines and buttons; a **notice** is a title, text lines and one OK
//! button. A button may show one of your models registered with [`crate::models`]
//! ([`Button::model_item`]): its model on Java, its Bedrock look on Bedrock.
//!
//! What the player does comes back to your script as a plugin message from `user_interface`; read
//! it with [`event`] in your `handle_ipc_message`.
//!
//! ```ignore
//! use strawberry_tools::ui::{self, Button, Event, Screen, Text};
//!
//! let screen = Screen::menu("shop", Text::plain("Shop"))
//!     .rows(1)
//!     .button(Button::new("buy", Text::languages("Buy", "Acheter", "Comprar", "Satın al")).item("minecraft:emerald"));
//! ui::open(&player, &screen)?;
//!
//! // in handle_ipc_message(from, message):
//! if let Some(Event::Click { button, .. }) = ui::event(&from, &message) { /* ... */ }
//! ```
//!
//! Limits, checked by the tool: 54 buttons, 24 lines, a title of 256 characters and a line of 512,
//! ids of 1 to 64 of `a-z 0-9 _ . : -`, a count of 1 to 99, a message of 64 KiB.

use serde_json::{Map, Value, json};
use strawberry_plugin_api::Player;

/// The interface tool's name: the recipient of every request and the sender of every event.
pub const PLUGIN: &str = "user_interface";

/// What went wrong with a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The interface tool is not on this server.
    NotInstalled,
    /// The tool refused the request; the reason is its text (`player not online`, a limit, a
    /// model of another script, ...).
    Refused(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotInstalled => f.write_str("user_interface is not on this server"),
            Self::Refused(reason) => f.write_str(reason),
        }
    }
}

impl std::error::Error for Error {}

/// A text a player reads: the same for everyone, or one per language (the player reads his own;
/// English where his is missing).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Text {
    Plain(String),
    Languages {
        en: String,
        fr: String,
        es: String,
        tr: String,
    },
}

impl Text {
    #[must_use]
    pub fn plain(text: &str) -> Self {
        Self::Plain(text.to_owned())
    }

    #[must_use]
    pub fn languages(en: &str, fr: &str, es: &str, tr: &str) -> Self {
        Self::Languages {
            en: en.to_owned(),
            fr: fr.to_owned(),
            es: es.to_owned(),
            tr: tr.to_owned(),
        }
    }

    fn json(&self) -> Value {
        match self {
            Self::Plain(text) => json!(text),
            Self::Languages { en, fr, es, tr } => json!({"en": en, "fr": fr, "es": es, "tr": tr}),
        }
    }
}

/// One button of a screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Button {
    id: String,
    label: Text,
    item: Option<String>,
    model_item: Option<String>,
    slot: Option<u8>,
    tooltip: Vec<Text>,
    count: Option<u8>,
    close: bool,
}

impl Button {
    /// A button `id` (what comes back in [`Event::Click`]) with its label.
    #[must_use]
    pub fn new(id: &str, label: Text) -> Self {
        Self {
            id: id.to_owned(),
            label,
            item: None,
            model_item: None,
            slot: None,
            tooltip: Vec::new(),
            count: None,
            close: false,
        }
    }

    /// The vanilla item its icon is in a menu (`minecraft:emerald`).
    #[must_use]
    pub fn item(mut self, item: &str) -> Self {
        self.item = Some(item.to_owned());
        self
    }

    /// One of your models registered with [`crate::models`], by its id (`<your script>.<name>`):
    /// the button shows that item once players have it in the server's pack, and its own
    /// [`Button::item`] before. Only your own models: another script's is refused.
    #[must_use]
    pub fn model_item(mut self, id: &str) -> Self {
        self.model_item = Some(id.to_owned());
        self
    }

    /// The slot in a menu (0 is the top left); the tool places a button without one.
    #[must_use]
    pub const fn slot(mut self, slot: u8) -> Self {
        self.slot = Some(slot);
        self
    }

    /// A line under the label when the pointer is over it.
    #[must_use]
    pub fn tooltip(mut self, line: Text) -> Self {
        self.tooltip.push(line);
        self
    }

    /// How many the icon shows, 1 to 99.
    #[must_use]
    pub const fn count(mut self, count: u8) -> Self {
        self.count = Some(count);
        self
    }

    /// The screen closes when the player presses it.
    #[must_use]
    pub const fn close(mut self) -> Self {
        self.close = true;
        self
    }

    fn json(&self) -> Value {
        let mut map = Map::new();
        map.insert("id".to_owned(), json!(self.id));
        map.insert("label".to_owned(), self.label.json());
        if let Some(item) = &self.item {
            map.insert("item".to_owned(), json!(item));
        }
        if let Some(id) = &self.model_item {
            map.insert("model_item".to_owned(), json!(id));
        }
        if let Some(slot) = self.slot {
            map.insert("slot".to_owned(), json!(slot));
        }
        if !self.tooltip.is_empty() {
            map.insert(
                "tooltip".to_owned(),
                Value::Array(self.tooltip.iter().map(Text::json).collect()),
            );
        }
        if let Some(count) = self.count {
            map.insert("count".to_owned(), json!(count));
        }
        if self.close {
            map.insert("close".to_owned(), json!(true));
        }
        Value::Object(map)
    }
}

/// A screen: a menu, a dialog or a notice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Screen {
    id: String,
    kind: &'static str,
    title: Text,
    body: Vec<Text>,
    buttons: Vec<Button>,
    rows: Option<u8>,
    can_close: bool,
}

impl Screen {
    fn new(kind: &'static str, id: &str, title: Text) -> Self {
        Self {
            id: id.to_owned(),
            kind,
            title,
            body: Vec::new(),
            buttons: Vec::new(),
            rows: None,
            can_close: true,
        }
    }

    /// Buttons with item icons in slots: a chest on Java, a button form on Bedrock.
    #[must_use]
    pub fn menu(id: &str, title: Text) -> Self {
        Self::new("menu", id, title)
    }

    /// A title, text lines and buttons.
    #[must_use]
    pub fn dialog(id: &str, title: Text) -> Self {
        Self::new("dialog", id, title)
    }

    /// A title, text lines and one OK button.
    #[must_use]
    pub fn notice(id: &str, title: Text) -> Self {
        Self::new("notice", id, title)
    }

    /// A text line (a dialog's or a notice's body).
    #[must_use]
    pub fn line(mut self, line: Text) -> Self {
        self.body.push(line);
        self
    }

    #[must_use]
    pub fn button(mut self, button: Button) -> Self {
        self.buttons.push(button);
        self
    }

    /// A menu's rows, 1 to 6; the tool chooses from the buttons without it.
    #[must_use]
    pub const fn rows(mut self, rows: u8) -> Self {
        self.rows = Some(rows);
        self
    }

    /// Whether the player may close it himself (Esc); true unless set.
    #[must_use]
    pub const fn can_close(mut self, can_close: bool) -> Self {
        self.can_close = can_close;
        self
    }

    /// The screen as the tool reads it.
    #[must_use]
    pub fn json(&self) -> Value {
        let mut map = Map::new();
        map.insert("id".to_owned(), json!(self.id));
        map.insert("kind".to_owned(), json!(self.kind));
        map.insert("title".to_owned(), self.title.json());
        if !self.body.is_empty() {
            map.insert(
                "body".to_owned(),
                Value::Array(self.body.iter().map(Text::json).collect()),
            );
        }
        map.insert(
            "buttons".to_owned(),
            Value::Array(self.buttons.iter().map(Button::json).collect()),
        );
        if let Some(rows) = self.rows {
            map.insert("rows".to_owned(), json!(rows));
        }
        if !self.can_close {
            map.insert("can_close".to_owned(), json!(false));
        }
        Value::Object(map)
    }
}

/// How a screen reached the player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opened {
    /// `chest` (a menu, on both editions), `dialog` (Java) or `form` (a Bedrock player's dialog).
    pub shown_as: String,
}

/// The bytes of an `open` request.
#[must_use]
pub fn open_request(player: &str, screen: &Screen) -> Vec<u8> {
    json!({"op": "open", "player": player, "screen": screen.json()})
        .to_string()
        .into_bytes()
}

/// The bytes of an `update` request: the screen your script has open for the player, changed in
/// place.
#[must_use]
pub fn update_request(player: &str, screen: &Screen) -> Vec<u8> {
    json!({"op": "update", "player": player, "screen": screen.json()})
        .to_string()
        .into_bytes()
}

/// The bytes of a `close` request.
#[must_use]
pub fn close_request(player: &str) -> Vec<u8> {
    json!({"op": "close", "player": player})
        .to_string()
        .into_bytes()
}

/// The tool's answer: its fields, or the reason it refused.
///
/// # Errors
/// [`Error::Refused`] with the tool's reason, also for an answer that is no JSON object.
pub fn parse_answer(answer: &[u8]) -> Result<Map<String, Value>, Error> {
    match serde_json::from_slice::<Value>(answer) {
        Ok(Value::Object(map)) if map.get("ok") == Some(&Value::Bool(true)) => Ok(map),
        Ok(Value::Object(map)) => Err(Error::Refused(
            map.get("error")
                .and_then(Value::as_str)
                .unwrap_or("refused")
                .to_owned(),
        )),
        _ => Err(Error::Refused(
            "an answer that is no JSON object".to_owned(),
        )),
    }
}

fn ask(message: &[u8]) -> Result<Map<String, Value>, Error> {
    match strawberry_plugin_api::ipc::send_ipc_message(PLUGIN, message) {
        Ok(Ok(answer)) => parse_answer(&answer),
        Ok(Err(reason)) => Err(Error::Refused(reason)),
        Err(()) => Err(Error::NotInstalled),
    }
}

fn player_id(player: &Player) -> String {
    strawberry_plugin_api::uuid::to_string(player.get_id())
}

/// Shows `screen` to `player`, in place of what he has open.
///
/// # Errors
/// [`Error::NotInstalled`], or the tool's refusal.
pub fn open(player: &Player, screen: &Screen) -> Result<Opened, Error> {
    let answer = ask(&open_request(&player_id(player), screen))?;
    Ok(Opened {
        shown_as: answer
            .get("shown_as")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
    })
}

/// Changes the screen your script has open for `player` without closing it.
///
/// # Errors
/// [`Error::NotInstalled`], or the tool's refusal.
pub fn update(player: &Player, screen: &Screen) -> Result<(), Error> {
    ask(&update_request(&player_id(player), screen)).map(|_| ())
}

/// Closes `player`'s screen.
///
/// # Errors
/// [`Error::NotInstalled`], or the tool's refusal.
pub fn close(player: &Player) -> Result<(), Error> {
    ask(&close_request(&player_id(player))).map(|_| ())
}

/// What a player did on a screen your script opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// He pressed a button.
    Click {
        player: String,
        screen: String,
        button: String,
    },
    /// He pressed a button of a dialog with inputs: each input's id and what he entered.
    Submit {
        player: String,
        screen: String,
        button: String,
        inputs: Vec<(String, String)>,
    },
    /// The screen closed: `player` (Esc), `replaced`, `quit`, `request` or `invalid`.
    Close {
        player: String,
        screen: String,
        reason: String,
    },
}

/// Reads a plugin message as an interface event. Only a message whose sender the server named
/// `user_interface` is one; anything else gives `None`, so another script cannot fake a click.
#[must_use]
pub fn event(from: &str, message: &[u8]) -> Option<Event> {
    if from != PLUGIN {
        return None;
    }
    let Ok(Value::Object(map)) = serde_json::from_slice::<Value>(message) else {
        return None;
    };
    let text = |key: &str| map.get(key).and_then(Value::as_str).map(str::to_owned);
    let (player, screen) = (text("player")?, text("screen")?);
    match map.get("event").and_then(Value::as_str)? {
        "click" => Some(Event::Click {
            player,
            screen,
            button: text("button")?,
        }),
        "submit" => Some(Event::Submit {
            player,
            screen,
            button: text("button")?,
            inputs: map
                .get("inputs")
                .and_then(Value::as_object)
                .map(|inputs| {
                    inputs
                        .iter()
                        .map(|(id, value)| {
                            let value = match value {
                                Value::String(s) => s.clone(),
                                other => other.to_string(),
                            };
                            (id.clone(), value)
                        })
                        .collect()
                })
                .unwrap_or_default(),
        }),
        "close" => Some(Event::Close {
            player,
            screen,
            reason: text("reason")?,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_menu_with_a_registered_model_is_the_request_the_tool_reads() {
        let screen = Screen::menu(
            "shop",
            Text::languages("Shop", "Boutique", "Tienda", "Dükkan"),
        )
        .rows(1)
        .button(
            Button::new("buy", Text::plain("Buy"))
                .item("minecraft:emerald")
                .model_item("my_shop.coin")
                .slot(4)
                .count(3)
                .tooltip(Text::plain("One coin"))
                .close(),
        );
        let request: Value = serde_json::from_slice(&open_request("u-1", &screen)).unwrap();
        assert_eq!(
            request,
            json!({"op": "open", "player": "u-1", "screen": {
                "id": "shop", "kind": "menu",
                "title": {"en": "Shop", "fr": "Boutique", "es": "Tienda", "tr": "Dükkan"},
                "buttons": [{"id": "buy", "label": "Buy", "item": "minecraft:emerald",
                    "model_item": "my_shop.coin", "slot": 4, "tooltip": ["One coin"], "count": 3,
                    "close": true}],
                "rows": 1}})
        );
        let closed: Value = serde_json::from_slice(&close_request("u-1")).unwrap();
        assert_eq!(closed, json!({"op": "close", "player": "u-1"}));
    }

    #[test]
    fn an_answer_is_its_fields_or_the_tools_reason() {
        let ok = parse_answer(br#"{"ok": true, "shown_as": "chest"}"#).unwrap();
        assert_eq!(ok["shown_as"], "chest");
        assert_eq!(
            parse_answer(br#"{"ok": false, "error": "player not online"}"#),
            Err(Error::Refused("player not online".to_owned()))
        );
        assert!(parse_answer(b"not json").is_err());
    }

    #[test]
    fn only_the_interface_tool_sends_events() {
        let click = br#"{"event": "click", "player": "u-1", "screen": "shop", "button": "buy"}"#;
        assert_eq!(
            event(PLUGIN, click),
            Some(Event::Click {
                player: "u-1".to_owned(),
                screen: "shop".to_owned(),
                button: "buy".to_owned()
            })
        );
        // Another script cannot fake one.
        assert_eq!(event("thief", click), None);
        let submit = br#"{"event": "submit", "player": "u-1", "screen": "s", "button": "ok", "inputs": {"name": "Ada", "age": 12}}"#;
        let Some(Event::Submit { inputs, .. }) = event(PLUGIN, submit) else {
            panic!("a submit");
        };
        assert!(inputs.contains(&("name".to_owned(), "Ada".to_owned())));
        assert!(inputs.contains(&("age".to_owned(), "12".to_owned())));
        assert_eq!(
            event(PLUGIN, br#"{"event": "click", "player": "u-1"}"#),
            None
        );
    }
}
