//! Drops: what a vanilla block, a mob or fishing drops besides vanilla's, your models or vanilla
//! items.
//!
//! ```ignore
//! use custom_models_api::{DropItem, Drops, Silk, What, drops};
//!
//! // in on_load, after registering the models it names:
//! drops(
//!     &Drops::block(
//!         "ruby_ore",
//!         "minecraft:deepslate_diamond_ore",
//!         &[DropItem::new(What::Mine("ruby"), 1, 2, 2500).fortune()],
//!     )
//!     .silk(Silk::Forbid)
//!     .worlds(&["world"]),
//! )?;
//! drops(&Drops::fishing("lucky_catch", &[DropItem::new(What::Mine("pearl"), 1, 1, 50)]).xp(1, 3))?;
//! drops(
//!     &Drops::mob("zombie_gems", "minecraft:zombie", &[DropItem::new(What::Mine("ruby"), 0, 1, 2500).looting()])
//!         .no_spawner(),
//! )?;
//! ```
//!
//! A rule is evaluated only when its event happens. Replacing vanilla's drops (a block's, a mob's
//! or a catch) needs the server owner's word for your plugin: he lists it in custom-models'
//! setting `replace_drops_scripts`; until then it answers `Unavailable`. Register your rules in every `on_load`.

use super::{Error, Registered, ask, field, invalid};

/// Where drops come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source<'a> {
    /// A vanilla block, `minecraft:<block>`.
    Block(&'a str),
    /// One of your own custom blocks ([`crate::block`]), by its name.
    MyBlock(&'a str),
    /// A vanilla mob type, `minecraft:<entity type>`.
    Mob(&'a str),
    /// One of your own modelled mobs ([`crate::mob`]), by its name. Its rules are all it drops:
    /// its carrier's vanilla loot and the rules of the carrier's type never apply to it.
    MyMob(&'a str),
    Fishing,
}

/// What drops.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum What<'a> {
    /// A vanilla item, `minecraft:<item>`.
    Item(&'a str),
    /// One of your own models, by its name.
    Mine(&'a str),
}

/// One item of a rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DropItem<'a> {
    pub what: What<'a>,
    /// 0 to 64, `min` at most `max`, `max` at least 1; at most one stack drops.
    pub min: u8,
    pub max: u8,
    /// In 1/10000: 10000 always, 2500 a quarter of the time.
    pub chance: u16,
    /// Fortune adds, as vanilla's ore bonus (blocks), or Looting, as vanilla's mob bonus (mobs).
    pub fortune: bool,
}

impl<'a> DropItem<'a> {
    #[must_use]
    pub const fn new(what: What<'a>, min: u8, max: u8, chance: u16) -> Self {
        Self {
            what,
            min,
            max,
            chance,
            fortune: false,
        }
    }

    #[must_use]
    pub const fn fortune(mut self) -> Self {
        self.fortune = true;
        self
    }

    /// Looting adds, as vanilla's mob bonus (mob rules); the same flag as [`Self::fortune`].
    #[must_use]
    pub const fn looting(self) -> Self {
        self.fortune()
    }
}

/// Silk touch on the tool (blocks).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Silk {
    Any,
    Require,
    Forbid,
}

/// A drop rule of your plugin. Its id is `drops:<your plugin>.<name>`; the same name again
/// replaces it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Drops<'a> {
    /// `[a-z0-9_]`, 1 to 48 characters.
    pub name: &'a str,
    pub source: Source<'a>,
    /// 1 to 16 items.
    pub items: &'a [DropItem<'a>],
    pub silk: Silk,
    /// Replaces vanilla's drops instead of adding to them (needs the server owner's word; for your
    /// own custom block it replaces the block's own drop and needs nothing).
    pub replace: bool,
    /// Experience, `min..=max`, 0 to 1000, added to what the break, the death or the catch drops
    /// (a block's only for a player's break in survival, a mob's only when a player gets the kill).
    pub xp: Option<(u16, u16)>,
    /// Blocks: the main hand must hold one of these (`minecraft:<item>`, your model's name, or
    /// `model:<id>`), up to 16; any tool when empty.
    pub tools: &'a [&'a str],
    /// Mobs: mobs from a mob spawner count too (true by default).
    pub spawner: bool,
    /// Up to 16 world names; every world when empty.
    pub worlds: &'a [&'a str],
    /// Up to 16 biome ids; every biome when empty.
    pub biomes: &'a [&'a str],
}

