use std::{
    fs::{File, create_dir_all, remove_file},
    io::copy,
    path::Path,
};

use zip::ZipArchive;

use crate::{
    error::{IoContext, Result},
    game::GameDef,
    ident::PackageId,
    install::rules::{Placement, plan_loader, plan_mod},
    util::{remove_empty_parents, tree_path},
};

/// Unpacks one package zip into `tree` and returns the tracked files it wrote.
/// Blocking, call it from `spawn_blocking`.
pub fn unpack(game: &GameDef, id: &PackageId, zip_path: &Path, tree: &Path) -> Result<Vec<String>> {
    let mut archive = ZipArchive::new(File::open(zip_path).at(zip_path)?)?;
    let entries: Vec<String> = archive.file_names().map(str::to_owned).collect();
    let placements = match game.loader_package(id) {
        Some(loader) => plan_loader(game, loader, &entries),
        None => plan_mod(game, id, &entries)?,
    };

    let mut written = Vec::new();
    for Placement {
        entry,
        dest,
        tracked,
    } in placements
    {
        let target = tree_path(tree, &dest);
        if !tracked && target.exists() {
            continue;
        }
        if let Some(parent) = target.parent() {
            create_dir_all(parent).at(parent)?;
        }
        let mut source = archive.by_name(&entry)?;
        let mut file = File::create(&target).at(&target)?;
        copy(&mut source, &mut file).at(&target)?;
        if tracked {
            written.push(dest);
        }
    }
    Ok(written)
}

/// Deletes the tracked files of one package and the folders that became empty.
pub fn remove_files(tree: &Path, files: &[String]) -> Result<()> {
    for file in files {
        let path = tree_path(tree, file);
        if path.is_file() {
            remove_file(&path).at(&path)?;
        }
        if let Some(parent) = path.parent() {
            remove_empty_parents(parent, tree)?;
        }
    }
    Ok(())
}
