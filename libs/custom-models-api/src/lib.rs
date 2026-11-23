//! Typed calls for plugins that use custom-models.
//!
//! A plugin keeps its models in its own folder and registers them when it loads; a server then has
//! exactly the models of the plugins installed on it.
//!
//! ```ignore
//! use custom_models_api::{Behaviour, Model, register};
//!
//! const BUCKET: Model<'static> = Model {
//!     name: "villager_bucket",
//!     names: &[("en", "Bucket of Villager"), ("tr", "Köylü Kovası")],
//!     base: None,
//!     stack: Some(1),
//!     files: &[("texture.png", include_bytes!("../models/villager_bucket.png"))],
//!     wear: None,
//!     behaviour: Behaviour::NONE,
//!     variant_of: None,
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
//! An interface drawn with images and its own letters registers them with [`fonts`] (Java only).
//! Armour with its own look registers the look with [`armour`] and then the items that wear it
//! ([`Model::wear`]), with what they give ([`Behaviour`]: protection, durability, repair, the equip
//! sound); a whole set at once with [`armour_set`], any models at once with [`register_all`].
//! A solid block with its own look, placed with its item, is a [`block`] ([`Block`]).
//! A plugin with a valuable item treats anything but `Ok` as "give nothing".

use strawberry_plugin_api::{ItemStack, Player};

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
    /// Worn in an armour slot or by an animal: part of the item's registration, so every stack
    /// of it is worn the same way.
    pub wear: Option<Wear<'a>>,
    /// What the item gives beyond its look ([`Behaviour::NONE`] for nothing).
    pub behaviour: Behaviour<'a>,
    /// A variant of another of your models (by its name): what this model leaves out (its names,
    /// files, base, stack, wear, behaviour) is copied from that model as it is now. A copy, not a
    /// link: changing that model later does not change this one. A setting cannot be removed by
    /// a variant, only given another value.
    pub variant_of: Option<&'a str>,
}

impl<'a> Model<'a> {
    /// A model with its name, its names per language and its files; nothing else set.
    #[must_use]
    pub const fn new(
        name: &'a str,
        names: &'a [(&'a str, &'a str)],
        files: &'a [(&'a str, &'a [u8])],
    ) -> Self {
        Self {
            name,
            names,
            base: None,
            stack: None,
            files,
            wear: None,
            behaviour: Behaviour::NONE,
            variant_of: None,
        }
    }

    /// A variant of your model `of`, named `name`: everything is copied from `of`; set what
    /// differs with the other builders (names, files, base, stack, wear, what it gives).
    #[must_use]
    pub const fn variant(name: &'a str, of: &'a str) -> Self {
        let mut model = Self::new(name, &[], &[]);
        model.variant_of = Some(of);
        model
    }

    #[must_use]
    pub const fn names(mut self, names: &'a [(&'a str, &'a str)]) -> Self {
        self.names = names;
        self
    }

    #[must_use]
    pub const fn files(mut self, files: &'a [(&'a str, &'a [u8])]) -> Self {
        self.files = files;
        self
    }

    #[must_use]
    pub const fn base(mut self, base: &'a str) -> Self {
        self.base = Some(base);
        self
    }

    #[must_use]
    pub const fn stack(mut self, stack: u8) -> Self {
        self.stack = Some(stack);
        self
    }

    #[must_use]
    pub const fn worn(mut self, wear: Wear<'a>) -> Self {
        self.wear = Some(wear);
        self
    }

    #[must_use]
    pub const fn gives(mut self, behaviour: Behaviour<'a>) -> Self {
        self.behaviour = behaviour;
        self
    }
}

/// Where a model item is worn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Worn {
    Head,
    Chest,
    Legs,
    Feet,
    /// An animal's armour.
    Body,
}

impl Worn {
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Head => "head",
            Self::Chest => "chest",
            Self::Legs => "legs",
            Self::Feet => "feet",
            Self::Body => "body",
        }
    }
}

/// An animal that wears body armour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Animal {
    Wolf,
    Horse,
    /// A llama or a trader's llama.
    Llama,
}

impl Animal {
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Wolf => "wolf",
            Self::Horse => "horse",
            Self::Llama => "llama",
        }
    }
}

/// Where a model item is worn and how it is drawn there.
#[derive(Debug, Clone, Copy)]
pub struct Wear<'a> {
    pub slot: Worn,
    /// The name of one of your [`armour`] looks, registered before this model. `None` only on the
    /// head: the item is drawn there with its own model (a hat, a mask, any 3D head piece).
    pub armour: Option<&'a str>,
    /// Wings: with [`Worn::Chest`] and a look that has the `Wings` layer, the player glides as
    /// with an elytra.
    pub glide: bool,
    /// With [`Worn::Body`]: the animals that wear it; the look has the layer of each.
    pub fits: &'a [Animal],
}

impl<'a> Wear<'a> {
    /// Worn in `slot`, drawn with your armour look `look`.
    #[must_use]
    pub const fn armour(slot: Worn, look: &'a str) -> Self {
        Self {
            slot,
            armour: Some(look),
            glide: false,
            fits: &[],
        }
    }

    /// A hat: worn on the head, drawn with the item's own model.
    #[must_use]
    pub const fn hat() -> Self {
        Self {
            slot: Worn::Head,
            armour: None,
            glide: false,
            fits: &[],
        }
    }

    /// Wings: worn on the chest, drawn with `look` (which has the `Wings` layer); the player
    /// glides as with an elytra.
    #[must_use]
    pub const fn wings(look: &'a str) -> Self {
        Self {
            slot: Worn::Chest,
            armour: Some(look),
            glide: true,
            fits: &[],
        }
    }

    /// An animal's armour, drawn with `look` (which has the layer of each animal in `fits`).
    #[must_use]
    pub const fn animal(look: &'a str, fits: &'a [Animal]) -> Self {
        Self {
            slot: Worn::Body,
            armour: Some(look),
            glide: false,
            fits,
        }
    }
}

/// How long an item lasts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Durability {
    /// Uses before it breaks, 1 to 100000 (vanilla's maximum damage). Such an item does not stack.
    Max(u32),
    Unbreakable,
}

/// What repairs an item in an anvil.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Repair<'a> {
    /// A vanilla item, `minecraft:diamond`.
    Item(&'a str),
    /// A vanilla item tag, without `#`: `minecraft:repairs_iron_armor`.
    Tag(&'a str),
}

/// The sound played when the item is put on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EquipSound<'a> {
    Generic,
    Leather,
    Chain,
    Iron,
    Gold,
    Diamond,
    Netherite,
    Turtle,
    Elytra,
    Wolf,
    Copper,
    /// One of your [`sound`]s, by its name, registered before this model.
    Own(&'a str),
}

impl EquipSound<'_> {
    fn setting(&self) -> String {
        let word = match self {
            Self::Generic => "generic",
            Self::Leather => "leather",
            Self::Chain => "chain",
            Self::Iron => "iron",
            Self::Gold => "gold",
            Self::Diamond => "diamond",
            Self::Netherite => "netherite",
            Self::Turtle => "turtle",
            Self::Elytra => "elytra",
            Self::Wolf => "wolf",
            Self::Copper => "copper",
            Self::Own(name) => return format!("own:{name}"),
        };
        word.to_owned()
    }
}