impl<'a> Drops<'a> {
    #[must_use]
    pub const fn block(name: &'a str, block: &'a str, items: &'a [DropItem<'a>]) -> Self {
        Self::of(name, Source::Block(block), items)
    }

    /// A rule for one of your own custom blocks: it adds to the block's own drop, or replaces it
    /// with [`Self::replace`] (no word from the server owner needed: the block is yours).
    #[must_use]
    pub const fn my_block(name: &'a str, block: &'a str, items: &'a [DropItem<'a>]) -> Self {
        Self::of(name, Source::MyBlock(block), items)
    }

    #[must_use]
    pub const fn fishing(name: &'a str, items: &'a [DropItem<'a>]) -> Self {
        Self::of(name, Source::Fishing, items)
    }

    /// A rule for one of your own modelled mobs ([`crate::mob`]), by its name.
    #[must_use]
    pub const fn my_mob(name: &'a str, mob: &'a str, items: &'a [DropItem<'a>]) -> Self {
        Self::of(name, Source::MyMob(mob), items)
    }

    /// A rule for a vanilla mob type, `minecraft:<entity type>`.
    #[must_use]
    pub const fn mob(name: &'a str, kind: &'a str, items: &'a [DropItem<'a>]) -> Self {
        Self::of(name, Source::Mob(kind), items)
    }

    #[must_use]
    pub const fn of(name: &'a str, source: Source<'a>, items: &'a [DropItem<'a>]) -> Self {
        Self {
            name,
            source,
            items,
            silk: Silk::Any,
            replace: false,
            xp: None,
            tools: &[],
            spawner: true,
            worlds: &[],
            biomes: &[],
        }
    }

    /// Blocks: only while the main hand holds one of `tools`.
    #[must_use]
    pub const fn tools(mut self, tools: &'a [&'a str]) -> Self {
        self.tools = tools;
        self
    }

    /// Mobs: not for mobs from a mob spawner.
    #[must_use]
    pub const fn no_spawner(mut self) -> Self {
        self.spawner = false;
        self
    }

    #[must_use]
    pub const fn silk(mut self, silk: Silk) -> Self {
        self.silk = silk;
        self
    }

    #[must_use]
    pub const fn replace(mut self) -> Self {
        self.replace = true;
        self
    }

    #[must_use]
    pub const fn xp(mut self, min: u16, max: u16) -> Self {
        self.xp = Some((min, max));
        self
    }

    #[must_use]
    pub const fn worlds(mut self, worlds: &'a [&'a str]) -> Self {
        self.worlds = worlds;
        self
    }

    #[must_use]
    pub const fn biomes(mut self, biomes: &'a [&'a str]) -> Self {
        self.biomes = biomes;
        self
    }
}

/// One word on one line: no line break, space, `=` or `,`.
fn word(text: &str, what: &str) -> Result<(), Error> {
    if text.is_empty() || text.contains(['\n', '\r', ' ', '=', ',']) {
        return Err(invalid(format!("{what} is one word")));
    }
    Ok(())
}

/// `chance` in 1/10000 as the request writes it: `0` to `1` with at most 4 decimals.
fn chance_text(chance: u16) -> Result<String, Error> {
    match chance {
        10_000 => Ok("1".to_owned()),
        0..10_000 => Ok(format!("0.{chance:04}")),
        _ => Err(invalid("a chance is 0 to 10000 (in 1/10000)")),
    }
}

/// The bytes of a `drops` request for `rule`.
pub fn drops_request(rule: &Drops<'_>) -> Result<Vec<u8>, Error> {
    word(rule.name, "a rule name")?;
    let from = match rule.source {
        Source::Block(id) => {
            word(id, "a block")?;
            format!("block:{id}")
        }
        Source::MyBlock(name) => {
            word(name, "a block")?;
            if name.contains(':') {
                return Err(invalid("a block of yours is named without a namespace"));
            }
            format!("block:{name}")
        }
        Source::Mob(id) => {
            word(id, "a mob type")?;
            format!("mob:{id}")
        }
        Source::MyMob(name) => {
            word(name, "a mob")?;
            if name.contains(':') {
                return Err(invalid("a mob of yours is named without a namespace"));
            }
            format!("mob:{name}")
        }
        Source::Fishing => "fishing".to_owned(),
    };
    let mut out = format!("drops {}\nfrom={from}\n", rule.name);
    for (n, item) in rule.items.iter().enumerate() {
        let what = match item.what {
            What::Item(id) | What::Mine(id) => id,
        };
        word(what, "an item")?;
        if matches!(item.what, What::Mine(_)) && what.contains(':') {
            return Err(invalid("a model of yours is named without a namespace"));
        }
        out.push_str(&format!(
            "item.{}={what} {} {} {}{}\n",
            n + 1,
            item.min,
            item.max,
            chance_text(item.chance)?,
            match (item.fortune, rule.source) {
                (false, _) => "",
                (true, Source::Mob(_) | Source::MyMob(_)) => " looting",
                (true, _) => " fortune",
            }
        ));
    }
    match rule.silk {
        Silk::Any => {}
        Silk::Require => out.push_str("silk=require\n"),
        Silk::Forbid => out.push_str("silk=forbid\n"),
    }
    if rule.replace {
        out.push_str("replace=true\n");
    }
    if let Some((min, max)) = rule.xp {
        out.push_str(&format!("xp={min}..{max}\n"));
    }
    if !rule.tools.is_empty() {
        for tool in rule.tools {
            word(tool, "a tool")?;
        }
        out.push_str(&format!("tool={}\n", rule.tools.join(",")));
    }
    if !rule.spawner {
        out.push_str("spawner=false\n");
    }
    for (key, names) in [("worlds", rule.worlds), ("biomes", rule.biomes)] {
        if names.is_empty() {
            continue;
        }
        for name in names {
            word(name, key)?;
        }
        out.push_str(&format!("{key}={}\n", names.join(",")));
    }
    Ok(out.into_bytes())
}

