//! The shared model service owns item components and pack construction.

use strawberry_tools::models::{Behaviour, Error, ErrorKind, Model, Registered};
use strawberry_plugin_api::{ItemStack, Player};

pub const PLUGIN: &str = strawberry_tools::models::PLUGIN;
pub const NAME: &str = "villager_bucket";
const FILES: &[(&str, &[u8])] = &[(
    "texture.png",
    include_bytes!("../models/villager_bucket.png"),
)];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Main,
    Off,
    Slot(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delivery {
    Model(Place),
    Vanilla,
}

/// Cosmetic only: neither variant authorizes restoring an entity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VisualId {
    VillagerBucket,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Models {
    id: String,
    registered: bool,
    ready: bool,
}

impl Models {
    /// Register once at load, using the same catalog as the vanilla fallback.
    /// The transport and wire format come from the shared typed client.
    pub fn connect_with(
        owner: &str,
        register: impl FnOnce(&Model<'_>) -> Result<Registered, Error>,
    ) -> Self {
        let names = crate::lang::FILES
            .map(|(locale, _)| (locale, crate::lang::text(locale, "bucket_name")));
        let names = names
            .each_ref()
            .map(|(locale, text)| (*locale, text.as_str()));
        let model = Model {
            name: NAME,
            names: &names,
            base: Some("minecraft:paper"),
            stack: Some(1),
            files: FILES,
            wear: None,
            behaviour: Behaviour::NONE,
            variant_of: None,
        };
        let id = format!("{owner}.{NAME}");
        let result = register(&model).ok().filter(|model| model.id == id);
        Self {
            id,
            registered: result.is_some(),
            ready: result.is_some_and(|model| model.ready),
        }
    }

    pub fn registered(&self) -> bool {
        self.registered
    }

    pub fn ready(&self) -> bool {
        self.ready
    }

    /// Visual only. Attach the independently validated snapshot at this slot.
    /// An uncertain mutation is never retried or followed by another grant.
    pub fn give_with(
        &self,
        give: impl FnOnce(&str) -> Result<u32, Error>,
    ) -> Result<Delivery, ErrorKind> {
        // Registration readiness is a load-time hint. The shared service is
        // authoritative after a later pack build, without polling or a timer.
        if !self.registered {
            return Ok(Delivery::Vanilla);
        }
        match give(&self.id) {
            Ok(slot @ 0..=40) => Ok(Delivery::Model(Place::Slot(slot as u8))),
            Ok(_) => Err(ErrorKind::Other),
            Err(error) if unavailable(error.kind) => Ok(Delivery::Vanilla),
            Err(error) => Err(error.kind),
        }
    }

    pub fn give_visual(&self, player: &Player) -> Result<Delivery, ErrorKind> {
        self.give_with(|id| strawberry_tools::models::give(player, id, 1))
    }

    /// The caller must validate the actual hand and its empty bucket first.
    pub fn hand_with(
        &self,
        off: bool,
        hand: impl FnOnce(&str, bool) -> Result<(), Error>,
    ) -> Result<Delivery, ErrorKind> {
        if !self.registered {
            return Ok(Delivery::Vanilla);
        }
        match hand(&self.id, off) {
            Ok(()) => Ok(Delivery::Model(if off { Place::Off } else { Place::Main })),
            Err(error) if unavailable(error.kind) => Ok(Delivery::Vanilla),
            Err(error) => Err(error.kind),
        }
    }

    pub fn hand_visual(&self, player: &Player, off: bool) -> Result<Delivery, ErrorKind> {
        self.hand_with(off, |id, off| strawberry_tools::models::hand(player, off, id, 1))
    }

    /// The visible/copyable model id is not a persisted villager record.
    pub fn identify_id(&self, id: Option<&str>) -> VisualId {
        if id == Some(self.id.as_str()) {
            VisualId::VillagerBucket
        } else {
            VisualId::Other
        }
    }

    pub fn identify(&self, stack: &ItemStack) -> VisualId {
        self.identify_id(strawberry_tools::models::model_id(stack).as_deref())
    }
}

fn unavailable(error: ErrorKind) -> bool {
    matches!(error, ErrorKind::NotInstalled | ErrorKind::Unavailable)
}
