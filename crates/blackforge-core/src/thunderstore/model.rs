use std::sync::Arc;

use semver::Version;
use serde::Deserialize;

use crate::ident::{PackageId, VersionedId, parse_version};

/// A package as the Thunderstore API sends it.
#[derive(Debug, Deserialize)]
pub(super) struct ApiPackage {
    full_name: String,
    package_url: String,
    date_updated: String,
    rating_score: i64,
    is_pinned: bool,
    is_deprecated: bool,
    categories: Vec<String>,
    versions: Vec<ApiVersion>,
}

#[derive(Debug, Deserialize)]
struct ApiVersion {
    version_number: String,
    description: String,
    dependencies: Vec<String>,
    downloads: u64,
    file_size: u64,
    website_url: String,
}

/// The slim form blackforge works with. The API sends the description for
/// every version, which is a large part of its 180 MB, and only the newest is
/// kept.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Package {
    pub id: PackageId,
    pub description: String,
    pub website_url: String,
    pub package_url: String,
    pub updated: String,
    pub rating: i64,
    pub downloads: u64,
    pub pinned: bool,
    pub deprecated: bool,
    pub categories: Vec<String>,
    /// Newest first.
    pub versions: Vec<PackageVersion>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageVersion {
    pub version: Version,
    /// Shared, because a mod pack repeats the same few hundred dependencies in
    /// every one of its versions. The whole list holds about 2 million of
    /// them and only a small part is distinct.
    pub dependencies: Vec<Arc<VersionedId>>,
    pub file_size: u64,
    pub downloads: u64,
}

impl Package {
    pub fn latest(&self) -> Option<&PackageVersion> {
        self.versions.first()
    }

    pub fn version(&self, version: &Version) -> Option<&PackageVersion> {
        self.versions
            .iter()
            .find(|candidate| &candidate.version == version)
    }

    /// `None` when the id, a version, or a dependency does not parse. The
    /// caller counts these so a bad entry is reported and not silently lost.
    pub(super) fn from_api(api: ApiPackage) -> Option<Self> {
        let id: PackageId = api.full_name.parse().ok()?;
        let newest = api.versions.first()?;
        let description = newest.description.clone();
        let website_url = newest.website_url.clone();
        let downloads = api.versions.iter().map(|version| version.downloads).sum();

        let mut versions = Vec::with_capacity(api.versions.len());
        for version in api.versions {
            let dependencies: Option<Vec<Arc<VersionedId>>> = version
                .dependencies
                .iter()
                .map(|dependency| dependency.parse().ok().map(Arc::new))
                .collect();
            versions.push(PackageVersion {
                version: parse_version(&version.version_number).ok()?,
                dependencies: dependencies?,
                file_size: version.file_size,
                downloads: version.downloads,
            });
        }
        versions.sort_by(|a, b| b.version.cmp(&a.version));

        Some(Self {
            id,
            description,
            website_url,
            package_url: api.package_url,
            updated: api.date_updated,
            rating: api.rating_score,
            downloads,
            pinned: api.is_pinned,
            deprecated: api.is_deprecated,
            categories: api.categories,
            versions,
        })
    }
}

pub fn download_url(package: &VersionedId) -> String {
    format!(
        "https://thunderstore.io/package/download/{}/{}/{}/",
        package.id.owner(),
        package.id.name(),
        package.version
    )
}