/// Registers (or replaces) one of your drop rules. Call it in `on_load`, after the models it
/// drops.
pub fn drops(rule: &Drops<'_>) -> Result<Registered, Error> {
    let fields = ask(&drops_request(rule)?)?;
    Ok(Registered {
        id: field(&fields, 0)?.to_owned(),
        ready: field(&fields, 1)? == "ready",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_requests_carry_every_setting() {
        let items = [
            DropItem::new(What::Mine("ruby"), 1, 2, 2500).fortune(),
            DropItem::new(What::Item("minecraft:stick"), 0, 1, 10_000),
            DropItem::new(What::Mine("pearl"), 1, 1, 1),
        ];
        let rule = Drops::block("ruby_ore", "minecraft:diamond_ore", &items)
            .silk(Silk::Forbid)
            .worlds(&["world", "mining"])
            .biomes(&["minecraft:lush_caves"]);
        assert_eq!(
            String::from_utf8(drops_request(&rule).unwrap()).unwrap(),
            "drops ruby_ore\nfrom=block:minecraft:diamond_ore\nitem.1=ruby 1 2 0.2500 fortune\nitem.2=minecraft:stick 0 1 1\nitem.3=pearl 1 1 0.0001\nsilk=forbid\nworlds=world,mining\nbiomes=minecraft:lush_caves\n"
        );
        let ore = Drops::my_block("ore_gems", "ruby_ore", &items[..1]).replace();
        assert_eq!(
            String::from_utf8(drops_request(&ore).unwrap()).unwrap(),
            "drops ore_gems\nfrom=block:ruby_ore\nitem.1=ruby 1 2 0.2500 fortune\nreplace=true\n"
        );
        assert!(drops_request(&Drops::my_block("x", "gems:ruby_ore", &items[..1])).is_err());
        let mob = Drops::mob("zombie_gems", "minecraft:zombie", &items[..1])
            .no_spawner()
            .xp(1, 2);
        assert_eq!(
            String::from_utf8(drops_request(&mob).unwrap()).unwrap(),
            "drops zombie_gems\nfrom=mob:minecraft:zombie\nitem.1=ruby 1 2 0.2500 looting\nxp=1..2\nspawner=false\n"
        );
        let dragon = Drops::my_mob("dragon_loot", "dragon", &items[..1]);
        assert_eq!(
            String::from_utf8(drops_request(&dragon).unwrap()).unwrap(),
            "drops dragon_loot\nfrom=mob:dragon\nitem.1=ruby 1 2 0.2500 looting\n"
        );
        assert!(drops_request(&Drops::my_mob("x", "gems:dragon", &items[..1])).is_err());
        let tool = Drops::block("ore", "minecraft:diamond_ore", &items[1..2])
            .tools(&["minecraft:iron_pickaxe", "ruby_pickaxe"]);
        assert_eq!(
            String::from_utf8(drops_request(&tool).unwrap()).unwrap(),
            "drops ore\nfrom=block:minecraft:diamond_ore\nitem.1=minecraft:stick 0 1 1\ntool=minecraft:iron_pickaxe,ruby_pickaxe\n"
        );
        let fish = Drops::fishing("catch", &items[1..2]).xp(1, 3).replace();
        assert_eq!(
            String::from_utf8(drops_request(&fish).unwrap()).unwrap(),
            "drops catch\nfrom=fishing\nitem.1=minecraft:stick 0 1 1\nreplace=true\nxp=1..3\n"
        );
    }

    #[test]
    fn line_breaks_and_separators_cannot_add_settings_to_a_rule() {
        let ok = [DropItem::new(What::Mine("a"), 1, 1, 1)];
        let bad_items = [
            [DropItem::new(What::Mine("a\nreplace=true"), 1, 1, 1)],
            [DropItem::new(What::Mine("a b"), 1, 1, 1)],
            [DropItem::new(What::Mine("other:a"), 1, 1, 1)],
            [DropItem::new(What::Mine("a"), 1, 1, 10_001)],
        ];
        for items in &bad_items {
            let e = drops_request(&Drops::fishing("x", items)).unwrap_err();
            assert_eq!(e.kind, crate::ErrorKind::Invalid, "{items:?}");
        }
        let bad = [
            Drops::fishing("x\nfrom=fishing", &ok),
            Drops::block("x", "minecraft:stone\nreplace=true", &ok),
            Drops::fishing("x", &ok).worlds(&["a,b"]),
            Drops::fishing("x", &ok).biomes(&["a\nreplace=true"]),
            Drops::block("x", "minecraft:stone", &ok).tools(&["a,b"]),
            Drops::block("x", "minecraft:stone", &ok).tools(&["a\nreplace=true"]),
        ];
        for rule in &bad {
            let e = drops_request(rule).unwrap_err();
            assert_eq!(e.kind, crate::ErrorKind::Invalid, "{rule:?}");
        }
    }
}
