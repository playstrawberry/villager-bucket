//! The one binding to the server: complete entity snapshots, the checked removal
//! (`entity.try-remove`) and the checked restore (`world.spawn-entity-from-snapshot`).

use std::sync::Arc;

use strawberry_plugin_api::{
    Context, Entity, EntityType, EventHandler, ItemStack, Player, Result, Server, SnapshotError,
    World,
    common::{GameMode, Hand as SdkHand},
    data_components::DataComponent,
    events::{
        EventData, FailureVerdict, HandlerOptions, HandlerPriority, InteractAction,
        PlayerBucketFillEvent, PlayerInteractEntityEvent, PlayerInteractEvent, event_hand,
    },
    events_wit::EntityInteractionAction,
    persistent_data::PersistentDataHolder,
    text::TextComponent,
    world::BlockDirection,
};

use crate::{
    exchange::{BucketHost, EventKey, handle},
    models::{Delivery, Models, Place},
    policy::{Denial, Face, Hand, Interaction, Item, Plan, Refusal, Target},
    snapshot,
};

/// A player reaches a block or an entity within this distance; the server checks reach itself.
const REACH: f64 = 6.0;
/// Main storage slots of a player inventory (hotbar and the three rows).
const STORAGE_SLOTS: usize = 36;

/// What every handler shares: the model client, connected once at load.
pub struct Shared {
    pub models: Models,
}

/// Subscribes the three blocking handlers. Each runs after ordinary protection
/// (`High`) and never on an event that is already cancelled; a fault cancels
/// the event, so a half-run capture or release never lets the vanilla action through.
pub fn subscribe(context: &Context, shared: &Arc<Shared>) -> Result<()> {
    let options = || HandlerOptions {
        priority: HandlerPriority::Highest,
        blocking: true,
        ignore_cancelled: true,
        on_failure: FailureVerdict::Deny,
    };
    context.register_event_handler_with_options::<PlayerInteractEntityEvent, _>(
        Capture(Arc::clone(shared)),
        options(),
    )?;
    context.register_event_handler_with_options::<PlayerInteractEvent, _>(
        Release(Arc::clone(shared)),
        options(),
    )?;
    context
        .register_event_handler_with_options::<PlayerBucketFillEvent, _>(FluidGuard, options())?;
    Ok(())
}

struct Capture(Arc<Shared>);
struct Release(Arc<Shared>);
struct FluidGuard;

impl EventHandler<PlayerInteractEntityEvent> for Capture {
    fn handle(
        &self,
        _server: Server,
        mut event: EventData<PlayerInteractEntityEvent>,
    ) -> EventData<PlayerInteractEntityEvent> {
        // Cheapest first: an ordinary trade click without an empty bucket costs two reads.
        if event.action == EntityInteractionAction::Attack || capture_hand(&event.player).is_none()
        {
            return event;
        }
        let Some(target) = target_villager(&event.player, event.entity_id) else {
            return event;
        };
        // The client sends `interact-at` before `interact` for one click: only the
        // second acts, the first is only kept from opening the trading screen.
        if event.action == EntityInteractionAction::InteractAt {
            event.cancelled = true;
            return event;
        }
        let mut live = Live::new(&self.0, &event.player, Some(target), None);
        event.cancelled |= live.run();
        event
    }
}

impl EventHandler<PlayerInteractEvent> for Release {
    fn handle(
        &self,
        _server: Server,
        mut event: EventData<PlayerInteractEvent>,
    ) -> EventData<PlayerInteractEvent> {
        if event.action != InteractAction::RightClickBlock {
            return event;
        }
        let (Some(clicked), Some(hand)) = (event.clicked_pos, event_hand()) else {
            return event;
        };
        let hand = from_sdk(hand);
        if !matches!(
            classify(stack_in(&event.player, hand).as_ref()),
            Item::OccupiedBucket { .. }
        ) {
            return event;
        }
        let block = Clicked {
            hand,
            position: [clicked.x, clicked.y, clicked.z],
        };
        let mut live = Live::new(&self.0, &event.player, None, Some(block));
        event.cancelled |= live.run();
        event
    }
}

