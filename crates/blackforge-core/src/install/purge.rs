//! Deletes what a package leaves behind once it left the lock. `sync` removes
//! only the files it wrote, and a mod writes its config and other data while
//! the game runs, so those stayed and the Configs page kept listing the mod.

use std::{
    collections::BTreeSet,
    fs::{remove_dir_all, remove_file},
    path::Path,
};

use tokio::task::spawn_blocking;

use crate::{
    config::package_of,
    error::{IoContext, Result},
    game::GameDef,
    ident::PackageId,
    install::{Installed, rules::untracked_routes},
    lock::Lockfile,
    util::{remove_empty_parents, tree_path, walk_files},
};

/// Deletes the folders named after every installed package that is not in
/// the lock, and the files in the config folders that carry its name. A
/// disabled mod stays in the lock, so its files are kept. Returns the
/// packages it cleaned up.
pub async fn purge_gone(game: &GameDef, lock: &Lockfile, tree: &Path) -> Result<Vec<PackageId>> {
    let installed = Installed::read(tree).await?;
    let gone: Vec<PackageId> = installed
        .packages
        .keys()
        .filter(|id| lock.get(id).is_none())
        .cloned()
        .collect();
    if gone.is_empty() {
        return Ok(gone);
    }
    let routes = untracked_routes(game);
    let root = tree.to_path_buf();
    let ids = gone.clone();
    spawn_blocking(move || -> Result<()> {
        for id in &ids {
            remove_package_dirs(&root, id, &installed)?;
            remove_configs(&root, &routes, id, &installed)?;
        }
        Ok(())
    })
    .await??;
    Ok(gone)
}

/// A mod is unpacked into a folder with its own name, `BepInEx/plugins/Owner-Mod`
/// for example. Whatever the mod wrote into that folder goes with it.
fn remove_package_dirs(tree: &Path, id: &PackageId, installed: &Installed) -> Result<()> {
    let Some(package) = installed.packages.get(id) else {
        return Ok(());
    };
    let name = id.to_string();
    let mut dirs = BTreeSet::new();
    for file in &package.files {
        let parts: Vec<&str> = file.split('/').collect();
        if let Some(depth) = parts.iter().position(|part| *part == name)
            && depth + 1 < parts.len()
        {
            dirs.insert(parts[..=depth].join("/"));
        }
    }
    for dir in dirs {
        let path = tree_path(tree, &dir);
        if path.is_dir() {
            remove_dir_all(&path).at(&path)?;
        }
        if let Some(parent) = path.parent() {
            remove_empty_parents(parent, tree)?;
        }
    }
    Ok(())
}

/// Every installed package is a candidate, so a file goes to the same mod
/// the Configs page showed it under.
fn remove_configs(
    tree: &Path,
    routes: &[String],
    id: &PackageId,
    installed: &Installed,
) -> Result<()> {
    for route in routes {
        let dir = tree_path(tree, route);
        if !dir.is_dir() {
            continue;
        }
        for relative in walk_files(&dir)? {
            let name = relative.to_string_lossy().replace('\\', "/");
            if package_of(&name, installed.packages.keys()) != Some(id) {
                continue;
            }
            let path = dir.join(&relative);
            remove_file(&path).at(&path)?;
            if let Some(parent) = path.parent() {
                remove_empty_parents(parent, &dir)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs::{create_dir_all, write};

    use semver::Version;
    use tempfile::tempdir;

    use super::*;
    use crate::{game::Target, install::InstalledPackage, lock::LockedPackage, testing::valheim};

    fn touch(tree: &Path, file: &str) -> Result<()> {
        let path = tree_path(tree, file);
        if let Some(parent) = path.parent() {
            create_dir_all(parent).at(parent)?;
        }
        write(&path, file).at(&path)
    }

    fn locked(id: &str) -> Result<LockedPackage> {
        Ok(LockedPackage {
            id: id.parse()?,
            version: Version::new(1, 0, 0),
            dependencies: Vec::new(),
        })
    }

    fn installed(tree: &Path, packages: &[(&str, &[&str])]) -> Result<Installed> {
        let mut installed = Installed::default();
        for (id, files) in packages {
            for file in *files {
                touch(tree, file)?;
            }
            installed.packages.insert(
                id.parse()?,
                InstalledPackage {
                    version: Version::new(1, 0, 0),
                    files: files.iter().map(|file| (*file).to_owned()).collect(),
                },
            );
        }
        Ok(installed)
    }

    #[tokio::test]
    async fn a_removed_mod_takes_its_folder_and_configs_along() -> Result<()> {
        let temp = tempdir().at(Path::new("tempdir"))?;
        let tree = temp.path();
        let game = valheim(Target::Client)?;
        let state = installed(
            tree,
            &[
                ("denikson-BepInExPack_Valheim", &["winhttp.dll"]),
                ("Owner-Portal", &["BepInEx/plugins/Owner-Portal/Portal.dll"]),
                ("Owner-Other", &["BepInEx/plugins/Owner-Other/Other.dll"]),
            ],
        )?;
        state.write(tree).await?;
        for file in [
            "BepInEx/plugins/Owner-Portal/written_by_the_mod.json",
            "BepInEx/config/BepInEx.cfg",
            "BepInEx/config/owner.portal.cfg",
            "BepInEx/config/Owner.Portal_player_1.dat",
            "BepInEx/config/Owner/Portal/icon.png",
            "BepInEx/config/owner.other.cfg",
        ] {
            touch(tree, file)?;
        }

        let lock = Lockfile {
            version: 1,
            packages: vec![
                locked("denikson-BepInExPack_Valheim")?,
                locked("Owner-Other")?,
            ],
        };
        let gone = purge_gone(&game, &lock, tree).await?;

        assert_eq!(gone, ["Owner-Portal".parse()?]);
        assert!(!tree.join("BepInEx/plugins/Owner-Portal").exists());
        assert!(!tree.join("BepInEx/config/owner.portal.cfg").exists());
        assert!(
            !tree
                .join("BepInEx/config/Owner.Portal_player_1.dat")
                .exists()
        );
        assert!(!tree.join("BepInEx/config/Owner").exists());
        assert!(tree.join("BepInEx/plugins/Owner-Other/Other.dll").is_file());
        assert!(tree.join("BepInEx/config/owner.other.cfg").is_file());
        assert!(tree.join("BepInEx/config/BepInEx.cfg").is_file());
        assert!(tree.join("winhttp.dll").is_file());
        Ok(())
    }

    #[tokio::test]
    async fn a_disabled_mod_keeps_everything() -> Result<()> {
        let temp = tempdir().at(Path::new("tempdir"))?;
        let tree = temp.path();
        let game = valheim(Target::Client)?;
        let state = installed(
            tree,
            &[("Owner-Portal", &["BepInEx/plugins/Owner-Portal/Portal.dll"])],
        )?;
        state.write(tree).await?;
        touch(tree, "BepInEx/config/owner.portal.cfg")?;

        // The lock keeps a disabled mod, only the wanted set drops it.
        let lock = Lockfile {
            version: 1,
            packages: vec![locked("Owner-Portal")?],
        };
        let gone = purge_gone(&game, &lock, tree).await?;

        assert!(gone.is_empty());
        assert!(
            tree.join("BepInEx/plugins/Owner-Portal/Portal.dll")
                .is_file()
        );
        assert!(tree.join("BepInEx/config/owner.portal.cfg").is_file());
        Ok(())
    }
}
