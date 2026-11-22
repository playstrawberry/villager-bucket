//! Modelled mobs: one of your characters drawn on a real, invisible mob that carries its hitbox,
//! health, movement and AI.
//!
//! ```ignore
//! use custom_models_api::{Carrier, Mob, MobSound, MobState, mob, mob_spawn};
//!
//! // in on_load, after registering the character, sounds and line it names:
//! mob(
//!     &Mob::new("dragon", "walker")
//!         .carrier(Carrier::Ravager)
//!         .health(60.0)
//!         .attack(8.0)
//!         .scale(1.5)
//!         .animations(&[
//!             (MobState::Idle, "idle"),
//!             (MobState::Walk, "walk"),
//!             (MobState::Attack, "bite"),
//!             (MobState::Death, "die"),
//!         ])
//!         .hit_at(0.4)
//!         .sounds(&[(MobSound::Ambient, "growl")])
//!         .name_line("dragon_name"),
//! )?;
//!
//! // a mob of yours, where you want it:
//! let uuid = mob_spawn("gems.dragon", "world", [10.5, 64.0, -3.0], 90.0)?;
//! ```
//!
//! The mob's drops are your own [`Drops::my_mob`](crate::Drops::my_mob) rules; the carrier's
//! vanilla loot never drops. A mob whose script is unloaded stands still where it is and costs
//! nothing until the script registers it again. Limits per script and per server apply, and a
//! spawn over them is refused with an answer, never queued.

use super::furniture::{name, number};
use super::{Error, Registered, ask, field, invalid};

/// The real mob that carries a model: a short list of safe ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Carrier {
    Husk,
    Zombie,
    Skeleton,
    Stray,
    Spider,
    CaveSpider,
    Cow,
    Pig,
    Wolf,
    PolarBear,
    IronGolem,
    Ravager,
    Ghast,
}

impl Carrier {
    const fn word(self) -> &'static str {
        match self {
            Self::Husk => "husk",
            Self::Zombie => "zombie",
            Self::Skeleton => "skeleton",
            Self::Stray => "stray",
            Self::Spider => "spider",
            Self::CaveSpider => "cave_spider",
            Self::Cow => "cow",
            Self::Pig => "pig",
            Self::Wolf => "wolf",
            Self::PolarBear => "polar_bear",
            Self::IronGolem => "iron_golem",
            Self::Ravager => "ravager",
            Self::Ghast => "ghast",
        }
    }
}

/// What a mob is doing, each with its own animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MobState {
    /// Looping, up to 20 seconds.
    Idle,
    /// Looping, up to 20 seconds.
    Walk,
    /// Once, up to 3 seconds.
    Attack,
    /// Once, up to 3 seconds.
    Hurt,
    /// Once, up to 2 seconds; its last pose should hold.
    Death,
}

impl MobState {
    const fn word(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Walk => "walk",
            Self::Attack => "attack",
            Self::Hurt => "hurt",
            Self::Death => "death",
        }
    }
}

/// When a mob of yours makes a sound of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MobSound {
    Ambient,
    Hurt,
    Death,
    Step,
    Attack,
}

impl MobSound {
    const fn word(self) -> &'static str {
        match self {
            Self::Ambient => "ambient",
            Self::Hurt => "hurt",
            Self::Death => "death",
            Self::Step => "step",
            Self::Attack => "attack",
        }
    }
}

/// A modelled mob of your script. Its id is `<your script>.<name>`; the same name again replaces
/// it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mob<'a> {
    /// `[a-z0-9_]`, 1 to 48 characters.
    pub name: &'a str,
    /// One of your characters, by its name: its pieces are the mob's look.
    pub character: &'a str,
    /// The real mob under it; a husk when not set.
    pub carrier: Option<Carrier>,
    /// 1 to 1024; the carrier's own when `None`.
    pub health: Option<f32>,
    /// Movement speed, 0.01 to 2.
    pub speed: Option<f32>,
    /// Attack damage, 0 to 100.
    pub attack: Option<f32>,
    /// 0 to 30.
    pub armor: Option<f32>,
    /// 0 to 1.
    pub knockback_resistance: Option<f32>,
    /// 1 to 64.
    pub follow_range: Option<f32>,
    /// The model's size on its carrier, 0.25 to 8.
    pub scale: f32,
    /// Raises the model on its carrier, -4 to 4 blocks.
    pub lift: f32,
    /// One of the character's animations per state.
    pub animations: &'a [(MobState, &'a str)],
    /// Seconds into the attack on which the hit lands, 0 to 1; it needs an attack animation.
    pub hit: Option<f32>,
    /// One of your sounds per kind, by name.
    pub sounds: &'a [(MobSound, &'a str)],
    /// One of your `line`s, by name: the nametag, in each player's language.
    pub name_line: Option<&'a str>,
    /// Blocks over the carrier's feet; over its top when `None`.
    pub name_height: Option<f32>,
}