/// What a model item gives beyond its look. custom-models checks every value (the ranges are on
/// each builder) and answers `Invalid` naming the setting otherwise; decimals keep three places.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Behaviour<'a> {
    /// Armour points, 0 to 30 (needs [`Model::wear`]).
    pub armor: Option<f64>,
    /// Armour toughness, 0 to 20 (needs [`Model::wear`]).
    pub armor_toughness: Option<f64>,
    /// Knockback resistance, 0 to 1 (needs [`Model::wear`]).
    pub knockback_resistance: Option<f64>,
    /// A share of the base movement speed, -0.5 to 0.5 (needs [`Model::wear`]).
    pub movement_speed: Option<f64>,
    pub durability: Option<Durability>,
    pub repair: Option<Repair<'a>>,
    /// How well it takes enchantments, 1 to 50 (vanilla: iron 9, diamond 10, gold 25).
    pub enchantable: Option<u8>,
    /// Needs [`Model::wear`]; vanilla's generic sound when `None`.
    pub equip_sound: Option<EquipSound<'a>>,
    /// Held in the main hand (leave [`Model::wear`] out): attack damage added, 0 to 100.
    pub attack_damage: Option<f64>,
    /// Attack speed added to the base 4, -3.9 to 10 (a sword: -2.4).
    pub attack_speed: Option<f64>,
    /// Reach for blocks and for entities added, -3 to 10 each.
    pub reach_block: Option<f64>,
    pub reach_entity: Option<f64>,
    /// Attack knockback added, 0 to 5.
    pub attack_knockback: Option<f64>,
    pub tool: Option<Tool<'a>>,
    pub food: Option<Food<'a>>,
    pub cooldown: Option<Cooldown<'a>>,
    pub rarity: Option<Rarity>,
    /// The enchantment glint on or off.
    pub glint: Option<bool>,
    /// Lines of lore per language, `("en", &["First line", "Second line"])`: English has every
    /// line, fr, es and tr may leave one out (the player then reads English). Up to 8 lines of 1
    /// to 96 characters; the player reads them in their own language.
    pub lore: &'a [(&'a str, &'a [&'a str])],
    /// Vanilla enchantments with their level, 1 to 255: `("sharpness", 5)`; at most 16.
    pub enchantments: &'a [(&'a str, u8)],
    /// The item does not burn in fire or lava.
    pub fire_resistant: bool,
    /// Parts of the tooltip left out.
    pub hide: &'a [Hide],
    /// Whether the item also counts as its base item in vanilla's uses by id (a plain recipe
    /// ingredient, a fuel, a trade cost, an anvil repair material): false by default, so a model
    /// on paper is no paper. A model worn with a dyeable armour look gets it by itself (dyeing
    /// in the crafting grid matches the base item).
    pub acts_as: bool,
}

/// Blocks a tool rule applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blocks<'a> {
    /// A vanilla block tag, without `#`: `minecraft:mineable/pickaxe`.
    Tag(&'a str),
    /// Up to 16 vanilla blocks: `minecraft:stone`.
    Ids(&'a [&'a str]),
}

impl Blocks<'_> {
    fn setting(&self) -> Result<String, Error> {
        let text = match self {
            Self::Tag(tag) => format!("#{tag}"),
            Self::Ids(ids) => ids.join(","),
        };
        word_safe(&text, "a tool's blocks")?;
        Ok(text)
    }
}

/// A rule of a tool, tried before its main rule.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToolRule<'a> {
    pub blocks: Blocks<'a>,
    /// Mining speed on those blocks, 0.1 to 100.
    pub speed: Option<f64>,
    /// Whether those blocks drop their items.
    pub drops: Option<bool>,
}

/// How a tool mines: its main rule (the blocks it is for), up to 4 rules tried before it, and the
/// durability it loses per block.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tool<'a> {
    pub blocks: Blocks<'a>,
    /// Mining speed on those blocks, 0.1 to 100 (vanilla: wood 2, stone 4, iron 6, diamond 8).
    pub speed: Option<f64>,
    pub drops: Option<bool>,
    /// 0 to 10.
    pub damage_per_block: Option<u8>,
    pub rules: &'a [ToolRule<'a>],
}

impl<'a> Tool<'a> {
    /// A tool for `blocks`.
    #[must_use]
    pub const fn mines(blocks: Blocks<'a>) -> Self {
        Self {
            blocks,
            speed: None,
            drops: None,
            damage_per_block: None,
            rules: &[],
        }
    }

    #[must_use]
    pub const fn speed(mut self, speed: f64) -> Self {
        self.speed = Some(speed);
        self
    }

    #[must_use]
    pub const fn drops(mut self, drops: bool) -> Self {
        self.drops = Some(drops);
        self
    }

    #[must_use]
    pub const fn damage_per_block(mut self, damage: u8) -> Self {
        self.damage_per_block = Some(damage);
        self
    }

    #[must_use]
    pub const fn rules(mut self, rules: &'a [ToolRule<'a>]) -> Self {
        self.rules = rules;
        self
    }
}

/// An effect given on eating, with its chance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Effect<'a> {
    /// A vanilla effect: `speed`, `minecraft:regeneration`.
    pub id: &'a str,
    /// 1 to 3600.
    pub seconds: u16,
    /// 1 to 10.
    pub level: u8,
    /// 0 to 1.
    pub chance: f64,
}

/// Food or drink.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Food<'a> {
    /// 0 to 20.
    pub nutrition: u8,
    /// 0 to 20.
    pub saturation: f64,
    /// Eaten even when the player is not hungry.
    pub always: bool,
    /// How long eating takes, 0.1 to 60 seconds (1.6 when `None`).
    pub seconds: Option<f64>,
    /// Drunk rather than eaten.
    pub drink: bool,
    /// Up to 4.
    pub effects: &'a [Effect<'a>],
    /// A vanilla item left in the hand: `minecraft:bowl`.
    pub leftover: Option<&'a str>,
}

impl<'a> Food<'a> {
    #[must_use]
    pub const fn new(nutrition: u8, saturation: f64) -> Self {
        Self {
            nutrition,
            saturation,
            always: false,
            seconds: None,
            drink: false,
            effects: &[],
            leftover: None,
        }
    }

    #[must_use]
    pub const fn always(mut self) -> Self {
        self.always = true;
        self
    }

    #[must_use]
    pub const fn seconds(mut self, seconds: f64) -> Self {
        self.seconds = Some(seconds);
        self
    }

    #[must_use]
    pub const fn drink(mut self) -> Self {
        self.drink = true;
        self
    }

