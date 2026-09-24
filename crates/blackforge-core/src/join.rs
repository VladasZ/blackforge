//! Adds a join button per server to the main menu of the game.
//!
//! A small loader plugin, the source is in `assets/join`. The servers are the
//! registered ones with a join address, written to `servers.json` next to the
//! plugin before every start. A button finds the lobby of its server by the
//! address and the name. Before the join it asks the running app for a one
//! time code, a server lets in only a code from blackforge, see
//! `docs/gate.md`. The start arguments of [`bridge_args`] tell the plugin
//! where the app listens.
//!
//! The plugin belongs to no mod, so the lock never lists it and `sync` leaves
//! it alone. It has its own folder, since the achievements plugin removes its
//! folder when that setting is off.

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use blackforge_api::servers::{JOIN_ADMIN, Server};
use serde::Serialize;
use tokio::{fs, time::timeout};

use crate::{
    error::{IoContext, Result},
    game::{GameDef, Target, VALHEIM},
    http::Client,
    servers::fetch,
};

const PLUGIN: &[u8] = include_bytes!("../../../assets/join/BlackforgeJoin.dll");
const PLUGIN_DIR: [&str; 3] = ["BepInEx", "plugins", "blackforge-join"];
const PLUGIN_FILE: &str = "BlackforgeJoin.dll";
const LIST_FILE: &str = "servers.json";
/// The list is fetched on the way to the game, a slow server must not hold
/// the start up for long.
const FETCH_TIMEOUT: Duration = Duration::from_secs(5);

/// The shape of `servers.json`. Unity's `JsonUtility` in the plugin reads only
/// an object at the top, so the list sits in a field.
#[derive(Debug, Serialize)]
struct JoinList<'a> {
    servers: Vec<JoinServer<'a>>,
}

#[derive(Debug, Serialize)]
struct JoinServer<'a> {
    /// The plugin asks the app for a join code of this server.
    id: &'a str,
    name: &'a str,
    address: &'a str,
}

/// The game start argument with the port the app listens on for the plugin.
pub const BRIDGE_PORT_ARG: &str = "-blackforge-bridge";
/// The game start argument with the key the plugin shows the app, so no other
/// program on the machine gets a join code.
pub const BRIDGE_KEY_ARG: &str = "-blackforge-key";

/// The arguments that tell the plugin where the app listens and how to prove
/// it is the game the app started.
pub fn bridge_args(port: u16, key: &str) -> Vec<String> {
    vec![
        BRIDGE_PORT_ARG.to_owned(),
        port.to_string(),
        BRIDGE_KEY_ARG.to_owned(),
        key.to_owned(),
    ]
}

/// Only the Valheim client has a main menu, and the plugin patches Valheim
/// code.
pub fn applies_to(game: &GameDef) -> bool {
    game.label == VALHEIM && game.target == Target::Client
}

pub fn plugin_path(profile_dir: &Path) -> PathBuf {
    plugin_dir(profile_dir).join(PLUGIN_FILE)
}

pub fn list_path(profile_dir: &Path) -> PathBuf {
    plugin_dir(profile_dir).join(LIST_FILE)
}

fn plugin_dir(profile_dir: &Path) -> PathBuf {
    PLUGIN_DIR
        .iter()
        .fold(profile_dir.to_path_buf(), |path, part| path.join(part))
}

/// The servers that get a button, oldest registration first, so the main
/// server keeps the top spot. The backend lets only the join admin set an
/// address, the owner check here keeps a stray row out of every menu anyway.
fn join_list(servers: &[Server]) -> Result<String> {
    let mut servers: Vec<&Server> = servers.iter().collect();
    servers.sort_by_key(|server| server.created);
    let servers = servers
        .into_iter()
        .filter(|server| server.game == VALHEIM && server.owner == JOIN_ADMIN)
        .filter_map(|server| {
            server.address.as_deref().map(|address| JoinServer {
                id: &server.id,
                name: &server.name,
                address,
            })
        })
        .collect();
    Ok(serde_json::to_string_pretty(&JoinList { servers })?)
}

pub async fn write_list(profile_dir: &Path, servers: &[Server]) -> Result<()> {
    let path = list_path(profile_dir);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).await.at(dir)?;
    }
    fs::write(&path, join_list(servers)?).await.at(&path)
}

/// Fetches the servers and writes the list. A failed or slow fetch keeps the
/// list of the last start, the game starts either way.
pub async fn refresh_list(profile_dir: &Path) -> Result<()> {
    let fetched = timeout(FETCH_TIMEOUT, async { fetch(&Client::new()?).await }).await;
    match fetched {
        Ok(Ok(servers)) => write_list(profile_dir, &servers).await,
        _ => Ok(()),
    }
}

pub async fn apply(profile_dir: &Path) -> Result<()> {
    let path = plugin_path(profile_dir);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).await.at(dir)?;
    }
    fs::write(&path, PLUGIN).await.at(&path)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;
    use crate::testing::valheim;

    fn server(name: &str, owner: &str, address: Option<&str>, created: i64) -> Server {
        Server {
            id: name.to_owned(),
            name: name.to_owned(),
            game: VALHEIM.to_owned(),
            owner: owner.to_owned(),
            mods: Vec::new(),
            updated: 0,
            created,
            address: address.map(str::to_owned),
        }
    }

    #[tokio::test]
    async fn the_list_holds_only_admin_servers_with_an_address_oldest_first() -> Result<()> {
        let temp = tempdir().at(Path::new("tempdir"))?;
        let mut other_game = server("Valley", JOIN_ADMIN, Some("86.100.76.6:2456"), 0);
        other_game.game = "stardew".to_owned();
        // The backend sorts by name, the buttons go by registration.
        let servers = [
            server("Arkham Asylum", JOIN_ADMIN, Some("86.100.76.6:2456"), 20),
            server("Durka", JOIN_ADMIN, Some("86.100.76.6:2456"), 10),
            server("Mods only", JOIN_ADMIN, None, 0),
            server("Lookalike", "stranger", Some("1.2.3.4:2456"), 0),
            other_game,
        ];

        write_list(temp.path(), &servers).await?;
        let path = list_path(temp.path());
        let written: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).await.at(&path)?)?;
        assert_eq!(
            written,
            serde_json::json!({ "servers": [
                { "id": "Durka", "name": "Durka", "address": "86.100.76.6:2456" },
                { "id": "Arkham Asylum", "name": "Arkham Asylum", "address": "86.100.76.6:2456" },
            ]})
        );
        Ok(())
    }

    #[tokio::test]
    async fn writes_the_plugin() -> Result<()> {
        let temp = tempdir().at(Path::new("tempdir"))?;
        let path = plugin_path(temp.path());

        apply(temp.path()).await?;
        assert_eq!(fs::read(&path).await.at(&path)?, PLUGIN);

        // A second start writes it again over the old copy.
        apply(temp.path()).await?;
        assert_eq!(fs::read(&path).await.at(&path)?, PLUGIN);
        Ok(())
    }

    // The plugin looks for these exact words on the command line.
    #[test]
    fn bridge_args_name_the_port_and_the_key() {
        assert_eq!(
            bridge_args(51234, "k3y"),
            ["-blackforge-bridge", "51234", "-blackforge-key", "k3y"]
        );
    }

    #[test]
    fn only_the_valheim_client_gets_the_button() -> Result<()> {
        assert!(applies_to(&valheim(Target::Client)?));
        assert!(!applies_to(&valheim(Target::Server)?));
        Ok(())
    }
}
