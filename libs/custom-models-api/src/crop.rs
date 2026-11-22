//! Crops: a seed that is planted on farmland (or another soil) and grows through looks until it
//! can be harvested.
//!
//! ```ignore
//! use custom_models_api::{BlockFeel, BlockLook, Crop, CropHarvest, StateLook, crop};
//!
//! const TOMATO: Crop<'static> = Crop {
//!     name: "tomato",
//!     names: &[("en", "Tomato Seeds"), ("tr", "Domates Tohumu")],
//!     stack: None,
//!     grow: 120, // seconds each stage takes
//!     look: BlockLook::All(include_bytes!("../crops/tomato_0.png")), // stage 0, the seed's look
//!     stages: &[
//!         StateLook::All(include_bytes!("../crops/tomato_1.png")),
//!         StateLook::All(include_bytes!("../crops/tomato_2.png")),
//!         StateLook::All(include_bytes!("../crops/tomato_3.png")), // ripe
//!     ],
//!     soil: &[], // farmland
//!     light: Some(9),
//!     water: false,
//!     bonemeal: None,
//!     seeds: None,
//!     harvest: Some(CropHarvest { item: "tomato", min: 1, max: 3 }),
//!     feel: BlockFeel::default(),
//! };
//!
//! // in on_load, with "custom-models" in the metadata's dependencies:
//! let seed = crop(&TOMATO)?; // seed.id == "<your plugin>.tomato": the crop's block and its seed
//! ```
//!
//! The seed is the crop's item: a player plants it by placing it, on the top of its soil only
//! (farmland when `soil` is empty), and the block place event fires as for any block, so a
//! protection plugin can refuse it. A crop has one look for each stage (`look` and `stages`: 2 to
//! 8 in all) and passes the stages one after the other, each after `grow` seconds of game time.
//! Nothing grows while its chunk is not loaded; when it loads again the stages that came due
//! while it was away are gained at once. A crop that needs `light` or `water` (hydrated
//! farmland) waits one more stage of time when they are not there. Bone meal advances a crop 2 to
//! 5 stages as it does a vanilla crop, unless `bonemeal` is `Some(false)`.
//!
//! A crop that breaks young drops its seed; a ripe one drops its `harvest` (`min` to `max`, an
//! item of yours or `minecraft:<item>`, and one more with a 4 in 7 chance for each level of
//! Fortune) and seeds, as wheat does (`seeds`: `Some(false)` for none). A crop whose soil is
//! broken pops. Its feel lines are a passable block's (`feel`: hardness 0 or -1, resistance, a
//! sound of its own); a `drops` rule from the crop's block ([`crate::Drops::my_block`]) adds to
//! these drops as for any block.
//!
//! A server has 96 passable states for every plugin together and a crop takes one for each
//! stage: a request that cannot get its states answers [`crate::ErrorKind::Full`]. A plugin
//! registers at most 16 crops and a server holds at most 48.

use super::{
    Block, BlockDrop, BlockFeel, BlockLook, BlockShape, Error, Registered, StateLook, ask,
    block_request, field, invalid,
};

/// What a ripe crop gives besides its seeds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CropHarvest<'a> {
    /// An item of yours (its name) or a vanilla one (`minecraft:wheat`).
    pub item: &'a str,
    /// At most `max`, 0 to 64.
    pub min: u8,
    /// 1 to 64.
    pub max: u8,
}

