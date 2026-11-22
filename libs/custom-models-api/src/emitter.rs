//! Particle emitters: a piece of furniture or a place that gives off particles at a set rate,
//! only while a player is near enough to see them (`emit.<n>` lines of a furniture definition and
//! the `emitter` request).
//!
//! ```ignore
//! use custom_models_api::{Emit, EmitLook, emitter, emitter_remove};
//!
//! // A red dust, 3 particles every second, a block above the place, spread over 0.3 blocks.
//! let fire = Emit {
//!     look: EmitLook::Dust { color: 0xFF0000, size: 1.5 },
//!     count: 3,
//!     every: 20,
//!     offset: [0.0, 1.0, 0.0],
//!     spread: 0.3,
//! };
//! emitter("campfire", "world", [10.5, 64.0, -3.5], &fire)?;
//! // ... and when it is no longer wanted (it also goes when your script is unloaded):
//! emitter_remove("campfire")?;
//! ```
//!
//! On a piece, `Furniture::emit` takes the same [`Emit`]s (at most 4); the offset is from the
//! piece's look, in world directions. A script has at most 64 emitters and sends at most 200
//! particles a second; over that an emission is skipped, and the script is told nothing.

use crate::{Error, ask, invalid};

/// The most `emit` lines of one piece.
pub const MAX_PER_PIECE: usize = 4;

/// What an emitter gives off. A particle that carries options has its own case; every other
/// particle is `Plain` with its Minecraft name in `snake_case` (`flame`, `smoke`, `end_rod`, ...).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EmitLook<'a> {
    Plain(&'a str),
    /// A coloured dust: `color` is `0xRRGGBB`, `size` 0.1 to 4 (1 is redstone dust's).
    Dust {
        color: u32,
        size: f32,
    },
    /// A dust that fades from one colour to another.
    DustTransition {
        from: u32,
        to: u32,
        size: f32,
    },
    /// A block's texture breaking apart: `minecraft:stone`, or `stone`.
    Block(&'a str),
    /// One of your models' texture breaking apart, by its name.
    Item(&'a str),
}

/// One emitter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Emit<'a> {
    pub look: EmitLook<'a>,
    /// Particles per emission, 1 to 16.
    pub count: u8,
    /// Ticks between emissions, 10 to 1200 (rounded up to a multiple of 10).
    pub every: u16,
    /// From the piece's look, or from the place, in blocks, -8 to 8 each.
    pub offset: [f32; 3],
    /// How far the particles spread, 0 to 4 blocks.
    pub spread: f32,
}

fn num(value: f32, what: &str) -> Result<String, Error> {
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

fn colour(hex: u32, what: &str) -> Result<String, Error> {
    if hex > 0x00FF_FFFF {
        return Err(invalid(format!("{what} is 0xRRGGBB")));
    }
    Ok(format!("{hex:06x}"))
}

fn word(text: &str, what: &str, extra: &[u8]) -> Result<(), Error> {
    let ok = !text.is_empty()
        && text.len() <= 64
        && text.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || extra.contains(&b)
        });
    if ok {
        Ok(())
    } else {
        Err(invalid(format!("{what} is not a valid name")))
    }
}

impl Emit<'_> {
    /// The text of the emitter as the request and the `emit.<n>` setting write it.
    ///
    /// # Errors
    /// `Invalid` for a name, colour or number out of its bounds.
    pub fn spec(&self) -> Result<String, Error> {
        let mut out = match self.look {
            EmitLook::Plain(p) => {
                word(p, "a particle", b"")?;
                p.to_owned()
            }
            EmitLook::Dust { color, size } => {
                format!(
                    "dust {} {}",
                    colour(color, "the colour")?,
                    num(size, "the size")?
                )
            }
            EmitLook::DustTransition { from, to, size } => format!(
                "dust_transition {} {} {}",
                colour(from, "the first colour")?,
                colour(to, "the second colour")?,
                num(size, "the size")?
            ),
            EmitLook::Block(b) => {
                word(b, "a block id", b":./-")?;
                format!("block {b}")
            }
            EmitLook::Item(m) => {
                word(m, "a model", b"")?;
                format!("item {m}")
            }
        };
        if !(1..=16).contains(&self.count) {
            return Err(invalid("count is 1 to 16"));
        }
        if !(10..=1200).contains(&self.every) {
            return Err(invalid("every is 10 to 1200 ticks"));
        }
        let [x, y, z] = self.offset;
        if !(0.0..=4.0).contains(&self.spread) || [x, y, z].iter().any(|v| v.abs() > 8.0) {
            return Err(invalid(
                "the offset is within 8 blocks and the spread 0 to 4",
            ));
        }
        out.push_str(&format!(
            " {} {} {} {} {} {}",
            self.count,
            self.every,
            num(x, "x")?,
            num(y, "y")?,
            num(z, "z")?,
            num(self.spread, "the spread")?
        ));
        Ok(out)
    }
}

fn emitter_name(name: &str) -> Result<(), Error> {
    let ok = !name.is_empty()
        && name.len() <= 48
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
    if ok {
        Ok(())
    } else {
        Err(invalid(
            "an emitter's name is [a-z0-9_], 1 to 48 characters",
        ))
    }
}

