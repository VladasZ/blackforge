//! A world saved as one `.db` file, world versions 26 to 35.

use crate::world::{
    Portal,
    reader::{Malformed, Parsed, Reader},
    zdo::{Object, properties, strings},
};

/// The version of the first public release of the game.
const OLDEST: i32 = 26;
const NEWEST: i32 = 35;
/// From here an object starts with flags that say which blocks follow.
const FLAGS_LAYOUT: i32 = 31;
const WIDE_COUNTS: i32 = 33;

const ROTATION: u16 = 0x1000;

pub fn parse(data: &[u8]) -> Parsed<Vec<Portal>> {
    let mut reader = Reader::new(data);
    let version = reader.i32()?;
    if !(OLDEST..=NEWEST).contains(&version) {
        return Err(Malformed::Version(version));
    }
    // The net time, the id of the writer and the next free object id.
    reader.skip(20)?;
    let count = reader.length()?;
    let mut portals = Vec::new();
    for _ in 0..count {
        let object = if version < FLAGS_LAYOUT {
            old_object(&mut reader)?
        } else {
            object(&mut reader, version >= WIDE_COUNTS)?
        };
        portals.extend(object.into_portal());
    }
    Ok(portals)
}

fn object(reader: &mut Reader, wide: bool) -> Parsed<Object> {
    let flags = reader.u16()?;
    // The sector.
    reader.skip(4)?;
    let position = reader.vector()?;
    let prefab = reader.i32()?;
    if flags & ROTATION != 0 {
        reader.skip(12)?;
    }
    let [blocks, _] = flags.to_le_bytes();
    let tag = properties(reader, blocks, wide)?;
    Ok(Object {
        prefab,
        position,
        tag,
    })
}

fn old_object(reader: &mut Reader) -> Parsed<Object> {
    // The object id.
    reader.skip(12)?;
    let length = reader.length()?;
    let end = reader.position() + length;
    // Two revisions, persistent, the owner, the creation time, the version
    // of the world generator, the type and distant.
    reader.skip(31)?;
    let prefab = reader.i32()?;
    // The sector.
    reader.skip(8)?;
    let position = reader.vector()?;
    // The rotation.
    reader.skip(16)?;
    // Floats, vectors, quaternions, ints and longs, every entry with its key.
    for size in [8, 16, 20, 8, 12] {
        let count = reader.char_count()?;
        reader.skip(count * size)?;
    }
    let count = reader.char_count()?;
    let tag = strings(reader, count)?;
    // Byte arrays follow from version 27 on, the length steps over them.
    let rest = end
        .checked_sub(reader.position())
        .ok_or(Malformed::Truncated(end))?;
    reader.skip(rest)?;
    Ok(Object {
        prefab,
        position,
        tag,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{PortalKind, testing::Bytes};

    fn header(version: i32, objects: i32) -> Bytes {
        Bytes::default().i32(version).raw(&[0; 20]).i32(objects)
    }

    #[test]
    fn reads_the_layout_with_flags() -> Parsed<()> {
        let data = header(35, 2)
            // A rock with a rotation and one float.
            .raw(&[0x02, 0x1b, 0, 0, 0, 0])
            .vector([1.0, 2.0, 3.0])
            .hash("rock")
            .raw(&[0; 12])
            .raw(&[1, 0, 0, 0, 0, 0, 0, 0, 0])
            // A portal with one long and two strings.
            .raw(&[0x60, 0x09, 0, 0, 0, 0])
            .vector([-249.0, 35.0, -408.0])
            .hash("portal_stone")
            .raw(&[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
            .raw(&[2])
            .hash("tagauthor")
            .text("someone")
            .hash("tag")
            .text("дом")
            .done();
        let portals = parse(&data)?;
        assert_eq!(portals.len(), 1);
        assert_eq!(portals[0].name, "дом");
        assert_eq!(portals[0].kind, PortalKind::Stone);
        assert_eq!((portals[0].x, portals[0].z), (-249.0, -408.0));
        Ok(())
    }

    #[test]
    fn reads_the_layout_before_flags() -> Parsed<()> {
        let portal = Bytes::default()
            .raw(&[0; 31])
            .hash("portal_wood")
            .raw(&[0; 8])
            .vector([10.0, 31.0, -20.0])
            .raw(&[0; 16])
            .raw(&[0, 0, 0, 0, 0, 1])
            .hash("tag")
            .text("home")
            // The count of byte arrays, only the length covers it.
            .raw(&[0])
            .done();
        let data = header(27, 1).raw(&[0; 12]).sized(&portal).done();
        let portals = parse(&data)?;
        assert_eq!(portals.len(), 1);
        assert_eq!(portals[0].name, "home");
        assert_eq!(portals[0].kind, PortalKind::Wood);
        Ok(())
    }

    #[test]
    fn refuses_a_version_it_does_not_know() {
        let data = header(36, 0).done();
        assert!(matches!(parse(&data), Err(Malformed::Version(36))));
    }
}
