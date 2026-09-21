use std::{collections::BTreeSet, path::Path};

use blackforge_api::setup::Setup;
use tokio::fs;

use super::{capture, portable, snapshot::manifests, validate};
use crate::{
    config::{self, ConfigFile},
    error::{Error, IoContext, Result},
    forge::Forge,
    profile::Profile,
    progress::Progress,
    thunderstore::FRESH_ENOUGH,
};

/// Complete a directory swap interrupted between its two renames.
pub async fn recover(dir: &Path) -> Result<()> {
    let backup = dir.with_extension("cloud-backup");
    if fs::try_exists(&backup).await.at(&backup)? {
        if fs::try_exists(dir).await.at(dir)? {
            fs::remove_dir_all(&backup).await.at(&backup)?;
        } else {
            fs::rename(&backup, dir).await.at(dir)?;
        }
    }
    Ok(())
}

pub async fn restore(
    forge: &Forge,
    profile: &Profile,
    setup: &Setup,
    progress: &Progress,
) -> Result<()> {
    validate(setup)?;
    let (manifest, lock) = manifests(setup)?;
    let game = forge.game(&manifest).await?;
    let index = forge.index(&game, FRESH_ENOUGH, progress).await?;
    for package in &lock.packages {
        let available = index
            .get(&package.id)
            .and_then(|p| p.version(&package.version))
            .ok_or_else(|| Error::VersionNotFound {
                id: package.id.to_string(),
                version: package.version.to_string(),
            })?;
        for dependency in &available.dependencies {
            if lock
                .get(&dependency.id)
                .is_none_or(|p| p.version < dependency.version)
            {
                return Err(Error::Invalid(format!(
                    "{} requires {}; review the mod choices",
                    package.id, dependency
                )));
            }
        }
    }
    stage(forge, profile, setup, progress).await
}

pub(super) async fn stage(
    forge: &Forge,
    profile: &Profile,
    setup: &Setup,
    progress: &Progress,
) -> Result<()> {
    validate(setup)?;
    let (manifest, lock) = manifests(setup)?;
    recover(profile.dir()).await?;
    let stage_dir = profile.dir().with_extension("cloud-stage");
    if fs::try_exists(&stage_dir).await.at(&stage_dir)? {
        fs::remove_dir_all(&stage_dir).await.at(&stage_dir)?;
    }
    let before = capture(profile).await?;
    copy_tree(profile.dir(), &stage_dir).await?;
    let staged = profile.staged(stage_dir.clone());
    staged.save(&manifest, &lock).await?;
    forge.sync(&staged, progress).await?;
    write_settings(&staged, &before, setup).await?;
    if capture(profile).await? != before {
        return Err(Error::Invalid(
            "local setup changed during installation; review again".to_owned(),
        ));
    }
    // Nothing above this line changes the live profile. A failed download
    // leaves its complete working installation intact.
    let backup = profile.dir().with_extension("cloud-backup");
    fs::rename(profile.dir(), &backup).await.at(&backup)?;
    if let Err(cause) = fs::rename(&stage_dir, profile.dir()).await {
        fs::rename(&backup, profile.dir()).await.at(profile.dir())?;
        return Err(Error::Io {
            path: stage_dir,
            cause,
        });
    }
    // Keep the backup until the next apply or app start. A cleanup error should
    // not turn a successfully installed setup into a failed apply.
    Ok(())
}

async fn copy_tree(source: &Path, target: &Path) -> Result<()> {
    let mut pending = vec![(source.to_path_buf(), target.to_path_buf())];
    while let Some((source, target)) = pending.pop() {
        fs::create_dir_all(&target).await.at(&target)?;
        let mut entries = fs::read_dir(&source).await.at(&source)?;
        while let Some(entry) = entries.next_entry().await.at(&source)? {
            let from = entry.path();
            let to = target.join(entry.file_name());
            let kind = entry.file_type().await.at(&from)?;
            if kind.is_symlink() {
                return Err(Error::Invalid(format!(
                    "cannot sync a profile containing a link: {}",
                    from.display()
                )));
            }
            if kind.is_dir() {
                pending.push((from, to));
            } else {
                fs::copy(&from, &to).await.at(&to)?;
            }
        }
    }
    Ok(())
}

pub(super) async fn write_settings(profile: &Profile, before: &Setup, setup: &Setup) -> Result<()> {
    let files: BTreeSet<_> = before.configs.keys().chain(setup.configs.keys()).collect();
    for file in files {
        let path = profile.dir().join("BepInEx/config").join(file);
        let mut config = if fs::try_exists(&path).await.at(&path)? {
            config::read(&path).await?
        } else {
            ConfigFile::parse("")
        };
        if let Some(sections) = before.configs.get(file) {
            for (section, settings) in sections {
                for key in settings.keys() {
                    if setup
                        .configs
                        .get(file)
                        .and_then(|s| s.get(section))
                        .and_then(|s| s.get(key))
                        .is_none()
                        && config
                            .get(section, key)
                            .is_ok_and(|s| portable(&s.key, &s.value))
                    {
                        config.remove(section, key);
                    }
                }
            }
        }
        if let Some(sections) = setup.configs.get(file) {
            for (section, settings) in sections {
                for (key, value) in settings {
                    // A path or secret on this machine always stays local,
                    // even if another machine uses an ordinary value here.
                    if config
                        .get(section, key)
                        .is_ok_and(|s| !portable(&s.key, &s.value))
                    {
                        continue;
                    }
                    config.upsert(section, key, value)?;
                }
            }
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.at(parent)?;
        }
        fs::write(&path, config.text()).await.at(&path)?;
    }
    Ok(())
}