/// The bytes of an `emitter` request.
///
/// # Errors
/// `Invalid` for a name, a world with a line break, a coordinate that is not finite, or an
/// emitter out of its bounds.
pub fn emitter_request(
    name: &str,
    world: &str,
    at: [f64; 3],
    emit: &Emit<'_>,
) -> Result<Vec<u8>, Error> {
    emitter_name(name)?;
    if world.is_empty() || world.len() > 64 || world.chars().any(char::is_control) {
        return Err(invalid("a world's name is 1 to 64 characters"));
    }
    if at.iter().any(|v| !v.is_finite() || v.abs() > 30_000_000.0) {
        return Err(invalid("a place's coordinates are finite"));
    }
    Ok(format!(
        "emitter {name}\nworld={world}\nat={} {} {}\nemit={}\n",
        at[0],
        at[1],
        at[2],
        emit.spec()?
    )
    .into_bytes())
}

/// Sets (or replaces) your emitter `name` at a place in `world`. It gives off particles while a
/// player is within 32 blocks of it and its chunk is loaded, and goes when your script does.
///
/// # Errors
/// `Limit` over 64 emitters, `Unknown` for a world that does not exist, `Invalid` otherwise.
pub fn emitter(name: &str, world: &str, at: [f64; 3], emit: &Emit<'_>) -> Result<(), Error> {
    ask(&emitter_request(name, world, at, emit)?).map(|_| ())
}

/// The bytes of an `emitter-remove` request.
///
/// # Errors
/// `Invalid` for a name that is not one.
pub fn emitter_remove_request(name: &str) -> Result<Vec<u8>, Error> {
    emitter_name(name)?;
    Ok(format!("emitter-remove {name}").into_bytes())
}

/// Removes your emitter `name` (`Unknown` when you have none by that name).
///
/// # Errors
/// `Unknown` or `Invalid`.
pub fn emitter_remove(name: &str) -> Result<(), Error> {
    ask(&emitter_remove_request(name)?).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn emit(look: EmitLook<'_>) -> Emit<'_> {
        Emit {
            look,
            count: 3,
            every: 20,
            offset: [0.0, 1.0, -0.5],
            spread: 0.25,
        }
    }

    #[test]
    fn every_kind_of_emitter_is_written_as_the_server_reads_it() {
        for (look, text) in [
            (EmitLook::Plain("flame"), "flame 3 20 0 1 -0.5 0.25"),
            (
                EmitLook::Dust {
                    color: 0xFF0000,
                    size: 1.5,
                },
                "dust ff0000 1.5 3 20 0 1 -0.5 0.25",
            ),
            (
                EmitLook::DustTransition {
                    from: 0x00FF00,
                    to: 0x0000FF,
                    size: 2.0,
                },
                "dust_transition 00ff00 0000ff 2 3 20 0 1 -0.5 0.25",
            ),
            (
                EmitLook::Block("minecraft:stone"),
                "block minecraft:stone 3 20 0 1 -0.5 0.25",
            ),
            (EmitLook::Item("ruby"), "item ruby 3 20 0 1 -0.5 0.25"),
        ] {
            assert_eq!(emit(look).spec().unwrap(), text);
        }
    }

    #[test]
    fn a_request_names_the_place_and_every_bound_is_checked_before_sending() {
        let bytes = emitter_request(
            "fire",
            "world",
            [1.5, 64.0, -3.0],
            &emit(EmitLook::Plain("smoke")),
        )
        .unwrap();
        assert_eq!(
            String::from_utf8(bytes).unwrap(),
            "emitter fire\nworld=world\nat=1.5 64 -3\nemit=smoke 3 20 0 1 -0.5 0.25\n"
        );
        assert_eq!(
            emitter_remove_request("fire").unwrap(),
            b"emitter-remove fire"
        );
        let ok = emit(EmitLook::Plain("smoke"));
        for bad in [
            Emit { count: 0, ..ok },
            Emit { count: 17, ..ok },
            Emit { every: 9, ..ok },
            Emit { every: 1201, ..ok },
            Emit { spread: 4.5, ..ok },
            Emit {
                spread: f32::NAN,
                ..ok
            },
            Emit {
                offset: [9.0, 0.0, 0.0],
                ..ok
            },
            Emit {
                offset: [0.0, f32::INFINITY, 0.0],
                ..ok
            },
            emit(EmitLook::Plain("Smoke")),
            emit(EmitLook::Plain("two words")),
            emit(EmitLook::Dust {
                color: 0x01FF_0000,
                size: 1.0,
            }),
            emit(EmitLook::Dust {
                color: 0,
                size: f32::NAN,
            }),
            emit(EmitLook::Item("a.b")),
        ] {
            assert!(bad.spec().is_err(), "{bad:?}");
        }
        for name in ["", "Fire", "a b", &"a".repeat(49)] {
            assert!(
                emitter_request(name, "world", [0.0; 3], &ok).is_err(),
                "{name:?}"
            );
            assert!(emitter_remove_request(name).is_err(), "{name:?}");
        }
        assert!(emitter_request("fire", "", [0.0; 3], &ok).is_err());
        assert!(emitter_request("fire", "w\nx", [0.0; 3], &ok).is_err());
        assert!(emitter_request("fire", "w", [f64::NAN, 0.0, 0.0], &ok).is_err());
        assert!(emitter_request("fire", "w", [1e9, 0.0, 0.0], &ok).is_err());
    }
}