    #[must_use]
    pub const fn effects(mut self, effects: &'a [Effect<'a>]) -> Self {
        self.effects = effects;
        self
    }

    #[must_use]
    pub const fn leftover(mut self, item: &'a str) -> Self {
        self.leftover = Some(item);
        self
    }
}

/// A cooldown after use, 0.05 to 3600 seconds. Models with the same `group` (one of your names)
/// share it; without one each model has its own.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cooldown<'a> {
    pub seconds: f64,
    pub group: Option<&'a str>,
}

/// The colour of an item's name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Epic,
}

/// A part of the tooltip to leave out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hide {
    Attributes,
    Enchantments,
    Unbreakable,
    DyedColor,
    Trim,
    /// The whole tooltip.
    Tooltip,
}

impl Hide {
    const fn word(self) -> &'static str {
        match self {
            Self::Attributes => "attributes",
            Self::Enchantments => "enchantments",
            Self::Unbreakable => "unbreakable",
            Self::DyedColor => "dyed_color",
            Self::Trim => "trim",
            Self::Tooltip => "tooltip",
        }
    }
}

impl<'a> Behaviour<'a> {
    /// Nothing beyond the look.
    pub const NONE: Self = Self {
        armor: None,
        armor_toughness: None,
        knockback_resistance: None,
        movement_speed: None,
        durability: None,
        repair: None,
        enchantable: None,
        equip_sound: None,
        attack_damage: None,
        attack_speed: None,
        reach_block: None,
        reach_entity: None,
        attack_knockback: None,
        tool: None,
        food: None,
        cooldown: None,
        rarity: None,
        glint: None,
        lore: &[],
        enchantments: &[],
        fire_resistant: false,
        hide: &[],
        acts_as: false,
    };

    #[must_use]
    pub const fn attack_damage(mut self, damage: f64) -> Self {
        self.attack_damage = Some(damage);
        self
    }

    #[must_use]
    pub const fn attack_speed(mut self, speed: f64) -> Self {
        self.attack_speed = Some(speed);
        self
    }

    #[must_use]
    pub const fn reach_block(mut self, blocks: f64) -> Self {
        self.reach_block = Some(blocks);
        self
    }

    #[must_use]
    pub const fn reach_entity(mut self, blocks: f64) -> Self {
        self.reach_entity = Some(blocks);
        self
    }

    #[must_use]
    pub const fn attack_knockback(mut self, knockback: f64) -> Self {
        self.attack_knockback = Some(knockback);
        self
    }

    #[must_use]
    pub const fn tool(mut self, tool: Tool<'a>) -> Self {
        self.tool = Some(tool);
        self
    }

    #[must_use]
    pub const fn food(mut self, food: Food<'a>) -> Self {
        self.food = Some(food);
        self
    }

    #[must_use]
    pub const fn cooldown(mut self, seconds: f64, group: Option<&'a str>) -> Self {
        self.cooldown = Some(Cooldown { seconds, group });
        self
    }

    #[must_use]
    pub const fn rarity(mut self, rarity: Rarity) -> Self {
        self.rarity = Some(rarity);
        self
    }

    #[must_use]
    pub const fn glint(mut self, glint: bool) -> Self {
        self.glint = Some(glint);
        self
    }

    #[must_use]
    pub const fn lore(mut self, lore: &'a [(&'a str, &'a [&'a str])]) -> Self {
        self.lore = lore;
        self
    }

    #[must_use]
    pub const fn enchantments(mut self, enchantments: &'a [(&'a str, u8)]) -> Self {
        self.enchantments = enchantments;
        self
    }

    #[must_use]
    pub const fn acts_as(mut self) -> Self {
        self.acts_as = true;
        self
    }

    #[must_use]
    pub const fn fire_resistant(mut self) -> Self {
        self.fire_resistant = true;
        self
    }

    #[must_use]
    pub const fn hide(mut self, parts: &'a [Hide]) -> Self {
        self.hide = parts;
        self
    }

    #[must_use]
    pub const fn armor(mut self, points: f64) -> Self {
        self.armor = Some(points);
        self
    }

    #[must_use]
    pub const fn armor_toughness(mut self, toughness: f64) -> Self {
        self.armor_toughness = Some(toughness);
        self
    }

    #[must_use]
    pub const fn knockback_resistance(mut self, share: f64) -> Self {
        self.knockback_resistance = Some(share);
        self
    }

    #[must_use]
    pub const fn movement_speed(mut self, share: f64) -> Self {
        self.movement_speed = Some(share);
        self
    }

    #[must_use]
    pub const fn durability(mut self, uses: u32) -> Self {
        self.durability = Some(Durability::Max(uses));
        self
    }

    #[must_use]
    pub const fn unbreakable(mut self) -> Self {
        self.durability = Some(Durability::Unbreakable);
        self
    }

    #[must_use]
    pub const fn repair(mut self, repair: Repair<'a>) -> Self {
        self.repair = Some(repair);
        self
    }

    #[must_use]
    pub const fn enchantable(mut self, value: u8) -> Self {
        self.enchantable = Some(value);
        self
    }

    #[must_use]
    pub const fn equip_sound(mut self, sound: EquipSound<'a>) -> Self {
        self.equip_sound = Some(sound);
        self
    }

    /// The settings lines.
    fn settings(&self, out: &mut String) -> Result<(), Error> {
        for (key, value) in [
            ("armor", self.armor),
            ("armor_toughness", self.armor_toughness),
            ("knockback_resistance", self.knockback_resistance),
            ("movement_speed", self.movement_speed),
        ] {
            if let Some(value) = value {
                out.push_str(&format!("{key}={}\n", decimal(key, value)?));
            }
        }
        match self.durability {
            Some(Durability::Max(uses)) => out.push_str(&format!("durability={uses}\n")),
            Some(Durability::Unbreakable) => out.push_str("unbreakable=true\n"),
            None => {}
        }
        match self.repair {
            Some(Repair::Item(item)) => {
                word_safe(item, "a repair item")?;
                out.push_str(&format!("repair={item}\n"));
            }
            Some(Repair::Tag(tag)) => {
                word_safe(tag, "a repair tag")?;
                out.push_str(&format!("repair=#{tag}\n"));
            }
            None => {}
        }
        if let Some(value) = self.enchantable {
            out.push_str(&format!("enchantable={value}\n"));
        }
        if let Some(sound) = &self.equip_sound {
            let value = sound.setting();
            word_safe(&value, "an equip sound")?;
            out.push_str(&format!("equip_sound={value}\n"));
        }
        for (key, value) in [
            ("attack_damage", self.attack_damage),
            ("attack_speed", self.attack_speed),
            ("reach.block", self.reach_block),
            ("reach.entity", self.reach_entity),
            ("attack_knockback", self.attack_knockback),
        ] {
            if let Some(value) = value {
                out.push_str(&format!("{key}={}\n", decimal(key, value)?));
            }
        }
        if let Some(tool) = &self.tool {
            let main = ToolRule {
                blocks: tool.blocks,
                speed: tool.speed,
                drops: tool.drops,
            };
            if tool.rules.len() > 4 {
                return Err(invalid("a tool has at most 4 rules besides its main one"));
            }
            for (prefix, rule) in std::iter::once(("tool".to_owned(), &main)).chain(
                tool.rules
                    .iter()
                    .enumerate()
                    .map(|(i, rule)| (format!("tool.{}", i + 1), rule)),
            ) {
                out.push_str(&format!("{prefix}.blocks={}\n", rule.blocks.setting()?));
                if let Some(speed) = rule.speed {
                    out.push_str(&format!(
                        "{prefix}.speed={}\n",
                        decimal("tool.speed", speed)?
                    ));
                }
                if let Some(drops) = rule.drops {
                    out.push_str(&format!("{prefix}.drops={drops}\n"));
                }
            }
            if let Some(damage) = tool.damage_per_block {
                out.push_str(&format!("tool.damage_per_block={damage}\n"));
            }
        }
        if let Some(food) = &self.food {
            out.push_str(&format!(
                "food.nutrition={}\nfood.saturation={}\n",
                food.nutrition,
                decimal("food.saturation", food.saturation)?
            ));
            if food.always {
                out.push_str("food.always=true\n");
            }
            if let Some(seconds) = food.seconds {
                out.push_str(&format!(
                    "food.seconds={}\n",
                    decimal("food.seconds", seconds)?
                ));
            }
            if food.drink {
                out.push_str("food.animation=drink\n");
            }
            if food.effects.len() > 4 {
                return Err(invalid("food gives at most 4 effects"));
            }
            for (i, effect) in food.effects.iter().enumerate() {
                word_safe(effect.id, "an effect")?;
                out.push_str(&format!(
                    "food.effect.{}={} {} {} {}\n",
                    i + 1,
                    effect.id,
                    effect.seconds,
                    effect.level,
                    decimal("food.effect", effect.chance)?
                ));
            }
            if let Some(item) = food.leftover {
                word_safe(item, "a leftover item")?;
                out.push_str(&format!("food.leftover={item}\n"));
            }
        }
        if let Some(cooldown) = &self.cooldown {
            out.push_str(&format!(
                "cooldown={}\n",
                decimal("cooldown", cooldown.seconds)?
            ));
            if let Some(group) = cooldown.group {
                word_safe(group, "a cooldown group")?;
                out.push_str(&format!("cooldown.group={group}\n"));
            }
        }
        if let Some(rarity) = self.rarity {
            let word = match rarity {
                Rarity::Common => "common",
                Rarity::Uncommon => "uncommon",
                Rarity::Rare => "rare",
                Rarity::Epic => "epic",
            };
            out.push_str(&format!("rarity={word}\n"));
        }
        if let Some(glint) = self.glint {
            out.push_str(&format!("glint={glint}\n"));
        }
        for (language, lines) in self.lore {
            word_safe(language, "a lore language")?;
            for (n, line) in lines.iter().enumerate() {
                if line.contains(['\n', '\r']) {
                    return Err(invalid("a line of lore has no line breaks"));
                }
                out.push_str(&format!("lore.{language}.{}={line}\n", n + 1));
            }
        }
        for (name, level) in self.enchantments {
            word_safe(name, "an enchantment")?;
            out.push_str(&format!("enchant.{name}={level}\n"));
        }
        if self.acts_as {
            out.push_str("acts_as=true\n");
        }
        if self.fire_resistant {
            out.push_str("fire_resistant=true\n");
        }
        if !self.hide.is_empty() {
            let parts: Vec<&str> = self.hide.iter().map(|h| h.word()).collect();
            out.push_str(&format!("hide={}\n", parts.join(",")));
        }
        Ok(())
    }
}

