#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Denial {
    Unsupported,
    Cancelled,
    Permission,
    Stale,
    Full,
    Invalid,
    Obstructed,
    /// An invulnerable, AI-disabled or unaware villager: usually one a plugin manages.
    Protected,
    /// The server refused to make a villager from the bucket's record; the bucket is kept.
    Refused(Refusal),
}

/// Why a release made no villager.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The record is not a villager's, too large, malformed or from a newer server.
    Damaged,
    /// A protection or other plugin cancelled the spawn.
    Cancelled,
}

impl Denial {
    /// The catalog key of what the player is told, if anything. Cancelled actions are
    /// explained by whoever cancelled them; stale ones need no word.
    pub fn message(self) -> Option<&'static str> {
        match self {
            Self::Unsupported | Self::Cancelled | Self::Stale => None,
            Self::Permission => Some("denied_permission"),
            Self::Full => Some("denied_full"),
            Self::Invalid => Some("denied_invalid"),
            Self::Obstructed => Some("denied_obstructed"),
            Self::Protected => Some("denied_protected"),
            Self::Refused(Refusal::Damaged) => Some("refused_damaged"),
            Self::Refused(Refusal::Cancelled) => Some("refused_cancelled"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hand {
    Main,
    Off,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    Down,
    Up,
    North,
    South,
    West,
    East,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    EmptyBucket { count: u8 },
    OccupiedBucket { count: u8 },
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Villager,
    Block { position: [i32; 3], face: Face },
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Interaction {
    pub cancelled: bool,
    pub permitted: bool,
    pub connected_alive: bool,
    pub hand: Option<Hand>,
    pub item: Item,
    pub target: Target,
    /// Space available before removal, not space a hand replacement would free.
    pub output_room: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plan {
    Capture { hand: Hand },
    Release { hand: Hand, destination: [i32; 3] },
}

/// Bounded policy before destructive operations. The adapter prepares the
/// snapshot, capacity and ordinary protection decisions before removing anything.
pub fn plan(view: Interaction) -> Result<Option<Plan>, Denial> {
    let capture = matches!(
        (view.item, view.target),
        (Item::EmptyBucket { .. }, Target::Villager)
    );
    let release = matches!(
        (view.item, view.target),
        (Item::OccupiedBucket { .. }, Target::Block { .. })
    );
    if !capture && !release {
        return Ok(None);
    }
    if view.cancelled {
        return Err(Denial::Cancelled);
    }
    if !view.permitted {
        return Err(Denial::Permission);
    }
    if !view.connected_alive {
        return Err(Denial::Stale);
    }
    let hand = view.hand.ok_or(Denial::Invalid)?;
    match (view.item, view.target) {
        (Item::EmptyBucket { count: 1..=16 }, Target::Villager) => {
            if !view.output_room {
                return Err(Denial::Full);
            }
            Ok(Some(Plan::Capture { hand }))
        }
        (Item::OccupiedBucket { count: 1 }, Target::Block { position, face }) => {
            let offset = match face {
                Face::Down => [0, -1, 0],
                Face::Up => [0, 1, 0],
                Face::North => [0, 0, -1],
                Face::South => [0, 0, 1],
                Face::West => [-1, 0, 0],
                Face::East => [1, 0, 0],
            };
            let mut destination = position;
            for axis in 0..3 {
                if !(-30_000_000..30_000_000).contains(&position[axis]) {
                    return Err(Denial::Invalid);
                }
                destination[axis] = position[axis]
                    .checked_add(offset[axis])
                    .ok_or(Denial::Invalid)?;
                if !(-30_000_000..30_000_000).contains(&destination[axis]) {
                    return Err(Denial::Invalid);
                }
            }
            Ok(Some(Plan::Release { hand, destination }))
        }
        _ => Err(Denial::Invalid),
    }
}
