//! A world saved as a folder of `.chunk` files, world version 41.
//!
//! Every save writes `_main.<n>.chunks`, the list of chunk files that belong
//! to it, and `_main.<n>.ok` once the save is complete.

use std::path::Path;

use tokio::fs;

use crate::{
    error::{Error, IoContext, Result},
    world::{
        Portal, parsed,
        reader::{Malformed, Parsed, Reader},
        zdo::{Object, properties},
    },
};

const VERSION: u16 = 41;

const ROTATION: u8 = 0x10;
/// The position is two whole numbers, x and z, at height 0.
const WHOLE_POSITION: u8 = 0x20;
const UNKNOWN: u8 = 0xc0;

struct Entry {
    file: String,
    objects: usize,
}

pub async fn read(dir: &Path) -> Result<Vec<Portal>> {
    let save = newest_save(dir).await?;
    let index_path = dir.join(format!("_main.{save}.chunks"));
    let data = fs::read(&index_path).await.at(&index_path)?;
    let entries = parsed(&index_path, move || index(&data)).await?;
    let mut portals = Vec::new();
    for entry in entries {
        let path = dir.join(&entry.file);
        let data = fs::read(&path).await.at(&path)?;
        portals.extend(parsed(&path, move || chunk(&data, entry.objects)).await?);
    }
    Ok(portals)
}

/// The number of the newest save that is marked complete.
async fn newest_save(dir: &Path) -> Result<u32> {
    let mut entries = fs::read_dir(dir).await.at(dir)?;
    let mut newest = None;
    while let Some(entry) = entries.next_entry().await.at(dir)? {
        let number = entry.file_name().to_str().and_then(|name| {
            name.strip_prefix("_main.")?
                .strip_suffix(".ok")?
                .parse::<u32>()
                .ok()
        });
        newest = newest.max(number);
    }
    newest.ok_or_else(|| Error::World {
        path: dir.to_path_buf(),
        reason: Malformed::NoSave,
    })
}

fn version(reader: &mut Reader) -> Parsed<()> {
    let version = reader.u16()?;
    if version != VERSION {
        return Err(Malformed::Version(i32::from(version)));
    }
    Ok(())
}

fn index(data: &[u8]) -> Parsed<Vec<Entry>> {
    let mut reader = Reader::new(data);
    version(&mut reader)?;
    // The number of objects in the whole world.
    reader.skip(4)?;
    let count = reader.length()?;
    let mut entries = Vec::new();
    for _ in 0..count {
        let first = reader.u8()?;
        let second = reader.u8()?;
        let level = reader.u8()?;
        let revision = reader.i32()?;
        let objects = reader.length()?;
        // The file name holds the two coordinates in the other order.
        entries.push(Entry {
            file: format!("{second:02x}_{first:02x}__{level}_{revision}.chunk"),
            objects,
        });
    }
    end(&reader)?;
    Ok(entries)
}

fn chunk(data: &[u8], listed: usize) -> Parsed<Vec<Portal>> {
    let mut reader = Reader::new(data);
    version(&mut reader)?;
    let found = reader.length()?;
    if found != listed {
        return Err(Malformed::Count { listed, found });
    }
    let mut portals = Vec::new();
    for _ in 0..found {
        portals.extend(object(&mut reader)?.into_portal());
    }
    end(&reader)?;
    Ok(portals)
}

fn end(reader: &Reader) -> Parsed<()> {
    if reader.is_at_end() {
        Ok(())
    } else {
        Err(Malformed::Trailing(reader.position()))
    }
}

