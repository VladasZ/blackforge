//! Private, revisioned setup storage. Never exposed through the friends API.
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub type Settings = BTreeMap<String, BTreeMap<String, BTreeMap<String, String>>>;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Setup {
    pub mods: BTreeMap<String, Mod>,
    pub configs: Settings,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mod {
    pub version: String,
    /// None for a dependency; `*` or an exact version for an explicitly chosen mod.
    pub requested: Option<String>,
    pub enabled: bool,
    pub dependencies: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedSetup {
    pub revision: i64,
    pub setup: Setup,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetupAccount {
    /// Stable account ID, also namespaces the local sync history.
    pub account: String,
    pub saved: Option<SavedSetup>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveSetup {
    /// Zero creates the first revision. An outdated revision never writes.
    pub revision: i64,
    pub setup: Setup,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SaveResult {
    Saved(SavedSetup),
    Conflict,
}
