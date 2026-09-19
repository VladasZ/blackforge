mod locate;
mod schema;

use std::fmt::{self, Display, Formatter};

pub use locate::{GameInstall, locate};
pub use schema::{InstallRule, Schema, TrackingMethod};
use serde::{Deserialize, Serialize};

use crate::ident::PackageId;

pub const VALHEIM: &str = "valheim";

/// What a profile is for. The schema names these `game` and `server`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Target {
    #[default]
    Client,
    Server,
}

impl Display for Target {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Client => "client",
            Self::Server => "server",
        })
    }
}

/// A mod loader package, for example `denikson-BepInExPack_Valheim`. Its zip
/// keeps the loader inside `root_folder`, and that folder is unpacked into
/// the profile root instead of going through the install rules.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoaderPackage {
    pub id: PackageId,
    pub root_folder: String,
}

/// One game and one target, taken from the Thunderstore schema.
#[derive(Clone, Debug)]
pub struct GameDef {
    pub label: String,
    pub display_name: String,
    pub target: Target,
    pub steam_app_id: Option<u32>,
    /// The app id of the game itself. A server has its own `steam_app_id` but
    /// still reports to Steam under this one.
    pub client_steam_app_id: Option<u32>,
    pub steam_folder_name: String,
    pub data_folder_name: String,
    pub exe_names: Vec<String>,
    pub package_index: String,
    pub package_loader: String,
    pub install_rules: Vec<InstallRule>,
    pub file_exclusions: Vec<String>,
    pub loader_packages: Vec<LoaderPackage>,
}

impl GameDef {
    pub fn loader_package(&self, id: &PackageId) -> Option<&LoaderPackage> {
        self.loader_packages.iter().find(|loader| &loader.id == id)
    }
}
