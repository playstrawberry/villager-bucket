//! Tests of the real dispatcher against a small operation adapter, NOT engine
//! concurrency/persistence acceptance. No fake snapshot codec or production ledger.
use std::sync::{Arc, Barrier, Mutex};

use crate::{
    exchange::{BucketHost, EventKey, handle},
    policy::{Denial, Face, Hand, Interaction, Item, Plan, Refusal, Target},
};

const STATE: &[u8] = br#"{"profession":"librarian","level":4,"xp":180,"type":"swamp","age":-24000,"name":"Test Villager","offers":[{"uses":3,"maxUses":4,"price":7}],"gossips":[{"reputation":25}]}"#;

struct Prepared {
    state: Vec<u8>,
}

struct Probe {
    view: Interaction,
    source: Arc<Mutex<Option<Vec<u8>>>>,
    held: Option<Vec<u8>>,
    buckets: Vec<Vec<u8>>,
    spawned: Vec<Vec<u8>>,
    trace: Vec<&'static str>,
    prepare_error: Option<Denial>,
    fail_at: Option<&'static str>,
    prepared_barrier: Option<Arc<Barrier>>,
    spawn_refusal: Option<Refusal>,
}

impl Probe {
    fn capture() -> Self {
        Self {
            view: Interaction {
                cancelled: false,
                permitted: true,
                connected_alive: true,
                hand: Some(Hand::Main),
                item: Item::EmptyBucket { count: 1 },
                target: Target::Villager,
                output_room: true,
            },
            source: Arc::new(Mutex::new(Some(STATE.to_vec()))),
            held: None,
            buckets: Vec::new(),
            spawned: Vec::new(),
            trace: Vec::new(),
            prepare_error: None,
            fail_at: None,
            prepared_barrier: None,
            spawn_refusal: None,
        }
    }

    fn release() -> Self {
        let mut probe = Self::capture();
        probe.view.item = Item::OccupiedBucket { count: 1 };
        probe.view.target = Target::Block {
            position: [1, 64, 2],
            face: Face::Up,
        };
        probe.held = Some(STATE.to_vec());
        probe
    }

    fn stage(&mut self, name: &'static str) -> Result<(), Denial> {
        self.trace.push(name);
        if self.fail_at == Some(name) {
            Err(Denial::Stale)
        } else {
            Ok(())
        }
    }
}

impl BucketHost for Probe {
    type Prepared = Prepared;

    fn inspect(&self, _event: EventKey) -> Result<Interaction, Denial> {
        Ok(self.view)
    }

    fn prepare(&mut self, _event: EventKey, plan: Plan) -> Result<Prepared, Denial> {
        self.stage("prepare")?;
        if let Some(reason) = self.prepare_error {
            return Err(reason);
        }
        let state = match plan {
            Plan::Capture { .. } => self.source.lock().unwrap().clone(),
            Plan::Release { .. } => self.held.clone(),
        }
        .ok_or(Denial::Stale)?;
        if let Some(barrier) = &self.prepared_barrier {
            barrier.wait();
        }
        Ok(Prepared { state })
    }

    fn cancel(&mut self, _event: EventKey) -> Result<(), Denial> {
        self.stage("cancel")
    }

    fn try_remove(&mut self, _prepared: &Prepared) -> Result<bool, Denial> {
        self.stage("try-remove")?;
        Ok(self.source.lock().unwrap().take().is_some())
    }

    fn give_bucket(&mut self, prepared: Prepared) -> Result<(), Denial> {
        self.stage("give")?;
        self.buckets.push(prepared.state);
        self.view.item = Item::OccupiedBucket { count: 1 };
        Ok(())
    }

    fn take_bucket(&mut self, prepared: &Prepared) -> Result<bool, Denial> {
        self.stage("take")?;
        if self.held.as_deref() != Some(prepared.state.as_slice()) {
            return Ok(false);
        }
        self.held = None;
        self.view.item = Item::EmptyBucket { count: 1 };
        Ok(true)
    }

    fn spawn_villager(&mut self, prepared: &Prepared) -> Result<(), Denial> {
        self.stage("spawn")?;
        if let Some(refusal) = self.spawn_refusal {
            return Err(Denial::Refused(refusal));
        }
        self.spawned.push(prepared.state.clone());
        Ok(())
    }

