use super::Key;
use blackforge_api::setup::{Mod, Setup};

use crate::{
    config,
    error::{Error, Result},
    game::{Target, VALHEIM},
    ident::{PackageId, parse_version},
    lock::{LockedPackage, Lockfile},
    manifest::{Manifest, ModSpec, VersionReq},
    profile::Profile,
    social::secret::looks_secret,
};

pub fn portable(key: &str, value: &str) -> bool {
    let value = value.trim().trim_matches(['"', '\'']);
    !looks_secret(key, value)
        && !value.starts_with(['/', '\\', '~'])
        && value.as_bytes().get(1).is_none_or(|byte| *byte != b':')
        && !value.contains("%USERPROFILE%")
        && !value.contains("$HOME")
        && !value.to_ascii_lowercase().starts_with("file:")
}

pub async fn capture(profile: &Profile) -> Result<Setup> {
    Ok(snapshot(profile).await?.setup)
}

pub struct Snapshot {
    pub setup: Setup,
    pub local_only: Vec<Key>,
}

pub async fn snapshot(profile: &Profile) -> Result<Snapshot> {
    let manifest = profile.manifest().await?;
    let lock = profile.lock().await?;
    let mut setup = Setup::default();
    let mut local_only = Vec::new();
    for package in lock.packages {
        let spec = manifest.mods.get(&package.id);
        setup.mods.insert(
            package.id.to_string(),
            Mod {
                version: package.version.to_string(),
                requested: spec.map(|spec| spec.version.to_string()),
                enabled: manifest.is_enabled(&package.id),
                dependencies: package
                    .dependencies
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
            },
        );
    }
    for file in config::list(profile).await? {
        let path = config::find(profile, &file).await?;
        for setting in config::read(&path).await?.settings() {
            if portable(&setting.key, &setting.value) {
                setup
                    .configs
                    .entry(file.clone())
                    .or_default()
                    .entry(setting.section)
                    .or_default()
                    .insert(setting.key, setting.value);
            } else {
                local_only.push(Key::Setting {
                    file: file.clone(),
                    section: setting.section,
                    key: setting.key,
                });
            }
        }
    }
    validate(&setup)?;
    Ok(Snapshot { setup, local_only })
}

pub fn validate(setup: &Setup) -> Result<()> {
    manifests(setup)?;
    for (file, sections) in &setup.configs {
        if !safe_file(file) {
            return Err(Error::Invalid(format!("unsafe config path: {file}")));
        }
        for (section, settings) in sections {
            if section.contains(['\r', '\n', '[', ']']) {
                return Err(Error::Invalid("invalid config section".to_owned()));
            }
            for (key, value) in settings {
                if key.is_empty()
                    || key.contains(['\r', '\n', '=', '[', ']'])
                    || value.contains(['\r', '\n'])
                    || !portable(key, value)
                {
                    return Err(Error::Invalid(format!("setting cannot be synced: {key}")));
                }
            }
        }
    }
    Ok(())
}

fn safe_file(file: &str) -> bool {
    file.to_ascii_lowercase().ends_with(".cfg")
        && !file.contains(['\\', ':', '\0'])
        && file.split('/').all(|part| {
            !part.is_empty() && part != "." && part != ".." && !part.ends_with(['.', ' '])
        })
}

pub(super) fn manifests(setup: &Setup) -> Result<(Manifest, Lockfile)> {
    let mut manifest = Manifest::new(VALHEIM, Target::Client);
    let mut packages = Vec::new();
    for (id, value) in &setup.mods {
        let id: PackageId = id.parse()?;
        let version = parse_version(&value.version)?;
        if let Some(requested) = &value.requested {
            let requested: VersionReq = requested.parse()?;
            if matches!(&requested, VersionReq::Exact(pin) if pin != &version) {
                return Err(Error::Invalid(format!(
                    "{id} does not match its pinned version"
                )));
            }
            manifest.mods.insert(
                id.clone(),
                ModSpec {
                    version: requested,
                    enabled: value.enabled,
                },
            );
        } else if !value.enabled {
            return Err(Error::Invalid(format!(
                "dependency {id} cannot be disabled separately"
            )));
        }
        let mut dependencies = Vec::new();
        for dependency in &value.dependencies {
            if !setup.mods.contains_key(dependency) {
                return Err(Error::Invalid(format!(
                    "{id} needs {dependency}; review the mod choices"
                )));
            }
            dependencies.push(dependency.parse()?);
        }
        packages.push(LockedPackage {
            id,
            version,
            dependencies,
        });
    }
    Ok((manifest, Lockfile::new(packages)))
}
