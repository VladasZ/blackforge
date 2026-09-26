//! Keeps Cmd+Q on macOS from closing Valheim.
//!
//! A small loader plugin, the source is in `assets/quit`. It goes into every
//! Valheim client started from the app on macOS, there is no setting. Other
//! systems never get it.
//!
//! The plugin belongs to no mod, so the lock never lists it and `sync` leaves
//! it alone. It has its own folder, like the ravens plugin.

use std::path::{Path, PathBuf};

use tokio::fs;

use crate::{
    error::{IoContext, Result},
    launch::Os,
};

const PLUGIN: &[u8] = include_bytes!("../../../assets/quit/BlackforgeQuit.dll");
const PLUGIN_DIR: [&str; 3] = ["BepInEx", "plugins", "blackforge-quit"];
const PLUGIN_FILE: &str = "BlackforgeQuit.dll";

/// Only a Mac has Cmd+Q.
pub fn applies_to(os: Os) -> bool {
    matches!(os, Os::MacIntel | Os::MacArm)
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
    fn only_a_mac_gets_it() {
        assert!(applies_to(Os::MacArm));
        assert!(applies_to(Os::MacIntel));
        assert!(!applies_to(Os::Windows));
        assert!(!applies_to(Os::Linux));
    }
}