    fn return_bucket(&mut self, prepared: Prepared) -> Result<(), Denial> {
        self.stage("return")?;
        self.held = Some(prepared.state);
        self.view.item = Item::OccupiedBucket { count: 1 };
        Ok(())
    }
}

#[test]
fn capture_prepares_and_cancels_before_try_remove_and_gives_only_after_success() {
    let mut host = Probe::capture();
    assert_eq!(handle(&mut host, EventKey(1)), Ok(true));
    assert_eq!(host.trace, ["prepare", "cancel", "try-remove", "give"]);
    assert_eq!(host.buckets, [STATE]);
    assert!(host.source.lock().unwrap().is_none());
}

#[test]
fn missing_source_snapshot_does_not_remove_give_or_retry() {
    let mut host = Probe::capture();
    host.source = Arc::new(Mutex::new(None));
    // Late losing removal is exercised separately with two prepared players.
    assert_eq!(handle(&mut host, EventKey(2)), Err(Denial::Stale));
    assert!(host.buckets.is_empty());
    assert_eq!(host.trace, ["prepare"]);
}

#[test]
fn two_players_with_prepared_snapshots_get_exactly_one_try_remove_winner() {
    let mut first = Probe::capture();
    let mut second = Probe::capture();
    second.source = Arc::clone(&first.source);
    let barrier = Arc::new(Barrier::new(2));
    first.prepared_barrier = Some(Arc::clone(&barrier));
    second.prepared_barrier = Some(barrier);
    let a = std::thread::spawn(move || {
        let result = handle(&mut first, EventKey(1));
        (result, first)
    });
    let b = std::thread::spawn(move || {
        let result = handle(&mut second, EventKey(2));
        (result, second)
    });
    let (ar, a) = a.join().unwrap();
    let (br, b) = b.join().unwrap();
    assert_eq!(
        [ar, br]
            .iter()
            .filter(|result| **result == Ok(true))
            .count(),
        1
    );
    assert_eq!(
        [ar, br]
            .iter()
            .filter(|result| **result == Ok(false))
            .count(),
        1
    );
    assert_eq!(a.buckets.len() + b.buckets.len(), 1);
    let loser = if a.buckets.is_empty() { a } else { b };
    assert_eq!(loser.trace, ["prepare", "cancel", "try-remove"]);
}

#[test]
fn full_inventory_refuses_single_or_stacked_buckets_before_removal() {
    for count in [1, 2, 16] {
        let mut host = Probe::capture();
        host.view.item = Item::EmptyBucket { count };
        host.view.output_room = false;
        assert_eq!(handle(&mut host, EventKey(1)), Err(Denial::Full));
        assert!(host.trace.is_empty());
        assert_eq!(host.source.lock().unwrap().as_deref(), Some(STATE));
    }
}

#[test]
fn double_click_capture_does_not_produce_a_second_bucket() {
    let mut host = Probe::capture();
    assert_eq!(handle(&mut host, EventKey(1)), Ok(true));
    assert_eq!(handle(&mut host, EventKey(2)), Ok(false));
    assert_eq!(host.buckets.len(), 1);
}

#[test]
fn release_takes_the_bucket_then_spawns_and_second_click_cannot_repeat_it() {
    let mut host = Probe::release();
    assert_eq!(handle(&mut host, EventKey(1)), Ok(true));
    assert_eq!(host.trace, ["prepare", "cancel", "take", "spawn"]);
    assert_eq!(host.spawned, [STATE]);
    assert!(host.held.is_none());
    assert_eq!(handle(&mut host, EventKey(2)), Ok(false));
    assert_eq!(host.spawned.len(), 1);
}

#[test]
fn a_refused_record_keeps_the_bucket_and_spawns_nothing() {
    for refusal in [Refusal::Damaged, Refusal::Cancelled] {
        let mut host = Probe::release();
        host.spawn_refusal = Some(refusal);
        assert_eq!(
            handle(&mut host, EventKey(1)),
            Err(Denial::Refused(refusal))
        );
        assert_eq!(host.trace, ["prepare", "cancel", "take", "spawn", "return"]);
        assert!(host.spawned.is_empty());
        assert_eq!(host.held.as_deref(), Some(STATE));
        // The kept bucket still works once the reason is gone.
        host.spawn_refusal = None;
        host.trace.clear();
        assert_eq!(handle(&mut host, EventKey(2)), Ok(true));
        assert_eq!(host.spawned, [STATE]);
    }
}

