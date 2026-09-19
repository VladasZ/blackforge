//! The parts of a saved game object that every save layout shares.

use crate::world::{
    Portal, PortalKind,
    reader::{Parsed, Reader},
};

/// The game names properties and prefabs by this hash of the text. The C#
/// code hashes utf-16 units, so this is the same value for ASCII text only.
pub const fn stable_hash(text: &str) -> i32 {
    let bytes = text.as_bytes();
    let mut first = 5381_i32;
    let mut second = 5381_i32;
    let mut index = 0;
    while index < bytes.len() {
        first = (first << 5).wrapping_add(first) ^ bytes[index] as i32;
        if index + 1 < bytes.len() {
            second = (second << 5).wrapping_add(second) ^ bytes[index + 1] as i32;
        }
        index += 2;
    }
    first.wrapping_add(second.wrapping_mul(1_566_083_941))
}

const TAG: i32 = stable_hash("tag");
const PORTAL_WOOD: i32 = stable_hash("portal_wood");
const PORTAL_STONE: i32 = stable_hash("portal_stone");

const CONNECTION: u8 = 0x01;
const STRINGS: u8 = 0x40;
const BYTE_ARRAYS: u8 = 0x80;

/// The blocks of fixed size values, as the flag and the bytes of one entry
/// with its key: floats, vectors, quaternions, ints and longs.
const FIXED_BLOCKS: [(u8, usize); 5] = [(0x02, 8), (0x04, 16), (0x08, 20), (0x10, 8), (0x20, 12)];

pub struct Object {
    pub prefab: i32,
    pub position: [f32; 3],
    pub tag: Option<String>,
}

impl Object {
    /// An object with a portal name is a portal too, a mod can add portals
    /// under its own prefab names.
    pub fn into_portal(self) -> Option<Portal> {
        let kind = match self.prefab {
            PORTAL_WOOD => PortalKind::Wood,
            PORTAL_STONE => PortalKind::Stone,
            _ if self.tag.is_some() => PortalKind::Other,
            _ => return None,
        };
        let [x, y, z] = self.position;
        Some(Portal {
            name: self.tag.unwrap_or_default(),
            kind,
            x,
            y,
            z,
        })
    }
}

/// Reads the property blocks that `flags` names, in saves from version 31
/// on. Gives the portal name when the object has one.
pub fn properties(reader: &mut Reader, flags: u8, wide: bool) -> Parsed<Option<String>> {
    if flags & CONNECTION != 0 {
        reader.skip(5)?;
    }
    for (flag, size) in FIXED_BLOCKS {
        if flags & flag != 0 {
            let count = reader.count(wide)?;
            reader.skip(count * size)?;
        }
    }
    let mut tag = None;
    if flags & STRINGS != 0 {
        let count = reader.count(wide)?;
        tag = strings(reader, count)?;
    }
    if flags & BYTE_ARRAYS != 0 {
        for _ in 0..reader.count(wide)? {
            reader.skip(4)?;
            let length = reader.length()?;
            reader.skip(length)?;
        }
    }
    Ok(tag)
}

pub fn strings(reader: &mut Reader, count: usize) -> Parsed<Option<String>> {
    let mut tag = None;
    for _ in 0..count {
        if reader.i32()? == TAG {
            tag = Some(reader.text()?);
        } else {
            reader.skip_text()?;
        }
    }
    Ok(tag)
}

#[cfg(test)]
mod tests {
    use super::*;

    // The expected bytes are copied out of real saves.
    #[test]
    fn hashes_match_the_game() {
        assert_eq!(TAG.to_le_bytes(), [0xea, 0x91, 0x7c, 0x29]);
        assert_eq!(PORTAL_WOOD.to_le_bytes(), [0xc4, 0x77, 0x8c, 0xd8]);
        assert_eq!(PORTAL_STONE.to_le_bytes(), [0x1a, 0x28, 0x89, 0x6e]);
    }
}