fn object(reader: &mut Reader) -> Parsed<Object> {
    let blocks = reader.u8()?;
    let at = reader.position();
    let flags = reader.u8()?;
    if flags & UNKNOWN != 0 {
        return Err(Malformed::Flags { at, flags });
    }
    let position = if flags & WHOLE_POSITION == 0 {
        reader.vector()?
    } else {
        let x = reader.i16()?;
        let z = reader.i16()?;
        [f32::from(x), 0.0, f32::from(z)]
    };
    let prefab = reader.i32()?;
    // A turn around the up axis takes 2 bytes and sets the high bit of them.
    // Any other rotation takes 4 bytes.
    if flags & ROTATION != 0 && reader.u16()? & 0x8000 == 0 {
        reader.skip(2)?;
    }
    let tag = properties(reader, blocks, true)?;
    Ok(Object {
        prefab,
        position,
        tag,
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;
    use crate::world::{PortalKind, testing::Bytes};

    fn sample_chunk() -> Vec<u8> {
        Bytes::default()
            .raw(&[41, 0])
            .i32(3)
            // A portal that is turned around the up axis, with a connection,
            // a float, a long and two strings.
            .raw(&[0x63, 0x19])
            .vector([483.0, 65.0, -9549.0])
            .hash("portal_wood")
            .raw(&[0x95, 0x81])
            .raw(&[0x11, 0, 0, 0, 0])
            .raw(&[1, 0, 0, 0, 0, 0, 0, 0, 0])
            .raw(&[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
            .raw(&[2])
            .hash("tagauthor")
            .text("someone")
            .hash("tag")
            .text("крепость2")
            // A tree with a full rotation and a float.
            .raw(&[0x02, 0x19])
            .vector([410.0, 18.0, -6164.0])
            .hash("tree")
            .raw(&[0xf6, 0x00, 0xca, 0x72])
            .raw(&[1, 0, 0, 0, 0, 0, 0, 0, 0])
            // An object on a whole position with nothing else.
            .raw(&[0x00, 0x21])
            .raw(&[0x00, 0x10, 0x00, 0xe0])
            .hash("zone")
            .done()
    }

    async fn put(dir: &Path, name: &str, data: &[u8]) -> Result<()> {
        let path = dir.join(name);
        fs::write(&path, data).await.at(&path)
    }

    #[test]
    fn reads_a_chunk() -> Parsed<()> {
        let portals = chunk(&sample_chunk(), 3)?;
        assert_eq!(portals.len(), 1);
        assert_eq!(portals[0].name, "крепость2");
        assert_eq!(portals[0].kind, PortalKind::Wood);
        assert_eq!((portals[0].x, portals[0].z), (483.0, -9549.0));
        Ok(())
    }

    #[test]
    fn a_chunk_must_match_the_index_and_end_with_its_last_object() {
        assert!(matches!(
            chunk(&sample_chunk(), 4),
            Err(Malformed::Count {
                listed: 4,
                found: 3
            })
        ));
        let mut longer = sample_chunk();
        longer.push(0);
        assert!(matches!(chunk(&longer, 3), Err(Malformed::Trailing(_))));
    }

    #[tokio::test]
    async fn reads_the_newest_finished_save_of_a_folder() -> Result<()> {
        let dir = tempdir().at(Path::new("tempdir"))?;
        let index = Bytes::default()
            .raw(&[41, 0])
            .i32(3)
            .i32(1)
            .raw(&[0x01, 0x00, 0])
            .i32(9)
            .i32(3)
            .done();
        put(dir.path(), "_main.7.chunks", &index).await?;
        put(dir.path(), "_main.7.ok", &[41, 0, 0, 0]).await?;
        put(dir.path(), "00_01__0_9.chunk", &sample_chunk()).await?;
        // A save that was never finished, it has no ok file.
        put(dir.path(), "_main.8.chunks", &[0]).await?;

        let portals = read(dir.path()).await?;
        assert_eq!(portals.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn a_folder_without_a_finished_save_is_an_error() -> Result<()> {
        let dir = tempdir().at(Path::new("tempdir"))?;
        let found = read(dir.path()).await;
        assert!(matches!(
            found,
            Err(Error::World {
                reason: Malformed::NoSave,
                ..
            })
        ));
        Ok(())
    }
}
