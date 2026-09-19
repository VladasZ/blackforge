//! Reads the portals out of a Valheim world save.
//!
//! Up to world version 35 a world is one `.db` file. A world of version 41
//! is a folder of `.chunk` files. Both name a portal with the `tag` string
//! of the portal object.

mod chunks;
mod db;
mod reader;
#[cfg(test)]
mod testing;
mod zdo;

use std::{collections::BTreeMap, path::Path};

use tokio::{fs, task::spawn_blocking};

pub use crate::world::reader::Malformed;
use crate::{
    error::{Error, IoContext, Result},
    world::reader::Parsed,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortalKind {
    Wood,
    Stone,
    /// Any other object that carries a portal name, a mod added it.
    Other,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Portal {
    /// Empty when the portal was never named. The game links two such
    /// portals like any other pair.
    pub name: String,
    pub kind: PortalKind,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// The portals of the world at `world`, a `.db` file or a world folder,
/// sorted by name.
pub async fn portals(world: &Path) -> Result<Vec<Portal>> {
    let mut portals = if fs::metadata(world).await.at(world)?.is_dir() {
        chunks::read(world).await?
    } else {
        let data = fs::read(world).await.at(world)?;
        parsed(world, move || db::parse(&data)).await?
    };
    portals.sort_by(|a, b| a.name.cmp(&b.name).then(a.x.total_cmp(&b.x)));
    Ok(portals)
}

/// The names that one portal alone has, so it leads nowhere.
pub fn unpaired(portals: &[Portal]) -> Vec<&str> {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for portal in portals {
        *counts.entry(&portal.name).or_default() += 1;
    }
    counts
        .into_iter()
        .filter(|(_, count)| *count == 1)
        .map(|(name, _)| name)
        .collect()
}

/// A world is tens of megabytes, so the parser runs off the async threads.
async fn parsed<T, F>(path: &Path, parser: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> Parsed<T> + Send + 'static,
{
    spawn_blocking(parser)
        .await?
        .map_err(|reason| Error::World {
            path: path.to_path_buf(),
            reason,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn portal(name: &str) -> Portal {
        Portal {
            name: name.to_owned(),
            kind: PortalKind::Wood,
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }

    #[test]
    fn a_name_used_once_is_unpaired() {
        let portals = ["home", "mine", "home", "swamp", "swamp", "swamp"].map(portal);
        assert_eq!(unpaired(&portals), ["mine"]);
    }
}