impl<'a> Mob<'a> {
    /// A husk-carried mob of `character` at its own size, with no animation and no sound.
    #[must_use]
    pub const fn new(name: &'a str, character: &'a str) -> Self {
        Self {
            name,
            character,
            carrier: None,
            health: None,
            speed: None,
            attack: None,
            armor: None,
            knockback_resistance: None,
            follow_range: None,
            scale: 1.0,
            lift: 0.0,
            animations: &[],
            hit: None,
            sounds: &[],
            name_line: None,
            name_height: None,
        }
    }

    #[must_use]
    pub const fn carrier(mut self, carrier: Carrier) -> Self {
        self.carrier = Some(carrier);
        self
    }

    #[must_use]
    pub const fn health(mut self, health: f32) -> Self {
        self.health = Some(health);
        self
    }

    #[must_use]
    pub const fn speed(mut self, speed: f32) -> Self {
        self.speed = Some(speed);
        self
    }

    #[must_use]
    pub const fn attack(mut self, damage: f32) -> Self {
        self.attack = Some(damage);
        self
    }

    #[must_use]
    pub const fn armor(mut self, armor: f32) -> Self {
        self.armor = Some(armor);
        self
    }

    #[must_use]
    pub const fn knockback_resistance(mut self, resistance: f32) -> Self {
        self.knockback_resistance = Some(resistance);
        self
    }

    #[must_use]
    pub const fn follow_range(mut self, blocks: f32) -> Self {
        self.follow_range = Some(blocks);
        self
    }

    #[must_use]
    pub const fn scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }

    #[must_use]
    pub const fn lift(mut self, lift: f32) -> Self {
        self.lift = lift;
        self
    }

    #[must_use]
    pub const fn animations(mut self, animations: &'a [(MobState, &'a str)]) -> Self {
        self.animations = animations;
        self
    }

    /// The hit of an attack lands `seconds` into the attack animation.
    #[must_use]
    pub const fn hit_at(mut self, seconds: f32) -> Self {
        self.hit = Some(seconds);
        self
    }

    #[must_use]
    pub const fn sounds(mut self, sounds: &'a [(MobSound, &'a str)]) -> Self {
        self.sounds = sounds;
        self
    }

    #[must_use]
    pub const fn name_line(mut self, line: &'a str) -> Self {
        self.name_line = Some(line);
        self
    }

    #[must_use]
    pub const fn name_height(mut self, blocks: f32) -> Self {
        self.name_height = Some(blocks);
        self
    }
}

/// The bytes of a `mob` request for `m`.
pub fn mob_request(m: &Mob<'_>) -> Result<Vec<u8>, Error> {
    name(m.name, "a mob name")?;
    name(m.character, "a character")?;
    let mut out = format!("mob {}\ncharacter={}\n", m.name, m.character);
    if let Some(carrier) = m.carrier {
        out.push_str(&format!("carrier={}\n", carrier.word()));
    }
    for (key, value) in [
        ("health", m.health),
        ("speed", m.speed),
        ("attack", m.attack),
        ("armor", m.armor),
        ("knockback_resistance", m.knockback_resistance),
        ("follow_range", m.follow_range),
    ] {
        if let Some(value) = value {
            out.push_str(&format!("{key}={}\n", number(value, key)?));
        }
    }
    out.push_str(&format!("scale={}\n", number(m.scale, "scale")?));
    out.push_str(&format!("lift={}\n", number(m.lift, "lift")?));
    for (state, animation) in m.animations {
        name(animation, "an animation")?;
        out.push_str(&format!("anim.{}={animation}\n", state.word()));
    }
    if let Some(seconds) = m.hit {
        out.push_str(&format!("attack.hit={}\n", number(seconds, "the hit")?));
    }
    for (kind, sound) in m.sounds {
        name(sound, "a sound")?;
        out.push_str(&format!("sound.{}={sound}\n", kind.word()));
    }
    if let Some(line) = m.name_line {
        name(line, "a line")?;
        out.push_str(&format!("name={line}\n"));
        if let Some(height) = m.name_height {
            out.push_str(&format!("name.height={}\n", number(height, "name.height")?));
        }
    } else if m.name_height.is_some() {
        return Err(invalid("name_height goes with name_line"));
    }
    Ok(out.into_bytes())
}

/// Registers (or replaces) one of your mobs. Call it in `on_load`, after the character, sounds
/// and line it names. Mobs already in the world work again once it is registered.
pub fn mob(m: &Mob<'_>) -> Result<Registered, Error> {
    let fields = ask(&mob_request(m)?)?;
    Ok(Registered {
        id: field(&fields, 0)?.to_owned(),
        ready: field(&fields, 1)? == "ready",
    })
}