/// A decimal setting as custom-models reads it: at most three places, never `NaN` or infinite.
fn decimal(key: &str, value: f64) -> Result<String, Error> {
    if !value.is_finite() {
        return Err(invalid(format!("{key} must be a finite number")));
    }
    let text = format!("{value:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    Ok(if text == "-0" {
        "0".to_owned()
    } else {
        text.to_owned()
    })
}

/// The bytes of a `model` request for `model`.
pub fn model_request(model: &Model<'_>) -> Result<Vec<u8>, Error> {
    let line_safe = |s: &str| !s.contains(['\n', '\r', '=']);
    if !line_safe(model.name) {
        return Err(invalid("a model name has no line breaks or '='"));
    }
    let mut out = format!("model {}\n", model.name);
    if let Some(of) = model.variant_of {
        if !line_safe(of) {
            return Err(invalid("a model name has no line breaks or '='"));
        }
        out.push_str(&format!("variant_of={of}\n"));
    }
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
    if let Some(wear) = model.wear {
        out.push_str(&format!("wear={}\n", wear.slot.word()));
        if let Some(armour) = wear.armour {
            if !line_safe(armour) {
                return Err(invalid("an armour name has no line breaks or '='"));
            }
            out.push_str(&format!("armour={armour}\n"));
        }
        if wear.glide {
            out.push_str("glide=true\n");
        }
        if !wear.fits.is_empty() {
            let fits: Vec<&str> = wear.fits.iter().map(|a| a.word()).collect();
            out.push_str(&format!("fits={}\n", fits.join(",")));
        }
    }
    model.behaviour.settings(&mut out)?;
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

/// The most models in one [`register_all`].
pub const MAX_BATCH: usize = 16;

/// The bytes of a `batch` request: the line `batch`, then each model's request as its length on a
/// line of its own and its bytes.
pub fn batch_request(models: &[Model<'_>]) -> Result<Vec<u8>, Error> {
    if models.is_empty() {
        return Err(invalid("a batch has at least one model"));
    }
    if models.len() > MAX_BATCH {
        return Err(Error {
            kind: ErrorKind::Limit,
            detail: format!("a batch has at most {MAX_BATCH} models"),
        });
    }
    let mut out = b"batch\n".to_vec();
    for model in models {
        let request = model_request(model)?;
        out.extend_from_slice(format!("{}\n", request.len()).as_bytes());
        out.extend_from_slice(&request);
    }
    Ok(out)
}

/// Reads the answer of a batch: the id and `ready` or `pending` of each model, in order.
fn registered_all(fields: &[String], count: usize) -> Result<Vec<Registered>, Error> {
    if fields.len() != count * 2 {
        return Err(Error {
            kind: ErrorKind::Other,
            detail: "an answer from custom-models that does not match the batch".to_owned(),
        });
    }
    Ok(fields
        .chunks(2)
        .map(|pair| Registered {
            id: pair[0].clone(),
            ready: pair[1] == "ready",
        })
        .collect())
}

/// Registers (or replaces) up to 16 models in one request, all or none: when one is refused,
/// nothing of the others is kept or sent, and the error names the refused one. The server's
/// resource pack is built once for all of them. Call it in `on_load`.
pub fn register_all(models: &[Model<'_>]) -> Result<Vec<Registered>, Error> {
    let fields = ask(&batch_request(models)?)?;
    registered_all(&fields, models.len())
}

/// The models of an armour set worn with the look `look`: each piece in its slot (head, chest,
/// legs, feet, each at most once), with its own names, files and what it gives.
pub fn armour_set_models<'a>(
    look: &'a str,
    pieces: &[(Worn, Model<'a>)],
) -> Result<Vec<Model<'a>>, Error> {
    if pieces.is_empty() || pieces.len() > 4 {
        return Err(invalid("an armour set has 1 to 4 pieces"));
    }
    let mut out: Vec<Model<'a>> = Vec::with_capacity(pieces.len());
    for (slot, model) in pieces {
        if *slot == Worn::Body {
            return Err(invalid(
                "an armour set is worn by a player: head, chest, legs, feet",
            ));
        }
        if out.iter().any(|m| m.wear.is_some_and(|w| w.slot == *slot)) {
            return Err(invalid(format!(
                "an armour set has one piece for the {}",
                slot.word()
            )));
        }
        out.push(model.worn(Wear::armour(*slot, look)));
    }
    Ok(out)
}

/// Registers a whole armour set in one request, all pieces or none: the helmet, chestplate,
/// leggings and boots that wear your armour look `look` (registered before with [`armour`]),
/// each with its own names and what it gives. The ids come back in the order of `pieces`.
pub fn armour_set(look: &str, pieces: &[(Worn, Model<'_>)]) -> Result<Vec<Registered>, Error> {
    register_all(&armour_set_models(look, pieces)?)
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
    /// A block with a front (a carved pumpkin, a jack o'lantern): the front, the other three sides, the top and
    /// the bottom.
    Front {
        front: &'a [u8],
        side: &'a [u8],
        up: &'a [u8],
        down: &'a [u8],
    },
}

/// A text in the four languages every text ships in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
        Faces::Front {
            front,
            side,
            up,
            down,
        } => with_files(
            settings,
            &[
                ("front.png", front),
                ("side.png", side),
                ("up.png", up),
                ("down.png", down),
            ],
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

/// Registers a language line for Java and Bedrock. Call it in `on_load`. `name` is `[a-z0-9_.]`, 1
/// to 64 characters, with no empty part between dots. The key comes back:
/// `custom-models.<your plugin>.<name>`, in custom-models' own section of the game's texts, so no
/// plugin can replace a vanilla text or another plugin's line.
pub fn line(name: &str, texts: &Texts<'_>) -> Result<String, Error> {
    let fields = ask(&line_request(name, texts)?)?;
    Ok(field(&fields, 0)?.to_owned())
}

/// The registered interface fonts of your plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredFonts {
    /// The id for [`ready`]: `<your plugin>.fonts`.
    pub id: String,
    /// Where the fonts are in the server's pack: images and moves in the font
    /// `<namespace>:<prefix>/ui`.
    pub namespace: String,
    pub prefix: String,
    /// Whether players already get them (in the server's resource pack). Until then a text in
    /// these fonts would show as empty boxes: draw the interface without them.
    pub ready: bool,
}

/// The bytes of a `fonts` request: the letters of each sheet's boxes, each place, then the images.
///
/// ```text
/// fonts
/// space.letters=4
/// row.letters=ABCDEFGH
/// row.letters=IJKLMNOP
/// place=image panel -6
/// place=sheet letters 14
///
/// image/panel.png 1234
/// <1234 bytes>
/// sheet/letters.png 567
/// <567 bytes>
/// ```
pub fn fonts_request(request: &font::Request<'_>) -> Result<Vec<u8>, Error> {
    fonts_request_with(request, &[])
}

/// A picture of the game's own interface that a fonts request replaces in the server's pack: a
/// boss bar's two sprites (`boss_bar/<colour>_background`, `boss_bar/<colour>_progress`), 182 x 5.
/// custom-models takes no other name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sprite<'a> {
    /// The sprite's name below `textures/gui/sprites/`, without `.png`.
    pub name: &'a str,
    pub png: &'a [u8],
}

/// [`fonts_request`] with sprites: after the fonts' images and sheets, each one as a
/// `sprite/<name>.png` file. A request may carry sprites and no places at all.
pub fn fonts_request_with(
    request: &font::Request<'_>,
    sprites: &[Sprite<'_>],
) -> Result<Vec<u8>, Error> {
    fonts_request_full(request, sprites, &[])
}

/// The one text of the game a fonts request may empty: the word the game writes over every chest,
/// furnace and inventory window.
pub const INVENTORY_WORD: &str = "container.inventory";

/// [`fonts_request_with`] and the game's texts to empty, each as a line `empty_text=<key>` in the
/// settings: custom-models then puts a lang file holding that key as `""` into the server's pack for
/// every language the game has, so every other text stays the game's. It takes no key but
/// [`INVENTORY_WORD`]. A request without the line is what it was before.
pub fn fonts_request_full(
    request: &font::Request<'_>,
    sprites: &[Sprite<'_>],
    empty_texts: &[&str],
) -> Result<Vec<u8>, Error> {
    use font::Place;
    let mut settings = "fonts\n".to_owned();
    for key in empty_texts {
        word_safe(key, "a text's key")?;
        settings.push_str(&format!("empty_text={key}\n"));
    }
    for sheet in &request.sheets {
        word_safe(sheet.name, "a sheet's name")?;
        settings.push_str(&format!("space.{}={}\n", sheet.name, sheet.space));
        for row in &sheet.rows {
            if row.contains(['\n', '\r']) {
                return Err(invalid("a row of letters has no line breaks"));
            }
            settings.push_str(&format!("row.{}={row}\n", sheet.name));
        }
    }
    for place in &request.places {
        let (kind, name, top) = match place {
            Place::Image { image, top } => ("image", image, top),
            Place::Sheet { sheet, top } => ("sheet", sheet, top),
        };
        word_safe(name, "a place's name")?;
        settings.push_str(&format!("place={kind} {name} {top}\n"));
    }
    let mut out = settings.into_bytes();
    out.push(b'\n');
    let images = request.images.iter().map(|i| ("image", i.name, i.png));
    let sheets = request.sheets.iter().map(|s| ("sheet", s.name, s.png));
    for (kind, name, png) in images.chain(sheets) {
        word_safe(name, "an image's name")?;
        out.extend_from_slice(format!("{kind}/{name}.png {}\n", png.len()).as_bytes());
        out.extend_from_slice(png);
    }
    for sprite in sprites {
        word_safe(sprite.name, "a sprite's name")?;
        out.extend_from_slice(
            format!("sprite/{}.png {}\n", sprite.name, sprite.png.len()).as_bytes(),
        );
        out.extend_from_slice(sprite.png);
    }
    Ok(out)
}

/// Registers the images and letter sheets your plugin draws an interface with (a window's panel,
/// its labels), replacing the ones it registered before. Java only: fonts do not exist on Bedrock.
/// Call it when the plugin loads, never at tick time: the request carries the images.
pub fn fonts(request: &font::Request<'_>) -> Result<RegisteredFonts, Error> {
    fonts_with_sprites(request, &[])
}

/// [`fonts`] with the game's own sprites the interface needs replaced (see [`Sprite`]). Without
/// places the request is only its sprites.
pub fn fonts_with_sprites(
    request: &font::Request<'_>,
    sprites: &[Sprite<'_>],
) -> Result<RegisteredFonts, Error> {
    fonts_full(request, sprites, &[])
}

/// [`fonts_with_sprites`] and the game's texts to empty (see [`fonts_request_full`]). The server
/// owner approves them in the server panel like every other file that changes the game's own.
pub fn fonts_full(
    request: &font::Request<'_>,
    sprites: &[Sprite<'_>],
    empty_texts: &[&str],
) -> Result<RegisteredFonts, Error> {
    let fields = ask(&fonts_request_full(request, sprites, empty_texts)?)?;
    Ok(RegisteredFonts {
        id: field(&fields, 0)?.to_owned(),
        namespace: field(&fields, 1)?.to_owned(),
        prefix: field(&fields, 2)?.to_owned(),
        ready: field(&fields, 3)? == "ready",
    })
}

/// A registered character.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredCharacter {
    /// The id for [`ready`]: `<your plugin>.<name>`.
    pub id: String,
    /// Its name in the server's pack.
    pub name: String,
    /// Whether players already get its pieces (in the server's resource pack). Until then a piece
    /// would show as a missing model: do not show the character.
    pub ready: bool,
}

/// The bytes of a `character` request: the line `character <name>` and the one file
/// `model.bbmodel`.
pub fn character_request(name: &str, bbmodel: &[u8]) -> Result<Vec<u8>, Error> {
    word_safe(name, "a character's name")?;
    Ok(with_files(
        format!("character {name}\n"),
        &[("model.bbmodel", bbmodel)],
    ))
}

/// Registers a character with limbs that move: a Blockbench project (`.bbmodel`, the format
/// Generic Model, textures embedded), replacing the one registered under this name before. Each
/// piece that moves becomes an item model in the server's Java pack. Give it a body with [`mob`]:
/// custom-models then plays its animations by what the mob does. A Bedrock player does not see it
/// yet. Call it when the plugin loads, never at tick time: the request carries the file.
pub fn character(name: &str, bbmodel: &[u8]) -> Result<RegisteredCharacter, Error> {
    let fields = ask(&character_request(name, bbmodel)?)?;
    Ok(RegisteredCharacter {
        id: field(&fields, 0)?.to_owned(),
        name: field(&fields, 1)?.to_owned(),
        ready: field(&fields, 2)? == "ready",
    })
}

/// A layer of an armour look, in vanilla's names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    /// A humanoid's head, chest and feet armour (a player, a zombie, an armour stand).
    Humanoid,
    /// A humanoid's leggings.
    HumanoidLeggings,
    /// Elytra-like wings on the chest (the item also needs the `glider` component).
    Wings,
    /// Wolf armour.
    WolfBody,
    /// Horse armour.
    HorseBody,
    /// A llama's carpet.
    LlamaBody,
}

impl Layer {
    /// Java's name of the layer: the picture's name in the request.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Humanoid => "humanoid",
            Self::HumanoidLeggings => "humanoid_leggings",
            Self::Wings => "wings",
            Self::WolfBody => "wolf_body",
            Self::HorseBody => "horse_body",
            Self::LlamaBody => "llama_body",
        }
    }
}

/// An armour look: the pictures an item worn in an armour slot is drawn with, in vanilla's layout.
#[derive(Debug, Clone, Copy)]
pub struct Armour<'a> {
    /// `[a-z0-9_]`, 1 to 48 characters. Its id becomes `<your plugin>.<name>`.
    pub name: &'a str,
    /// One picture per layer: `Humanoid` (head, chest and feet; 64 x 32), `HumanoidLeggings`
    /// (64 x 32), `Wings` (64 x 32), `WolfBody` (64 x 32), `HorseBody` (64 x 64), `LlamaBody`
    /// (128 x 64); 2 or 4 times those sizes for more detail.
    pub layers: &'a [(Layer, &'a [u8])],
    /// A dyed item (`dyed_color`) tints the look.
    pub dyeable: bool,
    /// The colour of an undyed item, `0xRRGGBB` (makes it dyeable); without it an undyed item
    /// shows no look, as in vanilla.
    pub undyed: Option<u32>,
}

/// A registered armour look.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredArmour {
    /// The id for [`ready`]: `<your plugin>.<name>`.
    pub id: String,
    /// What an item's `equippable.asset_id` names: custom-models sets it on the items that wear
    /// the look; an item you make yourself can name it too.
    pub asset_id: String,
    /// Whether players already get it (in the server's resource pack).
    pub ready: bool,
}

