//! Play Strawberry's tools for your script.
//!
//! Two scripts run on every Play Strawberry server beside yours and do the heavy, repeated work in
//! one place, the cheap way: the **model tool** (`custom-models`) puts your textures, item models,
//! armour looks, blocks and animated characters into the server's resource pack, for Java and
//! Bedrock, and plays your characters' animations; the **interface tool** (`user_interface`)
//! shows your menus and dialogs on both editions. Your script asks them through plugin messages;
//! this crate builds those messages and reads the answers, so you never write one by hand.
//!
//! - [`models`]: register a model, a texture, a sound, a picture in text, an animated character
//!   and the mob that carries it, then give, spawn and read them.
//! - [`ui`]: open, update and close a screen, and read what the player did.
//!
//! ```ignore
//! use strawberry_tools::{models, ui};
//!
//! const COIN: models::Model<'static> = models::Model {
//!     name: "coin",
//!     names: &[("en", "Coin"), ("fr", "Pièce"), ("es", "Moneda"), ("tr", "Jeton")],
//!     base: None,
//!     stack: None,
//!     files: &[("texture.png", include_bytes!("../models/coin.png"))],
//!     wear: None,
//!     behaviour: models::Behaviour::NONE,
//!     variant_of: None,
//! };
//!
//! // in on_load, with "custom-models" and "user_interface" in your metadata's dependencies:
//! let coin = models::register(&COIN)?; // coin.id == "<your script>.coin"
//!
//! // later, for a player:
//! let shop = ui::Screen::menu("shop", ui::Text::plain("Shop"))
//!     .button(ui::Button::new("buy", ui::Text::plain("Buy a coin")).item("minecraft:paper").model_item(&coin.id));
//! ui::open(&player, &shop)?;
//! ```
//!
//! **What the server decides.** The server writes the sender's name on every message, so a tool
//! always knows which script asks. Everything you register is yours alone (`<your script>.<name>`)
//! and in your own folder of the pack; a request naming another script's content is refused. Every
//! file is checked against the limits when you register it, and a request that breaks one is
//! refused whole, with the number that was too high. What would change the game's own files (a
//! vanilla block's look, a boss bar's pictures) waits until the server owner approves it in the
//! Play Strawberry panel.

pub use custom_models_api as models;

pub mod ui;
