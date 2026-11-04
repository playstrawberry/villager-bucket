use crate::{
    exchange::{BucketHost, EventKey},
    policy::{Denial, Interaction, Plan},
};

/// Complete snapshots and checked removal are not available yet. Do not substitute
/// today's void remove(), or invent another source of full villager state.
pub struct PinnedSdk;

pub fn require_support() -> Result<(), Denial> {
    Err(Denial::Unsupported)
}

impl BucketHost for PinnedSdk {
    type Prepared = ();

    fn inspect(&self, _event: EventKey) -> Result<Interaction, Denial> {
        Err(Denial::Unsupported)
    }

    fn prepare(&mut self, _event: EventKey, _plan: Plan) -> Result<(), Denial> {
        Err(Denial::Unsupported)
    }

    fn cancel(&mut self, _event: EventKey) -> Result<(), Denial> {
        Err(Denial::Unsupported)
    }

    fn try_remove(&mut self, _prepared: &()) -> Result<bool, Denial> {
        Err(Denial::Unsupported)
    }

    fn give_bucket(&mut self, _prepared: ()) -> Result<(), Denial> {
        Err(Denial::Unsupported)
    }

    fn consume_bucket(&mut self, _prepared: &()) -> Result<bool, Denial> {
        Err(Denial::Unsupported)
    }

    fn spawn_villager(&mut self, _prepared: ()) -> Result<(), Denial> {
        Err(Denial::Unsupported)
    }
}
