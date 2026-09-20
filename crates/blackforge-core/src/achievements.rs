//! Lets a modded game earn achievements.
//!
//! Valheim 1.0 treats a modded game as a cheated one, and the mod loader marks
//! every game as modded. A small loader plugin hides that mark from the one
//! check that reads it, the source is in `assets/achievements`. The real cheat
//! checks of the game stay as they are.
//!
//! The plugin belongs to no mod, so the lock never lists it and `sync` leaves
//! it alone. It is put in place before every start, and taken out again when
//! the setting is off.

use std::path::{Path, PathBuf};

use tokio::fs;

use crate::{
    error::{IoContext, Result},
    game::{GameDef, Target, VALHEIM},
    util::exists,
};

const PLUGIN: &[u8] = include_bytes!("../../../assets/achievements/BlackforgeAchievements.dll");
const PLUGIN_DIR: [&str; 3] = ["BepInEx", "plugins", "blackforge"];
const PLUGIN_FILE: &str = "BlackforgeAchievements.dll";

/// Only the Valheim client has achievements, and the plugin patches Valheim
/// code by name.
pub fn applies_to(game: &GameDef) -> bool {
    game.label == VALHEIM && game.target == Target::Client
}

pub fn plugin_path(profile_dir: &Path) -> PathBuf {
    plugin_dir(profile_dir).join(PLUGIN_FILE)
}

fn plugin_dir(profile_dir: &Path) -> PathBuf {
    PLUGIN_DIR
        .iter()
        .fold(profile_dir.to_path_buf(), |path, part| path.join(part))
}

pub async fn apply(profile_dir: &Path, keep: bool) -> Result<()> {
    let dir = plugin_dir(profile_dir);
    if keep {
        fs::create_dir_all(&dir).await.at(&dir)?;
        let path = dir.join(PLUGIN_FILE);
        fs::write(&path, PLUGIN).await.at(&path)
    } else if exists(&dir).await {
        fs::remove_dir_all(&dir).await.at(&dir)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;
    use crate::testing::valheim;

    #[tokio::test]
    async fn writes_the_plugin_and_takes_it_out_again() -> Result<()> {
        let temp = tempdir().at(Path::new("tempdir"))?;
        let path = plugin_path(temp.path());

        apply(temp.path(), false).await?;
        assert!(!exists(&path).await);

        apply(temp.path(), true).await?;
        assert_eq!(fs::read(&path).await.at(&path)?, PLUGIN);

        apply(temp.path(), false).await?;
        assert!(!exists(&path).await);
        assert!(exists(&temp.path().join("BepInEx").join("plugins")).await);
        Ok(())
    }

    #[test]
    fn only_the_valheim_client_has_achievements() -> Result<()> {
        assert!(applies_to(&valheim(Target::Client)?));
        assert!(!applies_to(&valheim(Target::Server)?));
        Ok(())
    }
}
