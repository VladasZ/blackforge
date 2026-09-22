//! Writes a profile as a plain folder, for a server that is rented or runs in
//! Docker, where blackforge cannot start the game itself.

use std::{
    fs::{copy, create_dir_all},
    path::{Path, PathBuf},
};

use tokio::task::spawn_blocking;

use crate::{
    error::{IoContext, Result},
    forge::Forge,
    game::GameDef,
    install::{SyncReport, ZipCache, rules::untracked_routes, sync_tree, wanted},
    profile::Profile,
    progress::Progress,
    util::walk_files,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeployReport {
    pub sync: SyncReport,
    pub configs_written: Vec<String>,
    /// Configs that already exist at the target and were left as they are.
    pub configs_kept: Vec<String>,
}

impl Forge {
    /// The target becomes a tree of its own. It keeps its own record of what
    /// was written, so the next deploy removes what left the lock and keeps
    /// every other file of the folder, which may be the game folder itself.
    pub async fn deploy(
        &self,
        profile: &Profile,
        target: &Path,
        overwrite_configs: bool,
        progress: &Progress,
    ) -> Result<DeployReport> {
        let manifest = profile.manifest().await?;
        let game = self.game(&manifest).await?;
        let lock = profile.lock().await?;
        // The configs of the profile go first. Unpacking a mod writes its
        // default config only where none exists, so this order lets the
        // edited config of the profile win over the default of the mod.
        let (configs_written, configs_kept) = {
            let game = game.clone();
            let source = profile.dir().to_path_buf();
            let target = target.to_path_buf();
            spawn_blocking(move || copy_configs(&game, &source, &target, overwrite_configs))
                .await??
        };
        let cache = ZipCache::new(self.data());
        let sync = sync_tree(
            self.client(),
            &cache,
            &game,
            &wanted(&manifest, &lock),
            target,
            progress,
        )
        .await?;
        Ok(DeployReport {
            sync,
            configs_written,
            configs_kept,
        })
    }
}

fn copy_configs(
    game: &GameDef,
    source: &Path,
    target: &Path,
    overwrite: bool,
) -> Result<(Vec<String>, Vec<String>)> {
    let (mut written, mut kept) = (Vec::new(), Vec::new());
    for route in untracked_routes(game) {
        let join = |root: &Path| {
            route
                .split('/')
                .fold(root.to_path_buf(), |path, part| path.join(part))
        };
        let from: PathBuf = join(source);
        if !from.is_dir() {
            continue;
        }
        for relative in walk_files(&from)? {
            let name = format!("{route}/{}", relative.to_string_lossy().replace('\\', "/"));
            let dest = join(target).join(&relative);
            if dest.exists() && !overwrite {
                kept.push(name);
                continue;
            }
            if let Some(parent) = dest.parent() {
                create_dir_all(parent).at(parent)?;
            }
            copy(from.join(&relative), &dest).at(&dest)?;
            written.push(name);
        }
    }
    Ok((written, kept))
}