/// The bytes of an `armour` request.
pub fn armour_request(armour: &Armour<'_>) -> Result<Vec<u8>, Error> {
    word_safe(armour.name, "an armour look's name")?;
    let mut settings = format!("armour {}\n", armour.name);
    if armour.dyeable {
        settings.push_str("dyeable=true\n");
    }
    if let Some(colour) = armour.undyed {
        if colour > 0x00ff_ffff {
            return Err(invalid("an undyed colour is 0xRRGGBB"));
        }
        settings.push_str(&format!("undyed=#{colour:06X}\n"));
    }
    let names: Vec<String> = armour
        .layers
        .iter()
        .map(|(layer, _)| format!("{}.png", layer.word()))
        .collect();
    let files: Vec<(&str, &[u8])> = names
        .iter()
        .zip(armour.layers)
        .map(|(name, (_, png))| (name.as_str(), *png))
        .collect();
    Ok(with_files(settings, &files))
}

/// Registers (or replaces) an armour look. Register it before the models that wear it, when the
/// plugin loads: the request carries the pictures.
pub fn armour(armour: &Armour<'_>) -> Result<RegisteredArmour, Error> {
    let fields = ask(&armour_request(armour)?)?;
    Ok(RegisteredArmour {
        id: field(&fields, 0)?.to_owned(),
        asset_id: field(&fields, 1)?.to_owned(),
        ready: field(&fields, 2)? == "ready",
    })
}