impl EventHandler<PlayerBucketFillEvent> for FluidGuard {
    fn handle(
        &self,
        _server: Server,
        mut event: EventData<PlayerBucketFillEvent>,
    ) -> EventData<PlayerBucketFillEvent> {
        // The vanilla fallback is a bucket item: it must never scoop up a fluid and
        // turn into a water or lava bucket, dropping the villager's record.
        let main = stack_in(&event.player, Hand::Main);
        let used = if main.as_ref().is_some_and(is_bucket) {
            main
        } else {
            stack_in(&event.player, Hand::Off)
        };
        if matches!(classify(used.as_ref()), Item::OccupiedBucket { .. }) {
            event.cancelled = true;
        }
        event
    }
}

#[derive(Clone, Copy)]
struct Clicked {
    hand: Hand,
    position: [i32; 3],
}

/// One event's adapter. `EventKey` is unused: the adapter itself is event-scoped.
struct Live<'a> {
    shared: &'a Shared,
    player: &'a Player,
    target: Option<Entity>,
    block: Option<Clicked>,
    cancelled: bool,
}

pub struct Prepared {
    hand: Hand,
    record: Vec<u8>,
    kind: Kind,
}

enum Kind {
    Capture {
        count: u8,
    },
    Release {
        held: ItemStack,
        world: World,
        at: (f64, f64, f64),
    },
}

impl<'a> Live<'a> {
    fn new(
        shared: &'a Shared,
        player: &'a Player,
        target: Option<Entity>,
        block: Option<Clicked>,
    ) -> Self {
        Self {
            shared,
            player,
            target,
            block,
            cancelled: false,
        }
    }

    /// Runs the transfer and tells the player why it did not happen. Returns whether
    /// the event is to be cancelled.
    fn run(&mut self) -> bool {
        match handle(self, EventKey(0)) {
            Ok(_) => self.cancelled,
            Err(denial) => {
                if let Some(key) = denial.message() {
                    let locale = self.player.get_locale();
                    self.player.send_system_message(
                        TextComponent::text(&crate::lang::text(&locale, key)),
                        false,
                    );
                }
                // A refused bucket action never falls through to the vanilla one.
                self.cancelled || !matches!(denial, Denial::Stale | Denial::Unsupported)
            }
        }
    }

    fn place_or_drop(&self, stack: ItemStack) {
        let inventory = self.player.get_inventory().as_inventory();
        if let Some(rest) = inventory.add_item(stack) {
            self.player
                .get_world()
                .drop_item(self.player.get_position(), &rest, false);
        }
    }
}

