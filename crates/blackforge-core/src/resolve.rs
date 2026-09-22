use std::collections::{BTreeMap, HashMap, VecDeque};

use semver::Version;

use crate::{
    error::{Error, Result},
    ident::PackageId,
    lock::{LockedPackage, Lockfile},
    manifest::Manifest,
    thunderstore::PackageIndex,
};

/// Which locked versions a resolve may move.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unlock {
    /// Keep every locked version. Only new packages get picked.
    Nothing,
    Everything,
    Only(Vec<PackageId>),
}

impl Unlock {
    fn frees(&self, id: &PackageId) -> bool {
        match self {
            Self::Nothing => false,
            Self::Everything => true,
            Self::Only(ids) => ids.contains(id),
        }
    }
}

/// A dependency that Thunderstore no longer has.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissingDependency {
    pub wanted: PackageId,
    pub required_by: PackageId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolution {
    pub lock: Lockfile,
    pub missing: Vec<MissingDependency>,
}

/// Builds the lock for `manifest`.
///
/// A dependency string names a version, and like r2modman and Gale it is read
/// as the lowest version that works. A package keeps its locked version while
/// that satisfies every requirement, otherwise it moves to the newest one.
pub fn resolve(
    manifest: &Manifest,
    index: &PackageIndex,
    previous: &Lockfile,
    unlock: &Unlock,
) -> Result<Resolution> {
    let mut lowest: HashMap<PackageId, Version> = HashMap::new();
    loop {
        let pass = walk(manifest, index, previous, unlock, &lowest)?;
        if pass.lowest == lowest {
            return Ok(Resolution {
                lock: Lockfile::new(pass.packages),
                missing: pass.missing,
            });
        }
        // A requirement found late in the walk can lift a package that was
        // already picked, so the walk repeats until the requirements settle.
        // They only ever rise, which bounds the number of passes.
        lowest = pass.lowest;
    }
}

struct Pass {
    packages: Vec<LockedPackage>,
    missing: Vec<MissingDependency>,
    lowest: HashMap<PackageId, Version>,
}

fn walk(
    manifest: &Manifest,
    index: &PackageIndex,
    previous: &Lockfile,
    unlock: &Unlock,
    known_lowest: &HashMap<PackageId, Version>,
) -> Result<Pass> {
    let mut lowest = known_lowest.clone();
    let mut picked: BTreeMap<PackageId, LockedPackage> = BTreeMap::new();
    let mut missing = Vec::new();
    let mut queue: VecDeque<(PackageId, Option<PackageId>)> =
        manifest.mods.keys().map(|id| (id.clone(), None)).collect();

    while let Some((id, required_by)) = queue.pop_front() {
        if picked.contains_key(&id) {
            continue;
        }
        let Some(package) = index.get(&id) else {
            match required_by {
                Some(required_by) => missing.push(MissingDependency {
                    wanted: id,
                    required_by,
                }),
                None => return Err(Error::PackageNotFound(id.to_string())),
            }
            continue;
        };

        let version = pick_version(manifest, index, previous, unlock, known_lowest, &id)?;
        let release = package
            .version(&version)
            .ok_or_else(|| Error::VersionNotFound {
                id: id.to_string(),
                version: version.to_string(),
            })?;

        let mut dependencies = Vec::new();
        for dependency in &release.dependencies {
            let entry = lowest
                .entry(dependency.id.clone())
                .or_insert_with(|| dependency.version.clone());
            if dependency.version > *entry {
                *entry = dependency.version.clone();
            }
            dependencies.push(dependency.id.clone());
            queue.push_back((dependency.id.clone(), Some(id.clone())));
        }
        dependencies.sort();
        dependencies.dedup();
        picked.insert(
            id.clone(),
            LockedPackage {
                id,
                version,
                dependencies,
            },
        );
    }

    // A dependency that is gone from Thunderstore stays out of the lock, the
    // packages that name it keep it in their list so the gap stays visible.
    Ok(Pass {
        packages: picked.into_values().collect(),
        missing,
        lowest,
    })
}