/// Whether what you registered under `id` (a model, a retexture, a sound, a line, your fonts, a
/// character, an armour look) is
/// in the resource pack players get by now.
pub fn ready(id: &str) -> Result<bool, Error> {
    word_safe(id, "an id")?;
    let fields = ask(format!("ready {id}").as_bytes())?;
    Ok(field(&fields, 0)? == "ready")
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

/// The model id of `stack` (no request): `None` for any other item. It is the server's own
/// custom item id of the stack (`content::item_id`), the same lookup custom-models makes itself,
/// so a plugin and custom-models never disagree about a stack. Reading it needs no permission.
/// A creative-mode player can copy an item with all its data, so never treat the id as
/// proof of value on its own.
#[must_use]
pub fn model_id(stack: &ItemStack) -> Option<String> {
    id_in(strawberry_plugin_api::content::item_id(stack).as_deref())
}

/// The model id in a server custom item id: what follows `custom-models:`; `None` for an item of
/// another namespace.
#[must_use]
pub fn id_in(item_id: Option<&str>) -> Option<String> {
    item_id?
        .strip_prefix(PLUGIN)?
        .strip_prefix(':')
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
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
            wear: None,
            behaviour: Behaviour::NONE,
            variant_of: None,
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
            wear: None,
            behaviour: Behaviour::NONE,
            variant_of: None,
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
            retexture_request(
                "jack_o_lantern",
                &Faces::Front {
                    front: b"F",
                    side: b"S",
                    up: b"U",
                    down: b"D"
                }
            )
            .unwrap(),
            b"retexture jack_o_lantern\n\nfront.png 1\nFside.png 1\nSup.png 1\nUdown.png 1\nD"
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

    #[test]
    fn a_model_id_is_the_servers_custom_item_id_in_our_namespace() {
        assert_eq!(
            id_in(Some("custom-models:villager-bucket.villager_bucket")).as_deref(),
            Some("villager-bucket.villager_bucket")
        );
        // Another plugin's custom item, a look-alike namespace, no id: not a model.
        for other in [
            Some("shop:coin"),
            Some("custom-models-x:a.b"),
            Some("custom-modelsa.b"),
            Some("custom-models:"),
            Some("minecraft:paper"),
            None,
        ] {
            assert_eq!(id_in(other), None, "{other:?}");
        }
    }

    const PLATE: &[(&str, &[u8])] = &[("texture.png", b"P")];

    #[test]
    fn what_a_worn_model_gives_becomes_its_settings() {
        let chest = Model::new("plate_chest", &[("en", "Plate")], PLATE)
            .base("minecraft:iron_chestplate")
            .worn(Wear::armour(Worn::Chest, "plate"))
            .gives(
                Behaviour::NONE
                    .armor(8.0)
                    .armor_toughness(2.0)
                    .knockback_resistance(0.1)
                    .movement_speed(-0.05)
                    .durability(528)
                    .repair(Repair::Item("minecraft:iron_ingot"))
                    .enchantable(9)
                    .equip_sound(EquipSound::Iron),
            );
        assert_eq!(
            String::from_utf8(model_request(&chest).unwrap()).unwrap(),
            "model plate_chest\nname.en=Plate\nbase=minecraft:iron_chestplate\nwear=chest\narmour=plate\n\
             armor=8\narmor_toughness=2\nknockback_resistance=0.1\nmovement_speed=-0.05\ndurability=528\n\
             repair=minecraft:iron_ingot\nenchantable=9\nequip_sound=iron\n\ntexture.png 1\nP"
        );
        let wings = Model::new("wings", &[("en", "Wings")], PLATE)
            .worn(Wear::wings("feathers"))
            .gives(
                Behaviour::NONE
                    .durability(432)
                    .repair(Repair::Tag("minecraft:repairs_leather_armor"))
                    .equip_sound(EquipSound::Elytra),
            );
        assert_eq!(
            String::from_utf8(model_request(&wings).unwrap()).unwrap(),
            "model wings\nname.en=Wings\nwear=chest\narmour=feathers\nglide=true\ndurability=432\n\
             repair=#minecraft:repairs_leather_armor\nequip_sound=elytra\n\ntexture.png 1\nP"
        );
        let coat = Model::new("coat", &[("en", "Coat")], PLATE)
            .worn(Wear::animal("coat", &[Animal::Wolf, Animal::Horse]))
            .gives(
                Behaviour::NONE
                    .armor(11.0)
                    .unbreakable()
                    .equip_sound(EquipSound::Own("clank")),
            );
        assert_eq!(
            String::from_utf8(model_request(&coat).unwrap()).unwrap(),
            "model coat\nname.en=Coat\nwear=body\narmour=coat\nfits=wolf,horse\narmor=11\nunbreakable=true\n\
             equip_sound=own:clank\n\ntexture.png 1\nP"
        );
        // Decimals keep three places; what is not a number never leaves the plugin.
        let fine = Model::new("x", &[("en", "X")], PLATE)
            .worn(Wear::hat())
            .gives(Behaviour::NONE.armor(1.234_56).movement_speed(-0.000_1));
        let text = String::from_utf8(model_request(&fine).unwrap()).unwrap();
        assert!(text.contains("armor=1.235\nmovement_speed=0\n"), "{text}");
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let model = fine.gives(Behaviour::NONE.armor(bad));
            assert_eq!(model_request(&model).unwrap_err().kind, ErrorKind::Invalid);
        }
        // A repair id cannot carry a setting of its own.
        let smuggled = fine.gives(Behaviour::NONE.repair(Repair::Item("x\nstack=99")));
        assert_eq!(
            model_request(&smuggled).unwrap_err().kind,
            ErrorKind::Invalid
        );
    }

    #[test]
    fn a_batch_and_an_armour_set_are_one_request() {
        let head =
            Model::new("plate_head", &[("en", "Helmet")], PLATE).gives(Behaviour::NONE.armor(2.0));
        let feet =
            Model::new("plate_feet", &[("en", "Boots")], PLATE).gives(Behaviour::NONE.armor(1.0));
        let set = armour_set_models("plate", &[(Worn::Head, head), (Worn::Feet, feet)]).unwrap();
        let bytes = batch_request(&set).unwrap();
        let first = model_request(&set[0]).unwrap();
        let second = model_request(&set[1]).unwrap();
        let mut expected = format!("batch\n{}\n", first.len()).into_bytes();
        expected.extend_from_slice(&first);
        expected.extend_from_slice(format!("{}\n", second.len()).as_bytes());
        expected.extend_from_slice(&second);
        assert_eq!(bytes, expected);
        assert!(String::from_utf8_lossy(&first).contains("wear=head\narmour=plate\narmor=2\n"));
        assert!(String::from_utf8_lossy(&second).contains("wear=feet\narmour=plate\narmor=1\n"));
        // One piece per slot, only a player's slots, 1 to 4 pieces.
        for bad in [
            vec![(Worn::Head, head), (Worn::Head, feet)],
            vec![(Worn::Body, head)],
            vec![],
            vec![(Worn::Head, head); 5],
        ] {
            assert_eq!(
                armour_set_models("plate", &bad).unwrap_err().kind,
                ErrorKind::Invalid
            );
        }
        assert_eq!(batch_request(&[]).unwrap_err().kind, ErrorKind::Invalid);
        assert_eq!(
            batch_request(&[head; MAX_BATCH + 1]).unwrap_err().kind,
            ErrorKind::Limit
        );
        // The answer: the id and readiness of each, in order; anything else is not trusted.
        let fields: Vec<String> = ["a.x", "ready", "a.y", "pending"]
            .map(str::to_owned)
            .to_vec();
        assert_eq!(
            registered_all(&fields, 2).unwrap(),
            [
                Registered {
                    id: "a.x".to_owned(),
                    ready: true
                },
                Registered {
                    id: "a.y".to_owned(),
                    ready: false
                }
            ]
        );
        assert_eq!(
            registered_all(&fields, 3).unwrap_err().kind,
            ErrorKind::Other
        );
    }

    #[test]
    fn a_tool_a_food_a_sword_and_a_variant_are_requests_custom_models_reads() {
        const PICK: Tool<'static> = Tool::mines(Blocks::Tag("minecraft:mineable/pickaxe"))
            .speed(6.0)
            .drops(true)
            .damage_per_block(1)
            .rules(&[ToolRule {
                blocks: Blocks::Tag("minecraft:incorrect_for_iron_tool"),
                speed: None,
                drops: Some(false),
            }]);
        let pick = Model::new("pick", &[("en", "Pick")], PLATE)
            .gives(Behaviour::NONE.tool(PICK).durability(250));
        assert_eq!(
            String::from_utf8(model_request(&pick).unwrap()).unwrap(),
            "model pick\nname.en=Pick\ndurability=250\ntool.blocks=#minecraft:mineable/pickaxe\ntool.speed=6\n\
             tool.drops=true\ntool.1.blocks=#minecraft:incorrect_for_iron_tool\ntool.1.drops=false\n\
             tool.damage_per_block=1\n\ntexture.png 1\nP"
        );
        let stew = Model::new("stew", &[("en", "Stew")], PLATE).gives(
            Behaviour::NONE
                .food(
                    Food::new(6, 7.2)
                        .always()
                        .seconds(0.8)
                        .effects(&[Effect {
                            id: "speed",
                            seconds: 30,
                            level: 2,
                            chance: 0.5,
                        }])
                        .leftover("minecraft:bowl"),
                )
                .cooldown(2.0, Some("snacks")),
        );
        assert_eq!(
            String::from_utf8(model_request(&stew).unwrap()).unwrap(),
            "model stew\nname.en=Stew\nfood.nutrition=6\nfood.saturation=7.2\nfood.always=true\nfood.seconds=0.8\n\
             food.effect.1=speed 30 2 0.5\nfood.leftover=minecraft:bowl\ncooldown=2\ncooldown.group=snacks\n\ntexture.png 1\nP"
        );
        let sword = Model::new("sword", &[("en", "Sword")], PLATE).gives(
            Behaviour::NONE
                .attack_damage(6.0)
                .attack_speed(-2.4)
                .reach_entity(0.5)
                .rarity(Rarity::Epic)
                .glint(true)
                .lore(&[("en", &["A blade", "Sharp"]), ("tr", &["Bir kılıç"])])
                .enchantments(&[("sharpness", 5)])
                .fire_resistant()
                .hide(&[Hide::Enchantments]),
        );
        assert_eq!(
            String::from_utf8(model_request(&sword).unwrap()).unwrap(),
            "model sword\nname.en=Sword\nattack_damage=6\nattack_speed=-2.4\nreach.entity=0.5\nrarity=epic\nglint=true\n\
             lore.en.1=A blade\nlore.en.2=Sharp\nlore.tr.1=Bir kılıç\nenchant.sharpness=5\nfire_resistant=true\n\
             hide=enchantments\n\ntexture.png 1\nP"
        );
        // A variant names its source and only what differs; no files means the source's.
        let golden = Model::variant("golden_sword", "sword")
            .names(&[("en", "Golden Sword")])
            .gives(Behaviour::NONE.attack_damage(4.0));
        assert_eq!(
            String::from_utf8(model_request(&golden).unwrap()).unwrap(),
            "model golden_sword\nvariant_of=sword\nname.en=Golden Sword\nattack_damage=4\n\n"
        );
        // Nothing in a line of lore, an effect or a group can start a setting of its own.
        for bad in [
            sword.gives(Behaviour::NONE.lore(&[("en", &["a\nbase=minecraft:diamond"])])),
            sword.gives(Behaviour::NONE.cooldown(1.0, Some("a b"))),
            sword.gives(Behaviour::NONE.food(Food::new(1, 1.0).effects(&[Effect {
                id: "speed\nstack=99",
                seconds: 1,
                level: 1,
                chance: 1.0,
            }]))),
            sword.gives(Behaviour::NONE.tool(Tool::mines(Blocks::Ids(&["stone", "dirt\nx=y"])))),
            sword.gives(Behaviour::NONE.attack_damage(f64::NAN)),
        ] {
            assert_eq!(model_request(&bad).unwrap_err().kind, ErrorKind::Invalid);
        }
    }
}

