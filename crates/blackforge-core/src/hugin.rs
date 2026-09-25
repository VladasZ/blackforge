//! Keeps the tutorial ravens Hugin and Munin out of the game.
//!
//! A small loader plugin, the source is in `assets/hugin`. It goes into every
//! Valheim client started from the app, with the join plugin, there is no
//! setting. The tutorial texts still land in the compendium.
//!
//! The plugin belongs to no mod, so the lock never lists it and `sync` leaves
//! it alone. It has its own folder, since the achievements plugin removes its
//! folder when that setting is off.

use std::path::{Path, PathBuf};

use tokio::fs;

use crate::error::{IoContext, Result};

const PLUGIN: &[u8] = include_bytes!("../../../assets/hugin/BlackforgeHugin.dll");
const PLUGIN_DIR: [&str; 3] = ["BepInEx", "plugins", "blackforge-hugin"];
const PLUGIN_FILE: &str = "BlackforgeHugin.dll";

pub fn plugin_path(profile_dir: &Path) -> PathBuf {
    PLUGIN_DIR
        .iter()
        .fold(profile_dir.to_path_buf(), |path, part| path.join(part))
        .join(PLUGIN_FILE)
}

pub async fn apply(profile_dir: &Path) -> Result<()> {
    let path = plugin_path(profile_dir);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).await.at(dir)?;
    }
    fs::write(&path, PLUGIN).await.at(&path)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[tokio::test]
    async fn writes_the_plugin() -> Result<()> {
        let temp = tempdir().at(Path::new("tempdir"))?;
        let path = plugin_path(temp.path());

        apply(temp.path()).await?;
        assert_eq!(fs::read(&path).await.at(&path)?, PLUGIN);

        // A second start writes it again over the old copy.
        apply(temp.path()).await?;
        assert_eq!(fs::read(&path).await.at(&path)?, PLUGIN);
        Ok(())
    }
}
