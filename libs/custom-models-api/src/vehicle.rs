//! Vehicles: one of your models that a player places with its item, gets on, drives with the
//! vanilla controls and fuels.
//!
//! ```ignore
//! use custom_models_api::{Fuel, Vehicle, vehicle};
//!
//! // in on_load, after registering the models it names:
//! vehicle(
//!     &Vehicle::new("kart", "kart", 0.2)
//!         .jump(0.45)
//!         .tank(900)
//!         .fuels(&[Fuel::vanilla("coal", 40), Fuel::model("cell", 300)]),
//! )?;
//! ```
//!
//! The vehicle is a vanilla mount (a saddled horse for one seat, a camel for two) that the rider's
//! own client steers with vanilla physics, made invisible and silent; the model's item is its look.
//! A player right-clicks a block with the item to place it, right-clicks the vehicle to get on
//! (the driver sits first), uses a fuel item on it to fill the tank, and hits it to take it back
//! as its item (the fuel left stays in the item). The tank burns once a second, only while the
//! vehicle moves; empty, it cannot move and the driver is told once.
//!
//! The name a player reads is the model's own (`name.en`, `name.fr`, `name.es`, `name.tr`).

use super::{Error, Registered, ask, field, invalid};

/// What burns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuelItem<'a> {
    /// A vanilla item by its name (`coal` or `minecraft:coal`).
    Vanilla(&'a str),
    /// One of your models, by its name.
    Model(&'a str),
}

/// A fuel: an item and the seconds of driving one of it gives (1 to 3600).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fuel<'a> {
    pub item: FuelItem<'a>,
    pub seconds: u32,
}

impl<'a> Fuel<'a> {
    #[must_use]
    pub const fn vanilla(name: &'a str, seconds: u32) -> Self {
        Self {
            item: FuelItem::Vanilla(name),
            seconds,
        }
    }

    #[must_use]
    pub const fn model(name: &'a str, seconds: u32) -> Self {
        Self {
            item: FuelItem::Model(name),
            seconds,
        }
    }
}

/// A vehicle of your script. Its id is `<your script>.<name>`; the same name again replaces it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vehicle<'a> {
    /// `[a-z0-9_]`, 1 to 48 characters.
    pub name: &'a str,
    /// One of your models, by its name: its item places the vehicle, drops from it and is its look.
    pub model: &'a str,
    /// The top speed, 0.05 to 0.35 (a vanilla horse runs 0.1125 to 0.3375).
    pub speed: f32,
    /// 0.1 to 1; 0.6 when not set.
    pub jump: f32,
    /// 1 or 2; 1 when not set.
    pub seats: u8,
    /// The most fuel it holds, in seconds of driving, 10 to 7200; 600 when not set.
    pub tank: u32,
    /// 1 to 8 fuels.
    pub fuels: &'a [Fuel<'a>],
    /// The look's size, 0.25 to 4.
    pub scale: f32,
    /// The look's offset in blocks, -4 to 4 each, in the vehicle's frame (`+z` is its front).
    pub offset: [f32; 3],
}

impl<'a> Vehicle<'a> {
    /// A one-seat vehicle of `model` with the top speed `speed`, jump 0.6 and a tank of 600
    /// seconds. Give it fuels with [`Vehicle::fuels`].
    #[must_use]
    pub const fn new(name: &'a str, model: &'a str, speed: f32) -> Self {
        Self {
            name,
            model,
            speed,
            jump: 0.6,
            seats: 1,
            tank: 600,
            fuels: &[],
            scale: 1.0,
            offset: [0.0; 3],
        }
    }

    #[must_use]
    pub const fn jump(mut self, jump: f32) -> Self {
        self.jump = jump;
        self
    }

    #[must_use]
    pub const fn seats(mut self, seats: u8) -> Self {
        self.seats = seats;
        self
    }

    #[must_use]
    pub const fn tank(mut self, seconds: u32) -> Self {
        self.tank = seconds;
        self
    }

    #[must_use]
    pub const fn fuels(mut self, fuels: &'a [Fuel<'a>]) -> Self {
        self.fuels = fuels;
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
}

/// One name of yours: `[a-z0-9_]`, `max` characters at most.
fn name(text: &str, what: &str, max: usize) -> Result<(), Error> {
    let ok = !text.is_empty()
        && text.len() <= max
        && text
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if ok {
        Ok(())
    } else {
        Err(invalid(format!(
            "{what} is [a-z0-9_], 1 to {max} characters"
        )))
    }
}