/// The bytes of a `mob-spawn` request.
pub fn mob_spawn_request(mob: &str, world: &str, at: [f64; 3], yaw: f32) -> Result<Vec<u8>, Error> {
    if mob.is_empty() || mob.len() > 128 || mob.contains(char::is_whitespace) {
        return Err(invalid("a mob id is one word: <your script>.<name>"));
    }
    if world.is_empty()
        || world.len() > 64
        || !world
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
    {
        return Err(invalid("a world name is letters, digits, _ - and ."));
    }
    if at.iter().any(|v| !v.is_finite()) || !yaw.is_finite() {
        return Err(invalid("a place and a turn are finite numbers"));
    }
    let [x, y, z] = at;
    Ok(format!("mob-spawn {mob} {world} {x:.3} {y:.3} {z:.3} {yaw:.1}").into_bytes())
}

/// Puts one of your mobs in `world` at `at`, facing `yaw`; the new mob's uuid. Only your own
/// mobs. Refused (`limit`) when your script or the server holds all the mobs it may.
pub fn mob_spawn(mob: &str, world: &str, at: [f64; 3], yaw: f32) -> Result<String, Error> {
    let fields = ask(&mob_spawn_request(mob, world, at, yaw)?)?;
    Ok(field(&fields, 0)?.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_carries_every_setting() {
        let m = Mob::new("dragon", "walker")
            .carrier(Carrier::Ravager)
            .health(60.0)
            .speed(0.25)
            .attack(8.0)
            .armor(2.0)
            .knockback_resistance(0.5)
            .follow_range(24.0)
            .scale(1.5)
            .lift(0.25)
            .animations(&[
                (MobState::Idle, "idle"),
                (MobState::Walk, "walk"),
                (MobState::Attack, "bite"),
                (MobState::Hurt, "flinch"),
                (MobState::Death, "die"),
            ])
            .hit_at(0.4)
            .sounds(&[(MobSound::Ambient, "growl"), (MobSound::Step, "stomp")])
            .name_line("dragon_name")
            .name_height(2.6);
        assert_eq!(
            String::from_utf8(mob_request(&m).unwrap()).unwrap(),
            "mob dragon\ncharacter=walker\ncarrier=ravager\nhealth=60\nspeed=0.25\nattack=8\narmor=2\nknockback_resistance=0.5\nfollow_range=24\nscale=1.5\nlift=0.25\nanim.idle=idle\nanim.walk=walk\nanim.attack=bite\nanim.hurt=flinch\nanim.death=die\nattack.hit=0.4\nsound.ambient=growl\nsound.step=stomp\nname=dragon_name\nname.height=2.6\n"
        );
        assert_eq!(
            String::from_utf8(mob_request(&Mob::new("rat", "walker")).unwrap()).unwrap(),
            "mob rat\ncharacter=walker\nscale=1\nlift=0\n"
        );
    }

    #[test]
    fn line_breaks_and_odd_numbers_cannot_add_settings() {
        for m in [
            Mob::new("a\nhealth=1", "walker"),
            Mob::new("a", "walker\ncarrier=ghast"),
            Mob::new("a", "walker").animations(&[(MobState::Idle, "idle\nattack=100")]),
            Mob::new("a", "walker").sounds(&[(MobSound::Hurt, "a b")]),
            Mob::new("a", "walker").name_line("x=y"),
            Mob::new("a", "walker").health(f32::NAN),
            Mob::new("a", "walker").scale(f32::INFINITY),
            Mob::new("a", "walker").name_height(2.0),
        ] {
            assert_eq!(
                mob_request(&m).unwrap_err().kind,
                crate::ErrorKind::Invalid,
                "{m:?}"
            );
        }
    }

    #[test]
    fn a_spawn_request_names_a_mob_a_world_and_a_place() {
        assert_eq!(
            mob_spawn_request("gems.dragon", "world", [10.5, 64.0, -3.25], 90.0).unwrap(),
            b"mob-spawn gems.dragon world 10.500 64.000 -3.250 90.0"
        );
        for bad in [
            mob_spawn_request("", "world", [0.0; 3], 0.0),
            mob_spawn_request("gems.dragon x", "world", [0.0; 3], 0.0),
            mob_spawn_request("gems.dragon", "wor ld", [0.0; 3], 0.0),
            mob_spawn_request("gems.dragon", "../world", [0.0; 3], 0.0),
            mob_spawn_request("gems.dragon", "world", [f64::NAN, 0.0, 0.0], 0.0),
            mob_spawn_request("gems.dragon", "world", [0.0; 3], f32::INFINITY),
        ] {
            assert_eq!(bad.unwrap_err().kind, crate::ErrorKind::Invalid);
        }
    }
}