/// The images, sheets and places of a [`fonts`] request.
pub mod font;

/// Recipes for model items (`recipe` requests).
mod recipe;
pub use recipe::{Category, Ingredient, Output, Recipe, Shape, Station, recipe, recipe_request};

/// Drops of blocks and fishing (`drops` requests).
mod drop_rules;
pub use drop_rules::{DropItem, Drops, Silk, Source, What, drops, drops_request};

/// Pictures in text (`picture` requests).
mod picture;
pub use picture::{Picture, RegisteredPicture, picture, picture_request};

mod registries;
pub use registries::{
    Disc, Entry as RegistryEntry, Look, Painting, TrimMaterial, TrimPattern, disc, disc_request,
    painting, painting_request, trim_material, trim_material_request, trim_pattern,
    trim_pattern_request,
};

/// Custom blocks (`block` requests) and the notices about them.
mod block;
pub use block::{
    ALLOW, Block, BlockDrop, BlockFeel, BlockLook, BlockNotice, BlockShape, BlockSound,
    BlockTier, BlockTool, DENY, NoticeKind, StateLook, block, block_notice, block_request,
    block_state, block_state_request,
};

/// Particle emitters of a piece (`Furniture::emit`) or a place (`emitter` requests).
mod emitter;
pub use emitter::{Emit, EmitLook, emitter, emitter_remove, emitter_remove_request, emitter_request};

/// Crops (`crop` requests): a seed that grows through looks on its soil.
mod crop;
pub use crop::{Crop, CropHarvest, crop, crop_request};

/// Furniture: a model placed in the world with its item (`furniture` requests), its looks
/// (`furniture-state`) and the notices its script hears (answered with [`ALLOW`] or [`DENY`]).
mod furniture;
pub use furniture::{
    Furniture, FurnitureNotice, FurnitureNoticeKind, HitBox, Place, Trade, TradeItem, Turn,
    furniture, furniture_notice, furniture_request, furniture_state, furniture_state_request,
};

/// Modelled mobs: a real invisible mob carries one of your characters (`mob` requests, `mob-spawn`).
mod mob;
pub use mob::{
    Carrier, Mob, MobSound, MobState, mob, mob_request, mob_spawn, mob_spawn_request,
};

/// Vehicles: a model placed with its item that a player gets on, drives and fuels (`vehicle`
/// requests).
mod vehicle;
pub use vehicle::{Fuel, FuelItem, Vehicle, vehicle, vehicle_request};
