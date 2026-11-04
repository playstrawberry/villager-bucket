#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Denial {
    Unsupported,
    Cancelled,
    Permission,
    Stale,
    Full,
    Invalid,
    Obstructed,
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
