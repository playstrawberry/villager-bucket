//! The record check that runs before a release changes anything. The engine checks the
//! bytes again in full; these tests pin what this plugin itself refuses.
use crate::snapshot::{MAX_BYTES, from_item, is_villager, root_id, to_item};

/// A tiny unnamed-NBT writer for the fixtures (type byte, then the compound's entries).
struct Nbt(Vec<u8>);

impl Nbt {
    fn root() -> Self {
        Self(vec![10])
    }
    fn name(mut self, tag: u8, name: &str) -> Self {
        self.0.push(tag);
        self.0.extend((name.len() as u16).to_be_bytes());
        self.0.extend(name.as_bytes());
        self
    }
    fn string(self, name: &str, value: &str) -> Self {
        let mut this = self.name(8, name);
        this.0.extend((value.len() as u16).to_be_bytes());
        this.0.extend(value.as_bytes());
        this
    }
    fn int(self, name: &str, value: i32) -> Self {
        let mut this = self.name(3, name);
        this.0.extend(value.to_be_bytes());
        this
    }
    fn raw(mut self, bytes: &[u8]) -> Self {
        self.0.extend(bytes);
        self
    }
    fn end(mut self) -> Vec<u8> {
        self.0.push(0);
        self.0
    }
}

/// A villager record shaped like the engine's: trades in a list of compounds, gossip
/// with an int array, a name and a nested brain, the id not first.
fn villager() -> Vec<u8> {
    let offer = Nbt(Vec::new())
        .int("uses", 3)
        .int("maxUses", 4)
        .string("id", "minecraft:emerald")
        .end();
    let mut recipes = vec![10];
    recipes.extend(2i32.to_be_bytes());
    recipes.extend(&offer);
    recipes.extend(&offer);
    Nbt::root()
        .string("CustomName", "Test Villager")
        .name(10, "Offers")
        .name(9, "Recipes")
        .raw(&recipes)
        .raw(&[0])
        .name(9, "Gossips")
        .raw(&[10, 0, 0, 0, 1])
        .name(11, "Target")
        .raw(&[0, 0, 0, 2, 0, 0, 0, 1, 0, 0, 0, 2])
        .raw(&[0])
        .name(10, "Brain")
        .name(10, "memories")
        .raw(&[0, 0])
        .string("id", "minecraft:villager")
        .int("DataVersion", 4_000)
        .end()
}

#[test]
fn a_villager_record_with_nested_trades_and_gossip_is_accepted() {
    let record = villager();
    assert_eq!(root_id(&record).as_deref(), Some("minecraft:villager"));
    assert!(is_villager(&record));
    // A trade's own `id` deep inside is not the entity's id.
    assert!(record.windows(17).any(|part| part == b"minecraft:emerald"));
}

#[test]
fn another_entity_is_refused_before_anything_is_made() {
    for id in [
        "minecraft:wither",
        "minecraft:wandering_trader",
        "minecraft:zombie_villager",
        "",
    ] {
        let record = Nbt::root().string("id", id).end();
        assert!(!is_villager(&record), "{id}");
    }
    assert!(is_villager(&Nbt::root().string("id", "villager").end()));
}

#[test]
fn records_without_one_clear_id_are_refused() {
    let missing = Nbt::root().int("Age", 0).end();
    let twice = Nbt::root()
        .string("id", "minecraft:villager")
        .string("id", "minecraft:villager")
        .end();
    let not_a_string = Nbt::root().int("id", 1).end();
    for record in [missing, twice, not_a_string] {
        assert!(!is_villager(&record));
    }
}

#[test]
fn cut_off_trailing_or_malformed_bytes_are_refused() {
    let record = villager();
    for cut in [0, 1, 2, record.len() / 2, record.len() - 1] {
        assert!(!is_villager(&record[..cut]), "cut at {cut}");
    }
    let mut trailing = record.clone();
    trailing.push(0);
    assert!(!is_villager(&trailing));
    let mut named_root = vec![10, 0, 1, b'x'];
    named_root.extend(&record[1..]);
    assert!(!is_villager(&named_root));
    assert!(!is_villager(&[8, 0, 0]));
    // An unknown tag type and a negative length.
    assert!(!is_villager(&Nbt::root().name(13, "x").end()));
    let negative = Nbt::root()
        .name(7, "bytes")
        .raw(&(-1i32).to_be_bytes())
        .string("id", "minecraft:villager")
        .end();
    assert!(!is_villager(&negative));
}

#[test]
fn a_list_of_empty_tags_with_entries_cannot_spin() {
    let record = Nbt::root()
        .name(9, "x")
        .raw(&[0])
        .raw(&i32::MAX.to_be_bytes())
        .string("id", "minecraft:villager")
        .end();
    assert!(!is_villager(&record));
    let empty = Nbt::root()
        .name(9, "x")
        .raw(&[0, 0, 0, 0, 0])
        .string("id", "minecraft:villager")
        .end();
    assert!(is_villager(&empty));
}

#[test]
fn nesting_deeper_than_nbt_allows_is_refused_without_overflow() {
    let deep = |levels: usize| {
        let mut record = Nbt::root().string("id", "minecraft:villager").0;
        for _ in 0..levels {
            record.extend([10, 0, 1, b'n']);
        }
        record.extend(std::iter::repeat_n(0, levels + 1));
        record
    };
    assert!(is_villager(&deep(400)));
    assert!(!is_villager(&deep(600)));
}

#[test]
fn records_over_the_engine_limit_are_refused() {
    let padding = "p".repeat(u16::MAX as usize);
    let record = Nbt::root()
        .string("id", "minecraft:villager")
        .string("a", &padding)
        .string("b", &padding)
        .end();
    assert!(record.len() > MAX_BYTES);
    assert!(!is_villager(&record));
}

#[test]
fn item_bytes_round_trip_exactly() {
    let record: Vec<u8> = (0..=255).collect();
    assert_eq!(from_item(&to_item(&record)), record);
}
