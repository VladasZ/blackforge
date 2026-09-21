use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::{
    error::{Error, IoContext, Result},
    game::{GameDef, Target},
    lock::{LOCK_FILE, Lockfile},
    manifest::{MANIFEST_FILE, Manifest},
    paths::DataDir,
    util::exists,
};

/// One named mod set. The folder holds the manifest, the lock, and the tree
/// that `sync` builds: the mod loader files and the `BepInEx` folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    name: String,
    dir: PathBuf,
}

impl Profile {
    pub(crate) fn staged(&self, dir: PathBuf) -> Self {
        Self {
            name: self.name.clone(),
            dir,
        }
    }
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn manifest_path(&self) -> PathBuf {
        self.dir.join(MANIFEST_FILE)
    }

    pub fn lock_path(&self) -> PathBuf {
        self.dir.join(LOCK_FILE)
    }

    pub async fn manifest(&self) -> Result<Manifest> {
        Manifest::read(&self.manifest_path()).await
    }

    pub async fn lock(&self) -> Result<Lockfile> {
        Lockfile::read(&self.lock_path()).await
    }

    pub async fn save(&self, manifest: &Manifest, lock: &Lockfile) -> Result<()> {
        manifest.write(&self.manifest_path()).await?;
        lock.write(&self.lock_path()).await
    }
}

/// `config.toml`, the global config: the facts that belong to this machine and
/// not to a profile.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    #[serde(default)]
    pub active_profile: Option<String>,
    /// Lets a modded game earn achievements, see `achievements`.
    #[serde(default)]
    pub keep_achievements: bool,
    /// Game folders the user gave by hand, keyed by `game_dir_key`.
    #[serde(default)]
    pub game_dirs: BTreeMap<String, PathBuf>,
}

pub fn game_dir_key(game: &GameDef) -> String {
    format!("{}-{}", game.label, game.target)
}

#[derive(Clone, Debug)]
pub struct ProfileStore {
    data: DataDir,
}

impl ProfileStore {
    pub fn new(data: DataDir) -> Self {
        Self { data }
    }

    pub fn data(&self) -> &DataDir {
        &self.data
    }

    pub async fn state(&self) -> Result<State> {
        let path = self.data.config_file();
        if !exists(&path).await {
            return Ok(State::default());
        }
        let text = fs::read_to_string(&path).await.at(&path)?;
        Ok(toml::from_str(&text)?)
    }

    pub async fn save_state(&self, state: &State) -> Result<()> {
        let path = self.data.config_file();
        fs::create_dir_all(self.data.root())
            .await
            .at(self.data.root())?;
        fs::write(&path, toml::to_string(state)?).await.at(&path)
    }

    pub async fn list(&self) -> Result<Vec<String>> {
        let dir = self.data.profiles_dir();
        if !exists(&dir).await {
            return Ok(Vec::new());
        }
        let mut names = Vec::new();
        let mut entries = fs::read_dir(&dir).await.at(&dir)?;
        while let Some(entry) = entries.next_entry().await.at(&dir)? {
            if exists(&entry.path().join(MANIFEST_FILE)).await {
                names.push(entry.file_name().to_string_lossy().into_owned());
            }
        }
        names.sort();
        Ok(names)
    }

    pub async fn get(&self, name: &str) -> Result<Profile> {
        let profile = self.handle(name)?;
        if !exists(&profile.manifest_path()).await {
            return Err(Error::ProfileNotFound(name.to_owned()));
        }
        Ok(profile)
    }

    pub async fn active(&self) -> Result<Profile> {
        let name = self
            .state()
            .await?
            .active_profile
            .ok_or(Error::NoActiveProfile)?;
        self.get(&name).await
    }

    /// Creates an empty profile. The first profile becomes the active one.
    pub async fn create(&self, name: &str, game: &str, target: Target) -> Result<Profile> {
        let profile = self.handle(name)?;
        if exists(&profile.manifest_path()).await {
            return Err(Error::ProfileExists(name.to_owned()));
        }
        fs::create_dir_all(profile.dir()).await.at(profile.dir())?;
        profile
            .save(&Manifest::new(game, target), &Lockfile::default())
            .await?;

        let mut state = self.state().await?;
        if state.active_profile.is_none() {
            state.active_profile = Some(name.to_owned());
            self.save_state(&state).await?;
        }
        Ok(profile)
    }

    pub async fn switch(&self, name: &str) -> Result<Profile> {
        let profile = self.get(name).await?;
        let mut state = self.state().await?;
        state.active_profile = Some(name.to_owned());
        self.save_state(&state).await?;
        Ok(profile)
    }

    pub async fn delete(&self, name: &str) -> Result<()> {
        let profile = self.get(name).await?;
        fs::remove_dir_all(profile.dir()).await.at(profile.dir())?;
        let mut state = self.state().await?;
        if state.active_profile.as_deref() == Some(name) {
            state.active_profile = None;
            self.save_state(&state).await?;
        }
        Ok(())
    }

    /// The name becomes a folder name, so it must not be able to leave the
    /// profiles folder or clash with a reserved name on any system.
    fn handle(&self, name: &str) -> Result<Profile> {
        let valid = !name.is_empty()
            && name.len() <= 64
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if !valid {
            return Err(Error::InvalidProfileName(name.to_owned()));
        }
        Ok(Profile {
            name: name.to_owned(),
            dir: self.data.profiles_dir().join(name),
        })
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;
    use crate::game::VALHEIM;

    #[tokio::test]
    async fn create_switch_delete() -> Result<()> {
        let temp = tempdir().at(Path::new("tempdir"))?;
        let store = ProfileStore::new(DataDir::at(temp.path().to_path_buf()));
        assert!(matches!(store.active().await, Err(Error::NoActiveProfile)));

        store.create("main", VALHEIM, Target::Client).await?;
        store.create("server", VALHEIM, Target::Server).await?;
        assert_eq!(store.list().await?, ["main", "server"]);
        assert_eq!(store.active().await?.name(), "main");
        assert!(matches!(
            store.create("main", VALHEIM, Target::Client).await,
            Err(Error::ProfileExists(_))
        ));

        let server = store.switch("server").await?;
        assert_eq!(server.manifest().await?.target, Target::Server);
        assert_eq!(store.active().await?.name(), "server");

        store.delete("server").await?;
        assert_eq!(store.list().await?, ["main"]);
        assert!(matches!(store.active().await, Err(Error::NoActiveProfile)));
        Ok(())
    }

    #[tokio::test]
    async fn rejects_names_that_leave_the_folder() -> Result<()> {
        let temp = tempdir().at(Path::new("tempdir"))?;
        let store = ProfileStore::new(DataDir::at(temp.path().to_path_buf()));
        for name in ["", "..", "a/b", "a b", "a.b"] {
            assert!(matches!(
                store.create(name, VALHEIM, Target::Client).await,
                Err(Error::InvalidProfileName(_))
            ));
        }
        Ok(())
    }
}
