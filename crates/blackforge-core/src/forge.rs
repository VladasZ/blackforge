//! The operations a frontend calls. Each one reads the profile, does the
//! work, and saves the manifest and the lock before it returns.

use std::{path::PathBuf, time::Duration};

use semver::Version;

use crate::{
    broken::{Broken, load_list},
    error::{Error, Result},
    game::{GameDef, GameInstall, Schema, Target, VALHEIM, locate},
    http::Client,
    ident::{PackageId, VersionedId},
    install::{SyncReport, ZipCache, purge_gone, sync_tree, wanted},
    lock::{LockedPackage, Lockfile},
    manifest::{Manifest, ModSpec, Pin, VersionReq},
    paths::DataDir,
    profile::{LaunchSettings, Profile, ProfileStore, game_dir_key},
    progress::Progress,
    resolve::{MissingDependency, Unlock, resolve},
    thunderstore::{FRESH_ENOUGH, PackageIndex},
};

/// The profile that is created by itself on a fresh machine.
pub const DEFAULT_PROFILE: &str = "default";

/// How a lock differs from the one before it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LockChange {
    pub added: Vec<VersionedId>,
    pub removed: Vec<VersionedId>,
    pub changed: Vec<(PackageId, Version, Version)>,
    pub missing: Vec<MissingDependency>,
}

impl LockChange {
    fn between(old: &Lockfile, new: &Lockfile, missing: Vec<MissingDependency>) -> Self {
        let mut change = Self {
            missing,
            ..Self::default()
        };
        for package in &new.packages {
            match old.get(&package.id) {
                None => change.added.push(package.versioned()),
                Some(before) if before.version != package.version => {
                    change.changed.push((
                        package.id.clone(),
                        before.version.clone(),
                        package.version.clone(),
                    ));
                }
                Some(_) => {}
            }
        }
        change.removed = old
            .packages
            .iter()
            .filter(|package| new.get(&package.id).is_none())
            .map(LockedPackage::versioned)
            .collect();
        change
    }

    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outdated {
    pub id: PackageId,
    pub locked: Version,
    pub latest: Version,
    /// A pin in the manifest holds this mod back, `update` will not move it.
    pub pinned: bool,
}

#[derive(Clone, Debug)]
pub struct Forge {
    client: Client,
    store: ProfileStore,
}

impl Forge {
    pub fn open() -> Result<Self> {
        Self::at(DataDir::locate()?)
    }

