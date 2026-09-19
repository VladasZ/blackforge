use std::{cmp::Reverse, collections::HashMap, path::PathBuf, time::Duration};

use futures_util::{StreamExt, TryStreamExt, stream};
use tokio::{fs, task::spawn_blocking};

use crate::{
    error::{Error, IoContext, Result},
    game::GameDef,
    http::Client,
    ident::PackageId,
    paths::DataDir,
    progress::{Event, Progress},
    thunderstore::{
        cache::CacheFile,
        model::{ApiPackage, Package, PackageVersion},
    },
    util::file_age,
};

const PARALLEL_CHUNKS: usize = 4;

/// Commands that only read the list accept a copy this old.
pub const FRESH_ENOUGH: Duration = Duration::from_hours(1);

/// Every package of one Thunderstore community.
#[derive(Clone, Debug)]
pub struct PackageIndex {
    packages: Vec<Package>,
    by_id: HashMap<PackageId, usize>,
    /// Entries of the API answer that did not parse and were left out.
    skipped: usize,
}

impl PackageIndex {
    pub fn from_packages(packages: Vec<Package>) -> Self {
        Self::build(packages, 0)
    }

    fn build(packages: Vec<Package>, skipped: usize) -> Self {
        let by_id = packages
            .iter()
            .enumerate()
            .map(|(at, package)| (package.id.clone(), at))
            .collect();
        Self {
            packages,
            by_id,
            skipped,
        }
    }

    fn cache_path(data: &DataDir, game: &GameDef) -> PathBuf {
        data.index_dir().join(format!("{}.json", game.label))
    }

    /// The cached list when it is younger than `max_age`, a fresh download
    /// otherwise.
    pub async fn load(
        client: &Client,
        data: &DataDir,
        game: &GameDef,
        max_age: Duration,
        progress: &Progress,
    ) -> Result<Self> {
        let path = Self::cache_path(data, game);
        if file_age(&path).await.is_some_and(|age| age < max_age) {
            return Self::read_cache(path).await;
        }
        Self::refresh(client, data, game, progress).await
    }

    /// The cached list of any age, for work that must not need the network.
    pub async fn cached(data: &DataDir, game: &GameDef) -> Result<Self> {
        let path = Self::cache_path(data, game);
        if file_age(&path).await.is_none() {
            return Err(Error::IndexMissing);
        }
        Self::read_cache(path).await
    }

    async fn read_cache(path: PathBuf) -> Result<Self> {
        let bytes = fs::read(&path).await.at(&path)?;
        let (packages, skipped) = spawn_blocking(move || {
            let cache: CacheFile = serde_json::from_slice(&bytes)?;
            cache.unpack()
        })
        .await??;
        Ok(Self::build(packages, skipped))
    }

    pub async fn refresh(
        client: &Client,
        data: &DataDir,
        game: &GameDef,
        progress: &Progress,
    ) -> Result<Self> {
        let chunk_urls: Vec<String> = client.get_json(&game.package_index).await?;
        let total = chunk_urls.len();
        progress.send(Event::IndexChunk { done: 0, total })?;

        let mut done = 0;
        let mut api_packages = Vec::new();
        let mut chunks = stream::iter(chunk_urls)
            .map(|url| async move { client.get_json::<Vec<ApiPackage>>(&url).await })
            .buffer_unordered(PARALLEL_CHUNKS);
        while let Some(chunk) = chunks.try_next().await? {
            api_packages.extend(chunk);
            done += 1;
            progress.send(Event::IndexChunk { done, total })?;
        }

        let received = api_packages.len();
        let mut packages: Vec<Package> = api_packages
            .into_iter()
            .filter_map(Package::from_api)
            .collect();
        packages.sort_by(|a, b| a.id.cmp(&b.id));
        let skipped = received - packages.len();

        let path = Self::cache_path(data, game);
        fs::create_dir_all(data.index_dir())
            .await
            .at(&data.index_dir())?;
        let (packages, bytes) = spawn_blocking(move || {
            let bytes = serde_json::to_vec(&CacheFile::pack(&packages, skipped)?)?;
            Ok::<_, Error>((packages, bytes))
        })
        .await??;
        fs::write(&path, bytes).await.at(&path)?;
        Ok(Self::build(packages, skipped))
    }

    pub fn len(&self) -> usize {
        self.packages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.packages.is_empty()
    }

    pub fn skipped(&self) -> usize {
        self.skipped
    }

    pub fn get(&self, id: &PackageId) -> Option<&Package> {
        self.by_id.get(id).map(|&at| &self.packages[at])
    }

    pub fn require(&self, id: &PackageId) -> Result<&Package> {
        self.get(id)
            .ok_or_else(|| Error::PackageNotFound(id.to_string()))
    }

    pub fn latest(&self, id: &PackageId) -> Result<&PackageVersion> {
        self.require(id)?
            .latest()
            .ok_or_else(|| Error::PackageNotFound(id.to_string()))
    }

    /// Accepts `Owner-Name`, or a bare `Name` when only one package has it.
    pub fn find(&self, query: &str) -> Result<&Package> {
        if let Ok(id) = query.parse::<PackageId>() {
            return self.require(&id);
        }
        let wanted = query.to_lowercase();
        let mut matches: Vec<&Package> = self
            .packages
            .iter()
            .filter(|package| package.id.name().to_lowercase() == wanted)
            .collect();
        matches.sort_by_key(|package| Reverse(package.downloads));
        match matches.as_slice() {
            [] => Err(Error::PackageNotFound(query.to_owned())),
            [only] => Ok(only),
            several => {
                let names: Vec<String> = several
                    .iter()
                    .map(|package| package.id.to_string())
                    .collect();
                Err(Error::Invalid(format!(
                    "several packages are named '{query}', pick one: {}",
                    names.join(", ")
                )))
            }
        }
    }

    /// Every word of `query` must be in the id or the description. A hit in
    /// the name ranks first, then more downloads.
    pub fn search(&self, query: &str) -> Vec<&Package> {
        let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        let mut hits: Vec<(bool, &Package)> = self
            .packages
            .iter()
            .filter(|package| !package.deprecated)
            .filter_map(|package| {
                let id = package.id.to_string().to_lowercase();
                let description = package.description.to_lowercase();
                let all = words
                    .iter()
                    .all(|word| id.contains(word) || description.contains(word));
                all.then(|| (words.iter().all(|word| id.contains(word)), package))
            })
            .collect();
        hits.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.downloads.cmp(&a.1.downloads)));
        hits.into_iter().map(|(_, package)| package).collect()
    }
}