impl BucketHost for Live<'_> {
    type Prepared = Prepared;

    fn inspect(&self, _event: EventKey) -> std::result::Result<Interaction, Denial> {
        let player = self.player;
        let connected_alive =
            player.get_health() > 0.0 && player.get_gamemode() != GameMode::Spectator;
        let (hand, item, target) = if self.target.is_some() {
            let hand = capture_hand(player);
            let item = hand.map_or(Item::Other, |hand| {
                classify(stack_in(player, hand).as_ref())
            });
            (hand, item, Target::Villager)
        } else if let Some(block) = self.block {
            let item = classify(stack_in(player, block.hand).as_ref());
            let face = clicked_face(player, block.position).ok_or(Denial::Invalid)?;
            let target = Target::Block {
                position: block.position,
                face,
            };
            (Some(block.hand), item, target)
        } else {
            (None, Item::Other, Target::Other)
        };
        Ok(Interaction {
            cancelled: false,
            permitted: player.has_permission(crate::USE_PERMISSION),
            connected_alive,
            hand,
            item,
            target,
            // Only a capture needs room; a release gives nothing.
            output_room: self.target.is_some() && has_room(player),
        })
    }

    fn prepare(&mut self, _event: EventKey, plan: Plan) -> std::result::Result<Prepared, Denial> {
        match plan {
            Plan::Capture { hand } => {
                let villager = self.target.as_ref().ok_or(Denial::Stale)?;
                guard(villager)?;
                let count = match classify(stack_in(self.player, hand).as_ref()) {
                    Item::EmptyBucket { count } => count,
                    _ => return Err(Denial::Stale),
                };
                // The whole villager first; nothing is removed without its record in hand.
                let record = villager.snapshot().map_err(|error| match error {
                    SnapshotError::NotAllowed => Denial::Stale,
                    _ => Denial::Protected,
                })?;
                if !snapshot::is_villager(&record) {
                    return Err(Denial::Protected);
                }
                Ok(Prepared {
                    hand,
                    record,
                    kind: Kind::Capture { count },
                })
            }
            Plan::Release { hand, destination } => {
                let held = stack_in(self.player, hand).ok_or(Denial::Stale)?;
                let record = record_of(&held).ok_or(Denial::Stale)?;
                if !snapshot::is_villager(&record) {
                    return Err(Denial::Refused(Refusal::Damaged));
                }
                let world = self.player.get_world();
                let at = free_spot(&world, destination)?;
                Ok(Prepared {
                    hand,
                    record,
                    kind: Kind::Release { held, world, at },
                })
            }
        }
    }

    fn cancel(&mut self, _event: EventKey) -> std::result::Result<(), Denial> {
        self.cancelled = true;
        Ok(())
    }

    fn try_remove(&mut self, _prepared: &Prepared) -> std::result::Result<bool, Denial> {
        let villager = self.target.as_ref().ok_or(Denial::Stale)?;
        Ok(villager.try_remove())
    }

    /// Never fails once the villager is removed: the bucket lands in the hand, the
    /// inventory, or at the player's feet, so the villager is not lost.
    fn give_bucket(&mut self, prepared: Prepared) -> std::result::Result<(), Denial> {
        let Kind::Capture { count } = prepared.kind else {
            return Err(Denial::Invalid);
        };
        let player = self.player;
        let inventory = player.get_inventory();
        let off = prepared.hand == Hand::Off;
        let locale = player.get_locale();
        let vanilla = || occupied_stack(&locale, &prepared.record);
        if count <= 1 {
            // The empty bucket becomes the occupied one, in the same hand.
            let attached = match self.shared.models.hand_visual(player, off) {
                Ok(Delivery::Model(_)) => attach(
                    stack_in(player, prepared.hand),
                    &self.shared.models,
                    &prepared.record,
                ),
                _ => None,
            };
            inventory.set_item_in_hand(
                to_sdk(prepared.hand),
                Some(attached.unwrap_or_else(vanilla)),
            );
            return Ok(());
        }
        // One bucket of the stack is used; the occupied bucket goes into the inventory.
        if let Some(stack) = stack_in(player, prepared.hand) {
            stack.set_count(count - 1);
            inventory.set_item_in_hand(to_sdk(prepared.hand), Some(stack));
        }
        let slots = inventory.as_inventory();
        let delivered = match self.shared.models.give_visual(player) {
            Ok(Delivery::Model(Place::Slot(slot))) => {
                let slot = u32::from(slot);
                attach(slots.get_item(slot), &self.shared.models, &prepared.record)
                    .map(|stack| slots.set_item(slot, Some(stack)))
                    .is_some()
            }
            _ => false,
        };
        if !delivered {
            self.place_or_drop(vanilla());
        }
        Ok(())
    }

    fn take_bucket(&mut self, prepared: &Prepared) -> std::result::Result<bool, Denial> {
        let still_held = stack_in(self.player, prepared.hand).is_some_and(|stack| {
            stack.get_count() == 1
                && record_of(&stack).as_deref() == Some(prepared.record.as_slice())
        });
        if !still_held {
            return Ok(false);
        }
        self.player.get_inventory().set_item_in_hand(
            to_sdk(prepared.hand),
            Some(ItemStack::new("minecraft:bucket", 1)),
        );
        Ok(true)
    }

    fn spawn_villager(&mut self, prepared: &Prepared) -> std::result::Result<(), Denial> {
        let Kind::Release { world, at, .. } = &prepared.kind else {
            return Err(Denial::Invalid);
        };
        world
            .spawn_entity_from_snapshot(&prepared.record, *at)
            .map(drop)
            .map_err(|error| {
                Denial::Refused(match error {
                    SnapshotError::Cancelled => Refusal::Cancelled,
                    SnapshotError::TooLarge
                    | SnapshotError::Malformed
                    | SnapshotError::NotAllowed => Refusal::Damaged,
                })
            })
    }

    fn return_bucket(&mut self, prepared: Prepared) -> std::result::Result<(), Denial> {
        let Kind::Release { held, .. } = prepared.kind else {
            return Err(Denial::Invalid);
        };
        let inventory = self.player.get_inventory();
        let hand = to_sdk(prepared.hand);
        // Back into the hand when it still holds the empty bucket put there; else into
        // the inventory or at the player's feet. Never lost, never doubled.
        let untouched = inventory
            .get_item_in_hand(hand)
            .is_some_and(|stack| is_plain_bucket(&stack) && stack.get_count() == 1);
        if untouched {
            inventory.set_item_in_hand(hand, Some(held));
        } else {
            self.place_or_drop(held);
        }
        Ok(())
    }
}