    pub fn at(data: DataDir) -> Result<Self> {
        Ok(Self {
            client: Client::new()?,
            store: ProfileStore::new(data),
        })
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn store(&self) -> &ProfileStore {
        &self.store
    }

    pub fn data(&self) -> &DataDir {
        self.store.data()
    }

    pub async fn game(&self, manifest: &Manifest) -> Result<GameDef> {
        Schema::load(&self.client, self.data())
            .await?
            .game(&manifest.game, manifest.target)
    }

    pub async fn index(
        &self,
        game: &GameDef,
        max_age: Duration,
        progress: &Progress,
    ) -> Result<PackageIndex> {
        PackageIndex::load(&self.client, self.data(), game, max_age, progress).await
    }

    /// Creates a profile that already holds the mod loader, so it can run.
    pub async fn create_profile(
        &self,
        name: &str,
        game: &str,
        target: Target,
        progress: &Progress,
    ) -> Result<Profile> {
        let label = game;
        let game = Schema::load(&self.client, self.data())
            .await?
            .game(label, target)?;
        let index = self.index(&game, FRESH_ENOUGH, progress).await?;
        let loader = pick_loader(&game, &index)?;

        let profile = self.store.create(name, label, target).await?;
        let mut manifest = profile.manifest().await?;
        manifest
            .mods
            .insert(loader, ModSpec::new(VersionReq::Latest));
        let resolution = resolve(&manifest, &index, &Lockfile::default(), &Unlock::Nothing)?;
        profile.save(&manifest, &resolution.lock).await?;
        Ok(profile)
    }

    /// The active profile. On a fresh machine there is none, so the profile
    /// `default` is created and synced on the spot and no setup step is
    /// needed. The flag says whether that happened.
    ///
    /// When profiles exist but none is active, the user deleted the active
    /// one, and picking another for them would be a guess.
    pub async fn active_or_default(&self, progress: &Progress) -> Result<(Profile, bool)> {
        match self.store.active().await {
            Err(Error::NoActiveProfile) if self.store.list().await?.is_empty() => {
                let profile = self
                    .create_profile(DEFAULT_PROFILE, VALHEIM, Target::Client, progress)
                    .await?;
                self.sync(&profile, progress).await?;
                Ok((profile, true))
            }
            other => Ok((other?, false)),
        }
    }

    /// Adds a mod that follows the newest version. There is no pin by hand,
    /// a pin comes only from a server, see `add_for_server`.
    pub async fn add(
        &self,
        profile: &Profile,
        query: &str,
        progress: &Progress,
    ) -> Result<(PackageId, LockChange)> {
        self.put(profile, query, VersionReq::Latest, progress).await
    }

    /// Pins a mod to the version a server needs, with the server's name.
    pub async fn add_for_server(
        &self,
        profile: &Profile,
        query: &str,
        version: Version,
        server: &str,
        progress: &Progress,
    ) -> Result<(PackageId, LockChange)> {
        let pin = VersionReq::Pinned(Pin {
            version,
            server: server.to_owned(),
        });
        self.put(profile, query, pin, progress).await
    }

    async fn put(
        &self,
        profile: &Profile,
        query: &str,
        version: VersionReq,
        progress: &Progress,
    ) -> Result<(PackageId, LockChange)> {
        let mut manifest = profile.manifest().await?;
        let game = self.game(&manifest).await?;
        let index = self.index(&game, FRESH_ENOUGH, progress).await?;
        let id = index.find(query)?.id.clone();

        let enabled = manifest.mods.get(&id).is_none_or(|spec| spec.enabled);
        manifest
            .mods
            .insert(id.clone(), ModSpec { version, enabled });
        // The added mod itself may move, so a new pin or a dropped pin takes
        // effect. Everything else keeps its locked version.
        let change = self
            .relock(profile, &manifest, &index, &Unlock::Only(vec![id.clone()]))
            .await?;
        Ok((id, change))
    }

    pub async fn remove(&self, profile: &Profile, query: &str) -> Result<(PackageId, LockChange)> {
        let mut manifest = profile.manifest().await?;
        let id = manifest_id(&manifest, query)?;
        manifest.mods.remove(&id);
        let game = self.game(&manifest).await?;
        let index = PackageIndex::cached(self.data(), &game).await?;
        let change = self
            .relock(profile, &manifest, &index, &Unlock::Nothing)
            .await?;
        Ok((id, change))
    }

    /// Moves the lock to the newest versions. No names means every package.
    pub async fn update(
        &self,
        profile: &Profile,
        queries: &[String],
        progress: &Progress,
    ) -> Result<LockChange> {
        let manifest = profile.manifest().await?;
        let game = self.game(&manifest).await?;
        let index = PackageIndex::refresh(&self.client, self.data(), &game, progress).await?;
        let unlock = if queries.is_empty() {
            Unlock::Everything
        } else {
            let lock = profile.lock().await?;
            let ids: Result<Vec<PackageId>> = queries
                .iter()
                .map(|query| locked_id(&lock, query))
                .collect();
            Unlock::Only(ids?)
        };
        self.relock(profile, &manifest, &index, &unlock).await
    }

    pub async fn outdated(&self, profile: &Profile, progress: &Progress) -> Result<Vec<Outdated>> {
        let manifest = profile.manifest().await?;
        let game = self.game(&manifest).await?;
        let index = PackageIndex::refresh(&self.client, self.data(), &game, progress).await?;
        let mut outdated = Vec::new();
        for package in profile.lock().await?.packages {
            let Some(latest) = index.get(&package.id).and_then(|found| found.latest()) else {
                continue;
            };
            if latest.version > package.version {
                let pinned = manifest
                    .mods
                    .get(&package.id)
                    .is_some_and(|spec| spec.version.pinned_version().is_some());
                outdated.push(Outdated {
                    id: package.id,
                    locked: package.version,
                    latest: latest.version.clone(),
                    pinned,
                });
            }
        }
        Ok(outdated)
    }

    pub async fn set_enabled(
        &self,
        profile: &Profile,
        query: &str,
        enabled: bool,
    ) -> Result<PackageId> {
        let mut manifest = profile.manifest().await?;
        let id = manifest_id(&manifest, query)?;
        manifest.require_mut(&id)?.enabled = enabled;
        manifest.write(&profile.manifest_path()).await?;
        Ok(id)
    }

    /// Builds the profile tree from the lock. A package that left the lock
    /// loses its config and whatever else it wrote into the profile too.
    pub async fn sync(&self, profile: &Profile, progress: &Progress) -> Result<SyncReport> {
        let manifest = profile.manifest().await?;
        let game = self.game(&manifest).await?;
        let lock = profile.lock().await?;
        purge_gone(&game, &lock, profile.dir()).await?;
        let cache = ZipCache::new(self.data());
        sync_tree(
            &self.client,
            &cache,
            &game,
            &wanted(&manifest, &lock),
            profile.dir(),
            progress,
        )
        .await
    }

    /// The game folder: a path given now wins and is remembered, then a path
    /// remembered earlier, then the Steam lookup.
    pub async fn locate_game(
        &self,
        game: &GameDef,
        game_dir: Option<PathBuf>,
    ) -> Result<GameInstall> {
        let key = game_dir_key(game);
        let mut state = self.store.state().await?;
        if let Some(dir) = game_dir {
            let install = locate(game, Some(dir.clone())).await?;
            state.game_dirs.insert(key, dir);
            self.store.save_state(&state).await?;
            return Ok(install);
        }
        locate(game, state.game_dirs.get(&key).cloned()).await
    }

    /// The mods the server lists as broken on the game version of this
    /// machine. A game Steam does not know has no version, so nothing is
    /// flagged, the doctor reports the missing game on its own.
    pub async fn broken(&self, game: &GameDef) -> Result<Broken> {
        let list = load_list(&self.client, self.data()).await?;
        let updated = self
            .locate_game(game, None)
            .await
            .ok()
            .and_then(|install| install.updated);
        Broken::resolve(&list, &game.label, updated)
    }

    /// How `profile` starts the game. A profile from before the launch file
    /// takes the achievements flag the machine had then.
    pub async fn launch_settings(&self, profile: &Profile) -> Result<LaunchSettings> {
        if let Some(settings) = profile.launch_settings().await? {
            return Ok(settings);
        }
        Ok(LaunchSettings {
            keep_achievements: self.store.state().await?.keep_achievements,
            ..LaunchSettings::default()
        })
    }

    /// Changes the settings of `profile` with `edit` and saves them.
    pub async fn edit_launch_settings(
        &self,
        profile: &Profile,
        edit: impl FnOnce(&mut LaunchSettings),
    ) -> Result<()> {
        let mut settings = self.launch_settings(profile).await?;
        edit(&mut settings);
        profile.save_launch_settings(&settings).await
    }

    async fn relock(
        &self,
        profile: &Profile,
        manifest: &Manifest,
        index: &PackageIndex,
        unlock: &Unlock,
    ) -> Result<LockChange> {
        let old = profile.lock().await?;
        let resolution = resolve(manifest, index, &old, unlock)?;
        profile.save(manifest, &resolution.lock).await?;
        Ok(LockChange::between(
            &old,
            &resolution.lock,
            resolution.missing,
        ))
    }
}

/// A game can have several loader packs. The pinned one is the pack the
/// community treats as the default, then the best rated one.
fn pick_loader(game: &GameDef, index: &PackageIndex) -> Result<PackageId> {
    game.loader_packages
        .iter()
        .filter_map(|loader| index.get(&loader.id))
        .filter(|package| !package.deprecated)
        .max_by_key(|package| (package.pinned, package.rating))
        .map(|package| package.id.clone())
        .ok_or_else(|| {
            Error::Invalid(format!(
                "Thunderstore has no mod loader package for {}",
                game.display_name
            ))
        })
}

/// Accepts `Owner-Name`, or a bare `Name` when only one entry has it.
fn match_id<'a>(ids: impl Iterator<Item = &'a PackageId>, query: &str) -> Result<PackageId> {
    let wanted = query.to_lowercase();
    let mut matches = ids
        .filter(|id| id.to_string().to_lowercase() == wanted || id.name().to_lowercase() == wanted);
    let first = matches
        .next()
        .ok_or_else(|| Error::NotInManifest(query.to_owned()))?;
    if matches.next().is_some() {
        return Err(Error::Invalid(format!(
            "several mods are named '{query}', use the Owner-Name form"
        )));
    }
    Ok(first.clone())
}

fn manifest_id(manifest: &Manifest, query: &str) -> Result<PackageId> {
    match_id(manifest.mods.keys(), query)
}

fn locked_id(lock: &Lockfile, query: &str) -> Result<PackageId> {
    match_id(lock.packages.iter().map(|package| &package.id), query)
}