/// A crop of your plugin. Its id is `<your plugin>.<name>`, the id of its block and its seed.
#[derive(Debug, Clone, Copy)]
pub struct Crop<'a> {
    /// `[a-z0-9_]`.
    pub name: &'a str,
    /// The seed's name by language (`en` required; `fr`, `es`, `tr`).
    pub names: &'a [(&'a str, &'a str)],
    /// The seed's stack size, 1 to 99; 64 when `None`.
    pub stack: Option<u8>,
    /// Seconds one stage takes, 5 to 86,400.
    pub grow: u32,
    /// Stage 0, the look of a seed that was just planted.
    pub look: BlockLook<'a>,
    /// The looks of the stages after it, the last one ripe: 1 to 7, so 2 to 8 stages in all.
    pub stages: &'a [StateLook<'a>],
    /// Up to 8 vanilla blocks (`minecraft:farmland`) the crop grows on; farmland when empty.
    pub soil: &'a [&'a str],
    /// The least light (1 to 15) that lets it grow.
    pub light: Option<u8>,
    /// Hydrated farmland only (farmland must be its only soil).
    pub water: bool,
    /// Bone meal advances it; `None` is the default, true.
    pub bonemeal: Option<bool>,
    /// A broken ripe crop drops seeds; `None` is the default, true.
    pub seeds: Option<bool>,
    /// What a ripe crop drops besides its seeds.
    pub harvest: Option<CropHarvest<'a>>,
    /// How the block feels (a passable block's lines only).
    pub feel: BlockFeel<'a>,
}

/// The bytes of a `crop` request for `crop`.
pub fn crop_request(crop: &Crop<'_>) -> Result<Vec<u8>, Error> {
    if crop.stages.is_empty() || crop.stages.len() > 7 {
        return Err(invalid(
            "a crop has 2 to 8 stages: its look and 1 to 7 more",
        ));
    }
    let word = |s: &str| !s.is_empty() && !s.contains(['\n', '\r', '=', ' ', ',']);
    if crop.soil.iter().any(|s| !word(s)) {
        return Err(invalid("a soil is one vanilla block, minecraft:<block>"));
    }
    if let Some(h) = &crop.harvest
        && (!word(h.item) || h.min > h.max)
    {
        return Err(invalid("a harvest is an item and min at most max"));
    }
    // The looks and the block's own lines are a block's: build that request, then make it a
    // crop's (its shape, its count of states and its drop are the crop's own).
    let block = block_request(&Block {
        name: crop.name,
        names: crop.names,
        stack: crop.stack,
        drop: BlockDrop::Itself,
        notify: false,
        look: crop.look,
        feel: crop.feel,
        shape: BlockShape::Cube,
        states: crop.stages,
        facing: false,
    })?;
    let end = block
        .windows(2)
        .position(|w| w == b"\n\n")
        .ok_or_else(|| invalid("the request has no settings"))?;
    let head = std::str::from_utf8(&block[..end]).map_err(|_| invalid("the settings are text"))?;
    let mut lines = head.lines();
    let first = lines
        .next()
        .and_then(|l| l.strip_prefix("block "))
        .ok_or_else(|| invalid("the request has no name"))?;
    let mut settings = format!("crop {first}\n");
    for line in lines {
        // The crop sets these itself.
        if !(line.starts_with("states=") || line.starts_with("shape=")) {
            settings.push_str(line);
            settings.push('\n');
        }
    }
    settings.push_str(&format!(
        "stages={}\ngrow={}\n",
        crop.stages.len() + 1,
        crop.grow
    ));
    if !crop.soil.is_empty() {
        settings.push_str(&format!("soil={}\n", crop.soil.join(",")));
    }
    if let Some(light) = crop.light {
        settings.push_str(&format!("light={light}\n"));
    }
    if crop.water {
        settings.push_str("water=true\n");
    }
    if let Some(on) = crop.bonemeal {
        settings.push_str(&format!("bonemeal={on}\n"));
    }
    if let Some(on) = crop.seeds {
        settings.push_str(&format!("seeds={on}\n"));
    }
    if let Some(h) = &crop.harvest {
        settings.push_str(&format!("harvest={} {}-{}\n", h.item, h.min, h.max));
    }
    // The blank line that ends the settings is the block request's own.
    let mut out = settings.trim_end().to_owned().into_bytes();
    out.extend_from_slice(&block[end..]);
    Ok(out)
}

