//! The package list as it is kept on disk between runs.
//!
//! Every distinct dependency string is stored once in a table and the
//! versions point into it by number. Written out in full the list is about
//! 80 MB of json, and every command has to load it.

use std::{collections::HashMap, sync::Arc};

use semver::Version;
use serde::{Deserialize, Serialize};

use crate::{
    error::{Error, Result},
    ident::{PackageId, VersionedId},
    thunderstore::model::{Package, PackageVersion},
};

#[derive(Serialize, Deserialize)]
pub(super) struct CacheFile {
    skipped: usize,
    dependencies: Vec<VersionedId>,
    packages: Vec<CachedPackage>,
}

#[derive(Serialize, Deserialize)]
struct CachedPackage {
    id: PackageId,
    description: String,
    website_url: String,
    package_url: String,
    updated: String,
    rating: i64,
    downloads: u64,
    pinned: bool,
    deprecated: bool,
    categories: Vec<String>,
    versions: Vec<CachedVersion>,
}

#[derive(Serialize, Deserialize)]
struct CachedVersion {
    version: Version,
    dependencies: Vec<u32>,
    file_size: u64,
    downloads: u64,
}

impl CacheFile {
    pub(super) fn pack(packages: &[Package], skipped: usize) -> Result<Self> {
        let mut table: Vec<VersionedId> = Vec::new();
        let mut seen: HashMap<VersionedId, u32> = HashMap::new();
        let mut cached = Vec::with_capacity(packages.len());
        for package in packages {
            let mut versions = Vec::with_capacity(package.versions.len());
            for release in &package.versions {
                let mut dependencies = Vec::with_capacity(release.dependencies.len());
                for dependency in &release.dependencies {
                    let at = if let Some(at) = seen.get(dependency.as_ref()) {
                        *at
                    } else {
                        let at = u32::try_from(table.len()).map_err(|_| {
                            Error::Invalid("the package list has too many dependencies".to_owned())
                        })?;
                        table.push(dependency.as_ref().clone());
                        seen.insert(dependency.as_ref().clone(), at);
                        at
                    };
                    dependencies.push(at);
                }
                versions.push(CachedVersion {
                    version: release.version.clone(),
                    dependencies,
                    file_size: release.file_size,
                    downloads: release.downloads,
                });
            }
            cached.push(CachedPackage {
                id: package.id.clone(),
                description: package.description.clone(),
                website_url: package.website_url.clone(),
                package_url: package.package_url.clone(),
                updated: package.updated.clone(),
                rating: package.rating,
                downloads: package.downloads,
                pinned: package.pinned,
                deprecated: package.deprecated,
                categories: package.categories.clone(),
                versions,
            });
        }
        Ok(Self {
            skipped,
            dependencies: table,
            packages: cached,
        })
    }

    pub(super) fn unpack(self) -> Result<(Vec<Package>, usize)> {
        let table: Vec<Arc<VersionedId>> = self.dependencies.into_iter().map(Arc::new).collect();
        let mut packages = Vec::with_capacity(self.packages.len());
        for package in self.packages {
            let mut versions = Vec::with_capacity(package.versions.len());
            for release in package.versions {
                let mut dependencies = Vec::with_capacity(release.dependencies.len());
                for at in release.dependencies {
                    let shared = usize::try_from(at)
                        .ok()
                        .and_then(|at| table.get(at))
                        .ok_or_else(|| {
                            Error::Invalid(
                                "the cached package list is damaged, delete the index folder"
                                    .to_owned(),
                            )
                        })?;
                    dependencies.push(Arc::clone(shared));
                }
                versions.push(PackageVersion {
                    version: release.version,
                    dependencies,
                    file_size: release.file_size,
                    downloads: release.downloads,
                });
            }
            packages.push(Package {
                id: package.id,
                description: package.description,
                website_url: package.website_url,
                package_url: package.package_url,
                updated: package.updated,
                rating: package.rating,
                downloads: package.downloads,
                pinned: package.pinned,
                deprecated: package.deprecated,
                categories: package.categories,
                versions,
            });
        }
        Ok((packages, self.skipped))
    }
}