/// The villager the player clicked, found by its id along the player's view.
fn target_villager(player: &Player, entity_id: i32) -> Option<Entity> {
    player
        .as_entity()
        .get_target_entity(REACH)
        .filter(|entity| entity.get_id() as i32 == entity_id)
        .filter(|entity| entity.get_type() == EntityType::Villager)
}

/// A villager a plugin manages (a shop keeper) is not taken: invulnerable, AI off or unaware.
fn guard(villager: &Entity) -> std::result::Result<(), Denial> {
    if villager.as_living().is_some_and(|living| living.is_dead()) {
        return Err(Denial::Stale);
    }
    let managed = villager.is_invulnerable()
        || villager
            .as_mob()
            .is_some_and(|mob| mob.is_ai_disabled() || !mob.is_aware());
    if managed {
        Err(Denial::Protected)
    } else {
        Ok(())
    }
}

/// The hand holding an empty bucket: the main hand first, as the client tries it first.
fn capture_hand(player: &Player) -> Option<Hand> {
    [Hand::Main, Hand::Off].into_iter().find(|hand| {
        matches!(
            classify(stack_in(player, *hand).as_ref()),
            Item::EmptyBucket { .. }
        )
    })
}

/// The face of `position` the player looks at, as the server traces it.
fn clicked_face(player: &Player, position: [i32; 3]) -> Option<Face> {
    let hit = player.as_entity().ray_trace_block(REACH, false)?;
    if [hit.pos.x, hit.pos.y, hit.pos.z] != position {
        return None;
    }
    Some(match hit.face {
        BlockDirection::Down => Face::Down,
        BlockDirection::Up => Face::Up,
        BlockDirection::North => Face::North,
        BlockDirection::South => Face::South,
        BlockDirection::West => Face::West,
        BlockDirection::East => Face::East,
    })
}

