use std::{collections::HashMap, time::Duration};

use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::{
    error::{Error, IoContext, Result},
    game::{GameDef, LoaderPackage, Target},
    http::Client,
    paths::DataDir,
    util::file_age,
};

const SCHEMA_URL: &str = "https://thunderstore.io/api/experimental/schema/dev/latest/";
const MAX_AGE: Duration = Duration::from_hours(7 * 24);

/// The Thunderstore ecosystem schema, only the parts blackforge reads.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Schema {
    games: HashMap<String, SchemaGame>,
    modloader_packages: Vec<SchemaLoaderPackage>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SchemaGame {
    #[serde(default)]
    distributions: Vec<SchemaDistribution>,
    #[serde(default)]
    r2modman: Option<Vec<SchemaEntry>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SchemaEntry {
    meta: SchemaMeta,
    data_folder_name: String,
    distributions: Vec<SchemaDistribution>,
    steam_folder_name: String,
    exe_names: Vec<String>,
    game_instance_type: String,
    package_index: String,
    package_loader: String,
    install_rules: Vec<InstallRule>,
    relative_file_exclusions: Option<Vec<String>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SchemaMeta {
    display_name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SchemaDistribution {
    platform: String,
    #[serde(default)]
    identifier: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SchemaLoaderPackage {
    package_id: String,
    root_folder: String,
    loader: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallRule {
    pub route: String,
    #[serde(default)]
    pub default_file_extensions: Vec<String>,
    pub tracking_method: TrackingMethod,
    #[serde(default)]
    pub sub_routes: Vec<InstallRule>,
    #[serde(default)]
    pub is_default_location: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TrackingMethod {
    Subdir,
    SubdirNoFlatten,
    State,
    None,
    PackageZip,
}

impl Schema {
    /// Reads the cached schema, and downloads it when the cache is missing or
    /// older than a week.
    pub async fn load(client: &Client, data: &DataDir) -> Result<Self> {
        let path = data.index_dir().join("schema.json");
        if file_age(&path).await.is_some_and(|age| age < MAX_AGE) {
            let text = fs::read(&path).await.at(&path)?;
            return Ok(serde_json::from_slice(&text)?);
        }
        let schema: Self = client.get_json(SCHEMA_URL).await?;
        fs::create_dir_all(data.index_dir())
            .await
            .at(&data.index_dir())?;
        fs::write(&path, serde_json::to_vec(&schema)?)
            .await
            .at(&path)?;
        Ok(schema)
    }

    pub fn from_json(json: &[u8]) -> Result<Self> {
        Ok(serde_json::from_slice(json)?)
    }

    pub fn game(&self, label: &str, target: Target) -> Result<GameDef> {
        let game = self
            .games
            .get(label)
            .ok_or_else(|| Error::UnknownGame(label.to_owned()))?;
        let wanted = match target {
            Target::Client => "game",
            Target::Server => "server",
        };
        let entry = game
            .r2modman
            .iter()
            .flatten()
            .find(|entry| entry.game_instance_type == wanted)
            .ok_or_else(|| Error::UnknownTarget {
                game: label.to_owned(),
                target: target.to_string(),
            })?;

        let steam_app_id = steam_id(&entry.distributions);
        let client_steam_app_id = steam_id(&game.distributions);

        // The list covers every game. An id that does not parse can never match a
        // package of this game, so it is left out instead of failing the lookup.
        let loader_packages = self
            .modloader_packages
            .iter()
            .filter(|package| package.loader == entry.package_loader)
            .filter_map(|package| {
                let id = package.package_id.parse().ok()?;
                Some(LoaderPackage {
                    id,
                    root_folder: package.root_folder.clone(),
                })
            })
            .collect();

        Ok(GameDef {
            label: label.to_owned(),
            display_name: entry.meta.display_name.clone(),
            target,
            steam_app_id,
            client_steam_app_id,
            steam_folder_name: entry.steam_folder_name.clone(),
            data_folder_name: entry.data_folder_name.clone(),
            exe_names: entry.exe_names.clone(),
            package_index: entry.package_index.clone(),
            package_loader: entry.package_loader.clone(),
            install_rules: entry.install_rules.clone(),
            file_exclusions: entry.relative_file_exclusions.clone().unwrap_or_default(),
            loader_packages,
        })
    }
}

fn steam_id(distributions: &[SchemaDistribution]) -> Option<u32> {
    distributions
        .iter()
        .filter(|distribution| distribution.platform == "steam")
        .find_map(|distribution| distribution.identifier.as_deref()?.parse().ok())
}