fn pick_version(
    manifest: &Manifest,
    index: &PackageIndex,
    previous: &Lockfile,
    unlock: &Unlock,
    lowest: &HashMap<PackageId, Version>,
    id: &PackageId,
) -> Result<Version> {
    if let Some(pinned) = manifest
        .mods
        .get(id)
        .and_then(|spec| spec.version.pinned_version())
    {
        return Ok(pinned.clone());
    }
    if !unlock.frees(id)
        && let Some(locked) = previous.get(id)
        && lowest
            .get(id)
            .is_none_or(|required| &locked.version >= required)
        && index
            .get(id)
            .is_some_and(|package| package.version(&locked.version).is_some())
    {
        return Ok(locked.version.clone());
    }
    Ok(index.latest(id)?.version.clone())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::{
        manifest::{ModSpec, VersionReq},
        thunderstore::{Package, PackageVersion},
    };

    fn package(id: &str, versions: &[(&str, &[&str])]) -> Result<Package> {
        let mut releases = Vec::new();
        for (version, dependencies) in versions {
            let dependencies: Result<Vec<_>> = dependencies
                .iter()
                .map(|dependency| dependency.parse().map(Arc::new))
                .collect();
            releases.push(PackageVersion {
                version: version
                    .parse()
                    .map_err(|_| Error::InvalidVersion((*version).to_owned()))?,
                dependencies: dependencies?,
                file_size: 0,
                downloads: 0,
            });
        }
        releases.sort_by(|a, b| b.version.cmp(&a.version));
        Ok(Package {
            id: id.parse()?,
            description: String::new(),
            website_url: String::new(),
            package_url: String::new(),
            updated: String::new(),
            rating: 0,
            downloads: 0,
            pinned: false,
            deprecated: false,
            categories: Vec::new(),
            versions: releases,
        })
    }

    fn index() -> Result<PackageIndex> {
        Ok(PackageIndex::from_packages(vec![
            package("x-Loader", &[("5.0.0", &[]), ("5.1.0", &[])])?,
            package(
                "x-Lib",
                &[
                    ("1.0.0", &["x-Loader-5.0.0"]),
                    ("1.1.0", &["x-Loader-5.0.0"]),
                    ("2.0.0", &["x-Loader-5.1.0"]),
                ],
            )?,
            package("x-ModA", &[("1.0.0", &["x-Lib-1.0.0"])])?,
            package("x-ModB", &[("1.0.0", &["x-Lib-2.0.0", "x-Gone-1.0.0"])])?,
        ]))
    }

    /// `*` follows the newest, a version is a pin of a test server.
    fn pin(version: &str) -> Result<VersionReq> {
        VersionReq::from_parts(version, Some("Test".to_owned()))
    }

    fn manifest(mods: &[(&str, &str)]) -> Result<Manifest> {
        let mut manifest = Manifest::new("valheim", crate::game::Target::Client);
        for (id, version) in mods {
            manifest
                .mods
                .insert(id.parse()?, ModSpec::new(pin(version)?));
        }
        Ok(manifest)
    }

    fn versions(lock: &Lockfile) -> Vec<String> {
        lock.packages
            .iter()
            .map(|package| package.versioned().to_string())
            .collect()
    }

    #[test]
    fn picks_newest_with_dependencies() -> Result<()> {
        let resolution = resolve(
            &manifest(&[("x-ModA", "*")])?,
            &index()?,
            &Lockfile::default(),
            &Unlock::Nothing,
        )?;
        assert_eq!(
            versions(&resolution.lock),
            ["x-Lib-2.0.0", "x-Loader-5.1.0", "x-ModA-1.0.0"]
        );
        assert!(resolution.missing.is_empty());
        Ok(())
    }

    #[test]
    fn keeps_locked_versions_when_adding() -> Result<()> {
        let index = index()?;
        let old = Lockfile::new(vec![
            LockedPackage {
                id: "x-Lib".parse()?,
                version: Version::new(1, 0, 0),
                dependencies: vec![],
            },
            LockedPackage {
                id: "x-Loader".parse()?,
                version: Version::new(5, 0, 0),
                dependencies: vec![],
            },
        ]);
        let resolution = resolve(
            &manifest(&[("x-ModA", "*")])?,
            &index,
            &old,
            &Unlock::Nothing,
        )?;
        assert_eq!(
            versions(&resolution.lock),
            ["x-Lib-1.0.0", "x-Loader-5.0.0", "x-ModA-1.0.0"]
        );

        let updated = resolve(
            &manifest(&[("x-ModA", "*")])?,
            &index,
            &old,
            &Unlock::Everything,
        )?;
        assert_eq!(
            versions(&updated.lock),
            ["x-Lib-2.0.0", "x-Loader-5.1.0", "x-ModA-1.0.0"]
        );
        Ok(())
    }

    #[test]
    fn lifts_a_locked_package_that_is_too_old() -> Result<()> {
        let old = Lockfile::new(vec![
            LockedPackage {
                id: "x-Lib".parse()?,
                version: Version::new(1, 0, 0),
                dependencies: vec![],
            },
            LockedPackage {
                id: "x-Loader".parse()?,
                version: Version::new(5, 0, 0),
                dependencies: vec![],
            },
        ]);
        let resolution = resolve(
            &manifest(&[("x-ModA", "*"), ("x-ModB", "*")])?,
            &index()?,
            &old,
            &Unlock::Nothing,
        )?;
        assert_eq!(
            versions(&resolution.lock),
            [
                "x-Lib-2.0.0",
                "x-Loader-5.1.0",
                "x-ModA-1.0.0",
                "x-ModB-1.0.0"
            ]
        );
        assert_eq!(
            resolution.missing,
            [MissingDependency {
                wanted: "x-Gone".parse()?,
                required_by: "x-ModB".parse()?,
            }]
        );
        Ok(())
    }

    #[test]
    fn a_pin_in_the_manifest_wins() -> Result<()> {
        let resolution = resolve(
            &manifest(&[("x-Lib", "1.1.0")])?,
            &index()?,
            &Lockfile::default(),
            &Unlock::Everything,
        )?;
        assert_eq!(
            versions(&resolution.lock),
            ["x-Lib-1.1.0", "x-Loader-5.1.0"]
        );
        Ok(())
    }

    #[test]
    fn unknown_root_is_an_error() -> Result<()> {
        let result = resolve(
            &manifest(&[("x-Nope", "*")])?,
            &index()?,
            &Lockfile::default(),
            &Unlock::Nothing,
        );
        assert!(matches!(result, Err(Error::PackageNotFound(_))));
        Ok(())
    }
}