/// Where the villager stands: two free blocks in a loaded chunk, inside the world's
/// height and border. Nothing loads a chunk or picks another spot.
fn free_spot(world: &World, at: [i32; 3]) -> std::result::Result<(f64, f64, f64), Denial> {
    if !world.is_chunk_loaded(at[0] >> 4, at[2] >> 4) {
        return Err(Denial::Obstructed);
    }
    let bottom = world.get_min_y();
    let top = i64::from(bottom) + i64::from(world.get_height()) - 2;
    if at[1] < bottom || i64::from(at[1]) > top {
        return Err(Denial::Obstructed);
    }
    let (x, y, z) = (
        f64::from(at[0]) + 0.5,
        f64::from(at[1]),
        f64::from(at[2]) + 0.5,
    );
    if !world.get_border().contains(x, z) {
        return Err(Denial::Obstructed);
    }
    for dy in 0..2 {
        let state = world.get_block_state(strawberry_plugin_api::common::BlockPos {
            x: at[0],
            y: at[1] + dy,
            z: at[2],
        });
        if !state.collision_shapes.is_empty() || state.block_name.ends_with("lava") {
            return Err(Denial::Obstructed);
        }
    }
    Ok((x, y, z))
}

fn has_room(player: &Player) -> bool {
    let slots = player.get_inventory().as_inventory().get_all_items();
    slots.len() >= STORAGE_SLOTS
        && slots[..STORAGE_SLOTS]
            .iter()
            .any(|slot| slot.as_ref().is_none_or(|stack| stack.get_count() == 0))
}

fn stack_in(player: &Player, hand: Hand) -> Option<ItemStack> {
    player
        .get_inventory()
        .get_item_in_hand(to_sdk(hand))
        .filter(|stack| stack.get_count() > 0)
}

/// What a stack is to this plugin: only its own record makes an occupied bucket; a
/// name, lore or model id alone never does.
fn classify(stack: Option<&ItemStack>) -> Item {
    let Some(stack) = stack else {
        return Item::Other;
    };
    let count = stack.get_count();
    if stack.has_custom_data(snapshot::NAMESPACE, snapshot::KEY) {
        Item::OccupiedBucket { count }
    } else if is_plain_bucket(stack) {
        Item::EmptyBucket { count }
    } else {
        Item::Other
    }
}

fn is_bucket(stack: &ItemStack) -> bool {
    stack.get_registry_key().trim_start_matches("minecraft:") == "bucket"
}

fn is_plain_bucket(stack: &ItemStack) -> bool {
    is_bucket(stack) && !stack.has_custom_data(snapshot::NAMESPACE, snapshot::KEY)
}

fn record_of(stack: &ItemStack) -> Option<Vec<u8>> {
    stack
        .get_byte_array(snapshot::NAMESPACE, snapshot::KEY)
        .map(|bytes| snapshot::from_item(&bytes))
}

/// The vanilla fallback: a plain bucket with the localized name, lore and the record.
fn occupied_stack(locale: &str, record: &[u8]) -> ItemStack {
    let stack = crate::appearance::vanilla_stack(locale);
    stack.set_byte_array(
        snapshot::NAMESPACE,
        snapshot::KEY,
        snapshot::to_item(record),
    );
    // Stack size one (VarInt 1); the release refuses a stack of more than one anyway.
    let _ = stack.try_set_component(DataComponent::MaxStackSize, &[1]);
    stack
}

/// Puts the record on a model item the shared service just delivered; `None` when the
/// place does not hold that fresh model item.
fn attach(stack: Option<ItemStack>, models: &Models, record: &[u8]) -> Option<ItemStack> {
    let stack = stack?;
    if models.identify(&stack) != crate::models::VisualId::VillagerBucket
        || stack.has_custom_data(snapshot::NAMESPACE, snapshot::KEY)
    {
        return None;
    }
    stack.set_byte_array(
        snapshot::NAMESPACE,
        snapshot::KEY,
        snapshot::to_item(record),
    );
    Some(stack)
}

fn to_sdk(hand: Hand) -> SdkHand {
    match hand {
        Hand::Main => SdkHand::Right,
        Hand::Off => SdkHand::Left,
    }
}

fn from_sdk(hand: SdkHand) -> Hand {
    match hand {
        SdkHand::Right => Hand::Main,
        SdkHand::Left => Hand::Off,
    }
}
