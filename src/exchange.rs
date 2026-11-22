use crate::policy::{Denial, Interaction, Plan};

/// An event-scoped host handle, not a client token or a new persistent identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventKey(pub u64);

/// Private adapter operations, not new SDK/WIT methods. `Prepared` owns the
/// complete snapshot and the held item; no partial villager schema.
///
/// Called from a blocking interact handler. `prepare` checks permission, the
/// live actor and target, room and destination, and takes the complete snapshot
/// (capture) or reads and checks the held bucket's record (release), before
/// anything changes. Ordinary cancellable protection events decide first.
///
/// Capture: the snapshot is taken first, then `try_remove`; only its winner
/// gives the bucket, and a loser gives nothing. A failure after a won removal
/// still delivers the bucket (hand, inventory or the player's feet), so the
/// villager is never lost and never doubled.
///
/// Release: the held bucket is swapped for an empty one, then the villager is
/// made from the record. The swap means the occupied bucket and the new villager
/// never exist together; a refused record puts the occupied bucket back, so the
/// bucket is only used up by a villager that was made.
///
/// Creative copies are allowed independent snapshots. Restore as a NEW entity,
/// preserving saved villager properties, not the source entity's runtime UUID.
/// No durable redemption/anti-replay ledger is requested or kept here.
pub trait BucketHost {
    type Prepared;

    fn inspect(&self, event: EventKey) -> Result<Interaction, Denial>;
    fn prepare(&mut self, event: EventKey, plan: Plan) -> Result<Self::Prepared, Denial>;
    fn cancel(&mut self, event: EventKey) -> Result<(), Denial>;
    /// True only for the one caller that removed the villager.
    fn try_remove(&mut self, prepared: &Self::Prepared) -> Result<bool, Denial>;
    fn give_bucket(&mut self, prepared: Self::Prepared) -> Result<(), Denial>;
    /// Swaps the held occupied bucket for an empty one; false when the hand no
    /// longer holds that exact bucket.
    fn take_bucket(&mut self, prepared: &Self::Prepared) -> Result<bool, Denial>;
    /// Makes the villager from the record; `Denial::Refused` when the server refuses it.
    fn spawn_villager(&mut self, prepared: &Self::Prepared) -> Result<(), Denial>;
    /// Puts the taken occupied bucket back after a refused spawn.
    fn return_bucket(&mut self, prepared: Self::Prepared) -> Result<(), Denial>;
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
            if !host.take_bucket(&prepared)? {
                return Ok(false);
            }
            if let Err(denial) = host.spawn_villager(&prepared) {
                host.return_bucket(prepared)?;
                return Err(denial);
            }
        }
    }
    Ok(true)
}
