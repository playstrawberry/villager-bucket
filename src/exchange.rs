use crate::policy::{Denial, Interaction, Plan};

/// An event-scoped host handle, not a client token or a new persistent identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventKey(pub u64);

/// Private adapter operations, not new SDK/WIT methods. `Prepared` owns the
/// complete snapshot and prepared output; no partial villager schema.
///
/// Called from a blocking interact handler. `prepare` validates the complete
/// snapshot, room, permission, live actor/target and destination before removal
/// or consumption, respecting ordinary cancellable protection events.
/// It prebuilds the non-stackable bucket, not an item-authority credential.
///
/// Capture uses checked removal: only its winner may give. Release consumes the
/// original held bucket before attempting to spawn, so another ordinary click
/// cannot reuse it. The adapter checks the actual hand/item, not just the lore.
/// These are deliberately separate operations, not an atomic transaction.
/// A crash or failure after source removal/consumption may lose the villager;
/// never retry or spawn/restore a replacement after an ambiguous result.
///
/// Creative copies are allowed independent snapshots. Restore as a NEW entity,
/// preserving saved villager properties, not the source entity's runtime UUID.
/// No durable redemption/anti-replay ledger is requested or kept here.
pub trait BucketHost {
    type Prepared;

    fn inspect(&self, event: EventKey) -> Result<Interaction, Denial>;
    fn prepare(&mut self, event: EventKey, plan: Plan) -> Result<Self::Prepared, Denial>;
    fn cancel(&mut self, event: EventKey) -> Result<(), Denial>;
    fn try_remove(&mut self, prepared: &Self::Prepared) -> Result<bool, Denial>;
    fn give_bucket(&mut self, prepared: Self::Prepared) -> Result<(), Denial>;
    fn consume_bucket(&mut self, prepared: &Self::Prepared) -> Result<bool, Denial>;
    fn spawn_villager(&mut self, prepared: Self::Prepared) -> Result<(), Denial>;
}

pub fn handle(host: &mut impl BucketHost, event: EventKey) -> Result<bool, Denial> {
    let Some(plan) = crate::policy::plan(host.inspect(event)?)? else {
        return Ok(false);
    };
    let prepared = host.prepare(event, plan)?;
    host.cancel(event)?;
    match plan {
        Plan::Capture { .. } => {
            if !host.try_remove(&prepared)? {
                return Ok(false);
            }
            host.give_bucket(prepared)?;
        }
        Plan::Release { .. } => {
            if !host.consume_bucket(&prepared)? {
                return Ok(false);
            }
            host.spawn_villager(prepared)?;
        }
    }
    Ok(true)
}