#[test]
fn a_bucket_that_left_the_hand_after_preparation_is_neither_taken_nor_spawned() {
    let mut host = Probe::release();
    let prepared = host.prepare(
        EventKey(1),
        Plan::Release {
            hand: Hand::Main,
            destination: [1, 65, 2],
        },
    );
    let prepared = prepared.expect("prepared");
    host.held = None;
    assert_eq!(host.take_bucket(&prepared), Ok(false));
    assert!(host.spawned.is_empty());
}

#[test]
fn a_losing_removal_gives_nothing_and_keeps_the_empty_bucket() {
    let mut host = Probe::capture();
    let prepared = host
        .prepare(EventKey(1), Plan::Capture { hand: Hand::Main })
        .expect("prepared");
    // Another player's removal wins between this snapshot and this removal.
    host.source.lock().unwrap().take();
    assert_eq!(host.try_remove(&prepared), Ok(false));
    assert!(host.buckets.is_empty());
    assert_eq!(host.view.item, Item::EmptyBucket { count: 1 });
}

#[test]
fn creative_copies_are_independent_and_explicitly_allowed_to_spawn_twice() {
    let mut original = Probe::release();
    let mut copy = Probe::release();
    assert_eq!(handle(&mut original, EventKey(1)), Ok(true));
    assert_eq!(handle(&mut copy, EventKey(2)), Ok(true));
    assert_eq!(original.spawned.len() + copy.spawned.len(), 2);
    assert_eq!(original.spawned[0], copy.spawned[0]);
}

#[test]
fn ordinary_protection_and_placement_rejection_during_preparation_change_nothing() {
    for reason in [
        Denial::Cancelled,
        Denial::Permission,
        Denial::Obstructed,
        Denial::Invalid,
    ] {
        for mut host in [Probe::capture(), Probe::release()] {
            host.prepare_error = Some(reason);
            assert_eq!(handle(&mut host, EventKey(1)), Err(reason));
            assert_eq!(host.trace, ["prepare"]);
            assert!(host.buckets.is_empty() && host.spawned.is_empty());
            assert_eq!(host.source.lock().unwrap().as_deref(), Some(STATE));
        }
    }
}

#[test]
fn death_or_disconnect_before_the_action_is_inert() {
    for mut host in [Probe::capture(), Probe::release()] {
        host.view.connected_alive = false;
        assert_eq!(handle(&mut host, EventKey(1)), Err(Denial::Stale));
        assert!(host.trace.is_empty());
    }
}

#[test]
fn a_failure_after_removal_is_never_retried_or_compensated_with_a_spawn() {
    let mut host = Probe::capture();
    host.fail_at = Some("give");
    assert_eq!(handle(&mut host, EventKey(1)), Err(Denial::Stale));
    assert_eq!(host.trace, ["prepare", "cancel", "try-remove", "give"]);
    assert!(host.source.lock().unwrap().is_none());
    assert!(host.buckets.is_empty() && host.spawned.is_empty());
}

#[test]
fn a_failed_take_spawns_nothing() {
    let mut host = Probe::release();
    host.fail_at = Some("take");
    assert_eq!(handle(&mut host, EventKey(1)), Err(Denial::Stale));
    assert_eq!(host.trace, ["prepare", "cancel", "take"]);
    assert!(host.spawned.is_empty());
    assert_eq!(host.held.as_deref(), Some(STATE));
}

#[test]
fn hopper_shift_click_and_death_drop_move_snapshot_bytes_without_redeeming_them() {
    for movement in ["hopper", "shift-click", "death-drop"] {
        let mut host = Probe::release();
        let container = host.held.take();
        host.view.item = Item::Other;
        assert_eq!(handle(&mut host, EventKey(1)), Ok(false), "{movement}");
        assert!(host.trace.is_empty() && host.spawned.is_empty());
        host.held = container;
        host.view.item = Item::OccupiedBucket { count: 1 };
        assert_eq!(handle(&mut host, EventKey(2)), Ok(true), "{movement}");
        assert_eq!(host.spawned, [STATE], "{movement}");
    }
}

#[test]
fn loss_and_creative_copy_limits_are_explained_in_every_language() {
    for (locale, _) in crate::lang::FILES {
        for key in ["limit_crash", "limit_creative"] {
            assert_ne!(crate::lang::text(locale, key), key);
        }
    }
}
