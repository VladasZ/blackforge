mod cache;
pub(crate) mod rules;
mod state;
mod unpack;

use std::path::{Path, PathBuf};

pub use cache::ZipCache;
use futures_util::{StreamExt, TryStreamExt, stream};
pub use state::{Installed, InstalledPackage, STATE_FILE};
use tokio::{fs, task::spawn_blocking};

use crate::{
    error::{Error, IoContext, Result},
    game::GameDef,
    http::Client,
    ident::{PackageId, VersionedId},
    lock::{LockedPackage, Lockfile},
    manifest::Manifest,
    progress::{Event, Progress},
};

const PARALLEL_DOWNLOADS: usize = 4;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SyncReport {
    pub installed: Vec<VersionedId>,
    pub removed: Vec<PackageId>,
    pub unchanged: usize,
}

/// The locked packages that belong in a tree. A mod that is disabled in the
/// manifest stays in the lock but its files are left out.
pub fn wanted(manifest: &Manifest, lock: &Lockfile) -> Vec<VersionedId> {
    lock.packages
        .iter()
        .filter(|package| manifest.is_enabled(&package.id))
        .map(LockedPackage::versioned)
        .collect()
}

/// Makes `tree` hold exactly `wanted`. It removes what left the set, replaces
/// what changed version, adds what is new, and never touches user configs.
pub async fn sync_tree(
    client: &Client,
    cache: &ZipCache,
    game: &GameDef,
    wanted: &[VersionedId],
    tree: &Path,
    progress: &Progress,
) -> Result<SyncReport> {
    fs::create_dir_all(tree).await.at(tree)?;
    let mut installed = Installed::read(tree).await?;
    let mut report = SyncReport::default();

    let stale: Vec<PackageId> = installed
        .packages
        .iter()
        .filter(|(id, package)| {
            !wanted
                .iter()
                .any(|want| &want.id == *id && want.version == package.version)
        })
        .map(|(id, _)| id.clone())
        .collect();
    for id in stale {
        let Some(package) = installed.packages.remove(&id) else {
            continue;
        };
        let root = tree.to_path_buf();
        spawn_blocking(move || unpack::remove_files(&root, &package.files)).await??;
        installed.write(tree).await?;
        // A version change shows up as an install below, not as a removal.
        if !wanted.iter().any(|want| want.id == id) {
            progress.send(Event::Removed {
                package: id.clone(),
            })?;
            report.removed.push(id);
        }
    }

    let missing: Vec<VersionedId> = wanted
        .iter()
        .filter(|want| !installed.packages.contains_key(&want.id))
        .cloned()
        .collect();
    report.unchanged = wanted.len() - missing.len();

    let mut zips: Vec<(VersionedId, PathBuf)> = stream::iter(missing)
        .map(|package| async move {
            let zip = cache.fetch(client, &package, progress).await?;
            Ok::<_, Error>((package, zip))
        })
        .buffer_unordered(PARALLEL_DOWNLOADS)
        .try_collect()
        .await?;

    // The loader goes first. A mod may ship a file that replaces one of the
    // loader, and the other order would undo that.
    zips.sort_by_key(|(package, _)| {
        (
            game.loader_package(&package.id).is_none(),
            package.id.clone(),
        )
    });

    for (package, zip) in zips {
        let files = {
            let game = game.clone();
            let id = package.id.clone();
            let root = tree.to_path_buf();
            spawn_blocking(move || unpack::unpack(&game, &id, &zip, &root)).await??
        };
        installed.packages.insert(
            package.id.clone(),
            InstalledPackage {
                version: package.version.clone(),
                files,
            },
        );
        // Saved after every package so a run that stops halfway leaves a
        // state that matches the files on disk.
        installed.write(tree).await?;
        progress.send(Event::Installed {
            package: package.clone(),
        })?;
        report.installed.push(package);
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use std::fs::{create_dir_all, read_to_string, write};

    use tempfile::tempdir;

    use super::*;
    use crate::{
        game::Target,
        paths::DataDir,
        testing::{valheim, write_zip},
    };

    const LOADER: &str = "denikson-BepInExPack_Valheim-5.4.2350";

    fn cached(cache: &ZipCache, package: &str, entries: &[&str]) -> Result<VersionedId> {
        let package: VersionedId = package.parse()?;
        let path = cache.path(&package);
        if let Some(parent) = path.parent() {
            create_dir_all(parent).at(parent)?;
        }
        write_zip(&path, entries)?;
        Ok(package)
    }

    #[tokio::test]
    async fn sync_adds_replaces_and_removes() -> Result<()> {
        let temp = tempdir().at(Path::new("tempdir"))?;
        let data = DataDir::at(temp.path().join("data"));
        let tree = temp.path().join("tree");
        let cache = ZipCache::new(&data);
        let game = valheim(Target::Client)?;
        let client = Client::new()?;
        let silent = Progress::silent();

        let loader = cached(
            &cache,
            LOADER,
            &[
                "manifest.json",
                "BepInExPack_Valheim/winhttp.dll",
                "BepInExPack_Valheim/BepInEx/core/BepInEx.Preloader.dll",
                "BepInExPack_Valheim/BepInEx/config/BepInEx.cfg",
            ],
        )?;
        let old = cached(
            &cache,
            "Owner-Mod-1.0.0",
            &["Mod.dll", "Old.dll", "config/owner.mod.cfg"],
        )?;
        let new = cached(
            &cache,
            "Owner-Mod-1.1.0",
            &["Mod.dll", "config/owner.mod.cfg"],
        )?;

        let report = sync_tree(
            &client,
            &cache,
            &game,
            &[loader.clone(), old],
            &tree,
            &silent,
        )
        .await?;
        assert_eq!(report.installed.len(), 2);
        assert!(tree.join("winhttp.dll").is_file());
        assert!(tree.join("BepInEx/core/BepInEx.Preloader.dll").is_file());
        assert!(tree.join("BepInEx/plugins/Owner-Mod/Old.dll").is_file());

        let config = tree.join("BepInEx/config/owner.mod.cfg");
        write(&config, "edited by the user").at(&config)?;

        let report = sync_tree(
            &client,
            &cache,
            &game,
            &[loader.clone(), new.clone()],
            &tree,
            &silent,
        )
        .await?;
        assert_eq!(report.installed, [new]);
        assert_eq!(report.unchanged, 1);
        assert!(report.removed.is_empty());
        assert!(!tree.join("BepInEx/plugins/Owner-Mod/Old.dll").exists());
        assert!(tree.join("BepInEx/plugins/Owner-Mod/Mod.dll").is_file());
        assert_eq!(read_to_string(&config).at(&config)?, "edited by the user");

        let report = sync_tree(&client, &cache, &game, &[loader], &tree, &silent).await?;
        assert_eq!(report.removed, ["Owner-Mod".parse()?]);
        assert!(!tree.join("BepInEx/plugins/Owner-Mod").exists());
        assert_eq!(read_to_string(&config).at(&config)?, "edited by the user");
        assert!(tree.join("winhttp.dll").is_file());
        Ok(())
    }
}
