//! Adds a "Join Durka" button to the main menu of the game.
//!
//! A small loader plugin, the source is in `assets/join`. The button queues a
//! join by the server address, and the game asks for the password itself.
//!
//! The plugin belongs to no mod, so the lock never lists it and `sync` leaves
//! it alone. It has its own folder, since the achievements plugin removes its
//! folder when that setting is off.

use std::path::{Path, PathBuf};

use tokio::fs;

use crate::{
    error::{IoContext, Result},
    game::{GameDef, Target, VALHEIM},
};

const PLUGIN: &[u8] = include_bytes!("../../../assets/join/BlackforgeJoin.dll");
const PLUGIN_DIR: [&str; 3] = ["BepInEx", "plugins", "blackforge-join"];
const PLUGIN_FILE: &str = "BlackforgeJoin.dll";

/// Only the Valheim client has a main menu, and the plugin patches Valheim
/// code.
pub fn applies_to(game: &GameDef) -> bool {
    game.label == VALHEIM && game.target == Target::Client
}

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
    use crate::testing::valheim;

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

    #[test]
    fn only_the_valheim_client_gets_the_button() -> Result<()> {
        assert!(applies_to(&valheim(Target::Client)?));
        assert!(!applies_to(&valheim(Target::Server)?));
        Ok(())
    }
}
