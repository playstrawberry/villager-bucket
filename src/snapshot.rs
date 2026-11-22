//! The occupied bucket's record: the engine's entity snapshot, kept whole in the item's
//! custom data. The engine checks the bytes again in full when it makes the villager; this
//! module only refuses, before anything changes, a record that is not a villager's.

/// Custom data namespace and key of the snapshot on an occupied bucket.
pub const NAMESPACE: &str = "villager-bucket";
pub const KEY: &str = "snapshot";
/// The engine's limit for one snapshot (`entity.snapshot`).
pub const MAX_BYTES: usize = 65_536;
/// NBT's own nesting limit.
const MAX_DEPTH: u16 = 512;

const END: u8 = 0;
const STRING: u8 = 8;
const LIST: u8 = 9;
const COMPOUND: u8 = 10;

/// Item custom data keeps bytes as signed bytes.
pub fn to_item(bytes: &[u8]) -> Vec<i8> {
    bytes.iter().map(|&byte| byte as i8).collect()
}

pub fn from_item(bytes: &[i8]) -> Vec<u8> {
    bytes.iter().map(|&byte| byte as u8).collect()
}

/// Whether `bytes` is one whole unnamed NBT compound, within the size limit, whose
/// single root `id` names a villager. Anything else (another mob, no id, two ids,
/// bytes after the compound, a cut-off record) is refused.
pub fn is_villager(bytes: &[u8]) -> bool {
    bytes.len() <= MAX_BYTES
        && root_id(bytes).is_some_and(|id| id == "minecraft:villager" || id == "villager")
}

/// The root compound's `id` string, when the record is well formed and has exactly one.
pub fn root_id(bytes: &[u8]) -> Option<String> {
    let mut reader = Reader { bytes, at: 0 };
    if reader.u8()? != COMPOUND {
        return None;
    }
    let mut id = None;
    loop {
        let tag = reader.u8()?;
        if tag == END {
            break;
        }
        let name = reader.string()?;
        if name == b"id" && tag == STRING {
            let value = std::str::from_utf8(reader.string()?).ok()?;
            if id.replace(value.to_owned()).is_some() {
                return None;
            }
        } else {
            reader.skip(tag, 1)?;
        }
    }
    (reader.at == bytes.len()).then_some(id).flatten()
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Option<&'a [u8]> {
        let end = self.at.checked_add(count)?;
        let slice = self.bytes.get(self.at..end)?;
        self.at = end;
        Some(slice)
    }

    fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|slice| slice[0])
    }

    fn length(&mut self) -> Option<usize> {
        let slice = self.take(4)?;
        let value = i32::from_be_bytes([slice[0], slice[1], slice[2], slice[3]]);
        usize::try_from(value).ok()
    }

    fn string(&mut self) -> Option<&'a [u8]> {
        let slice = self.take(2)?;
        let length = usize::from(u16::from_be_bytes([slice[0], slice[1]]));
        self.take(length)
    }

    /// Skips one payload of `tag`, refusing unknown tags and nesting deeper than NBT allows.
    fn skip(&mut self, tag: u8, depth: u16) -> Option<()> {
        if depth > MAX_DEPTH {
            return None;
        }
        match tag {
            1 => self.take(1).map(drop),
            2 => self.take(2).map(drop),
            3 | 5 => self.take(4).map(drop),
            4 | 6 => self.take(8).map(drop),
            7 => {
                let count = self.length()?;
                self.take(count).map(drop)
            }
            STRING => self.string().map(drop),
            LIST => {
                let element = self.u8()?;
                let count = self.length()?;
                // An empty element type carries no bytes, so only an empty list may have it.
                if element == END {
                    return (count == 0).then_some(());
                }
                for _ in 0..count {
                    self.skip(element, depth + 1)?;
                }
                Some(())
            }
            COMPOUND => loop {
                let inner = self.u8()?;
                if inner == END {
                    return Some(());
                }
                self.string()?;
                self.skip(inner, depth + 1)?;
            },
            11 => {
                let count = self.length()?;
                self.take(count.checked_mul(4)?).map(drop)
            }
            12 => {
                let count = self.length()?;
                self.take(count.checked_mul(8)?).map(drop)
            }
            _ => None,
        }
    }
}