/// Registers (or replaces) one of your crops: its block and its seed. Call it in `on_load`.
pub fn crop(crop: &Crop<'_>) -> Result<Registered, Error> {
    let fields = ask(&crop_request(crop)?)?;
    Ok(Registered {
        id: field(&fields, 0)?.to_owned(),
        ready: field(&fields, 1)? == "ready",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ErrorKind;

    const PNG: &[u8] = b"png";

    fn tomato() -> Crop<'static> {
        Crop {
            name: "tomato",
            names: &[("en", "Tomato Seeds")],
            stack: None,
            grow: 120,
            look: BlockLook::All(PNG),
            stages: &[StateLook::All(PNG), StateLook::All(PNG)],
            soil: &[],
            light: None,
            water: false,
            bonemeal: None,
            seeds: None,
            harvest: None,
            feel: BlockFeel::default(),
        }
    }

    fn text(c: &Crop<'_>) -> String {
        String::from_utf8(crop_request(c).unwrap()).unwrap()
    }

    #[test]
    fn a_crop_request_is_a_block_request_with_the_crops_own_lines() {
        assert_eq!(
            text(&tomato()),
            "crop tomato\nname.en=Tomato Seeds\nstages=3\ngrow=120\n\nall.png 3\npngstate.1.png 3\npngstate.2.png 3\npng"
        );
        let c = Crop {
            stack: Some(16),
            soil: &["minecraft:farmland", "minecraft:dirt"],
            light: Some(9),
            bonemeal: Some(false),
            seeds: Some(false),
            harvest: Some(CropHarvest {
                item: "tomato",
                min: 1,
                max: 3,
            }),
            feel: BlockFeel {
                hardness: Some(0.0),
                ..BlockFeel::default()
            },
            ..tomato()
        };
        let t = text(&c);
        let head = t.split("\n\n").next().unwrap();
        assert_eq!(
            head,
            "crop tomato\nname.en=Tomato Seeds\nstack=16\nhardness=0\nstages=3\ngrow=120\nsoil=minecraft:farmland,minecraft:dirt\nlight=9\nbonemeal=false\nseeds=false\nharvest=tomato 1-3\n"
                .trim_end()
        );
        // The block's own shape and state lines are never sent.
        assert!(!t.contains("shape=") && !t.contains("states="), "{t}");
        let wet = Crop {
            water: true,
            ..tomato()
        };
        assert!(text(&wet).contains("\nwater=true\n"));
    }

    #[test]
    fn a_crop_with_a_bad_stage_count_or_word_is_refused_before_it_is_sent() {
        let none = Crop {
            stages: &[],
            ..tomato()
        };
        let many_looks: Vec<StateLook<'_>> = (0..8).map(|_| StateLook::All(PNG)).collect();
        let many = Crop {
            stages: &many_looks,
            ..tomato()
        };
        let soil_with_space = Crop {
            soil: &["minecraft:dirt, minecraft:sand"],
            ..tomato()
        };
        let soil_with_comma = Crop {
            soil: &["minecraft:dirt,minecraft:sand"],
            ..tomato()
        };
        let backwards = Crop {
            harvest: Some(CropHarvest {
                item: "tomato",
                min: 3,
                max: 1,
            }),
            ..tomato()
        };
        let broken_item = Crop {
            harvest: Some(CropHarvest {
                item: "to\nmato",
                min: 1,
                max: 2,
            }),
            ..tomato()
        };
        let bad_name = Crop {
            name: "to mato",
            ..tomato()
        };
        for bad in [
            none,
            many,
            soil_with_space,
            soil_with_comma,
            backwards,
            broken_item,
            bad_name,
        ] {
            assert_eq!(crop_request(&bad).unwrap_err().kind, ErrorKind::Invalid);
        }
        // Seven more looks are eight stages: the most.
        let seven: Vec<StateLook<'_>> = (0..7).map(|_| StateLook::All(PNG)).collect();
        let most = Crop {
            stages: &seven,
            ..tomato()
        };
        assert!(text(&most).contains("\nstages=8\n"));
    }
}