/// A number as the request writes it: at most three decimals, finite.
fn number(value: f32, what: &str) -> Result<String, Error> {
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

/// The bytes of a `vehicle` request for `v`.
pub fn vehicle_request(v: &Vehicle<'_>) -> Result<Vec<u8>, Error> {
    name(v.name, "a vehicle name", 48)?;
    name(v.model, "a model", 48)?;
    let mut out = format!(
        "vehicle {}\nmodel={}\nspeed={}\n",
        v.name,
        v.model,
        number(v.speed, "speed")?
    );
    out.push_str(&format!("jump={}\n", number(v.jump, "jump")?));
    out.push_str(&format!("seats={}\n", v.seats));
    out.push_str(&format!("tank={}\n", v.tank));
    for (n, f) in v.fuels.iter().enumerate() {
        let item = match f.item {
            FuelItem::Vanilla(item) => {
                let bare = item.strip_prefix("minecraft:").unwrap_or(item);
                name(bare, "a vanilla fuel", 64)?;
                // Always the full id: a model of yours with the same name must not take its place.
                format!("minecraft:{bare}")
            }
            FuelItem::Model(model) => {
                name(model, "a fuel model", 48)?;
                model.to_owned()
            }
        };
        out.push_str(&format!("fuel.{}={item} {}\n", n + 1, f.seconds));
    }
    out.push_str(&format!("scale={}\n", number(v.scale, "scale")?));
    out.push_str(&format!(
        "offset={}\n",
        v.offset
            .iter()
            .map(|n| number(*n, "offset"))
            .collect::<Result<Vec<_>, _>>()?
            .join(" ")
    ));
    Ok(out.into_bytes())
}

/// Registers (or replaces) one of your vehicles. Call it in `on_load`, after the models it names.
/// Vehicles already in the world work again once it is registered; until then nobody can get on
/// them.
pub fn vehicle(v: &Vehicle<'_>) -> Result<Registered, Error> {
    let fields = ask(&vehicle_request(v)?)?;
    Ok(Registered {
        id: field(&fields, 0)?.to_owned(),
        ready: field(&fields, 1)? == "ready",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_carries_every_setting() {
        let fuels = [Fuel::vanilla("coal", 40), Fuel::model("cell", 300)];
        let v = Vehicle::new("kart", "kart", 0.2)
            .jump(0.45)
            .seats(2)
            .tank(900)
            .fuels(&fuels)
            .scale(1.5)
            .offset([0.0, -0.25, 0.5]);
        assert_eq!(
            String::from_utf8(vehicle_request(&v).unwrap()).unwrap(),
            "vehicle kart\nmodel=kart\nspeed=0.2\njump=0.45\nseats=2\ntank=900\nfuel.1=minecraft:coal 40\nfuel.2=cell 300\nscale=1.5\noffset=0 -0.25 0.5\n"
        );
        let fuels = [Fuel::vanilla("minecraft:blaze_rod", 90)];
        assert_eq!(
            String::from_utf8(vehicle_request(&Vehicle::new("a", "a", 0.1).fuels(&fuels)).unwrap())
                .unwrap(),
            "vehicle a\nmodel=a\nspeed=0.1\njump=0.6\nseats=1\ntank=600\nfuel.1=minecraft:blaze_rod 90\nscale=1\noffset=0 0 0\n"
        );
    }

    #[test]
    fn line_breaks_and_odd_numbers_cannot_add_settings() {
        let coal = [Fuel::vanilla("coal", 40)];
        for v in [
            Vehicle::new("a\ntank=1", "m", 0.2).fuels(&coal),
            Vehicle::new("a", "m\nseats=2", 0.2).fuels(&coal),
            Vehicle::new("A", "m", 0.2).fuels(&coal),
            Vehicle::new("a", "", 0.2).fuels(&coal),
            Vehicle::new("a", "m", f32::NAN).fuels(&coal),
            Vehicle::new("a", "m", f32::INFINITY).fuels(&coal),
            Vehicle::new("a", "m", 0.2)
                .fuels(&coal)
                .offset([f32::NAN, 0.0, 0.0]),
            Vehicle::new("a", "m", 0.2).fuels(&[Fuel::vanilla("coal\nseats=2", 40)]),
            Vehicle::new("a", "m", 0.2).fuels(&[Fuel::vanilla("other:coal", 40)]),
            Vehicle::new("a", "m", 0.2).fuels(&[Fuel::model("cell 5\nfuel.2=x", 40)]),
            Vehicle::new("a", "m", 0.2).fuels(&[Fuel::model("", 40)]),
        ] {
            assert!(vehicle_request(&v).is_err(), "{v:?}");
        }
    }

    #[test]
    fn the_numbers_are_written_with_at_most_three_decimals() {
        assert_eq!(number(0.2, "x").unwrap(), "0.2");
        assert_eq!(number(0.1234, "x").unwrap(), "0.123");
        assert_eq!(number(-0.0, "x").unwrap(), "0");
        assert_eq!(number(3.0, "x").unwrap(), "3");
    }
}
