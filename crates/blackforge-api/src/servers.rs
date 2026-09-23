//! Game servers people register, so a player installs the mods a server
//! requires with one click instead of by hand. A server is a name and a mod
//! list. A server of the join admin can also carry a join address, the app
//! then gives it a join button in the game menu, see `docs/join.md`. The
//! password never reaches blackforge, the game asks for it.
//!
//! `GET /api/servers` lists every server to anybody, no login needed.
//! `POST /api/servers` registers one, `PUT /api/servers/{id}` and
//! `DELETE /api/servers/{id}` change or remove an own one.

use std::net::SocketAddrV4;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const NAME_MAX: usize = 40;
/// More mods than any real server list, a guard against a runaway upload.
pub const MODS_MAX: usize = 200;
/// The one account that may give a server a join address. Every player gets a
/// button for such a server in the game menu, so a stranger must not add one.
/// A username is fixed once it is set, so it cannot be taken over.
pub const JOIN_ADMIN: &str = "vladas";

/// One mod a server requires, Thunderstore `Owner-Name` at an exact version.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerMod {
    pub id: String,
    pub version: String,
}

/// The body of `POST /api/servers` and `PUT /api/servers/{id}`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveServer {
    pub name: String,
    /// The game label of the profile, `valheim`.
    pub game: String,
    pub mods: Vec<ServerMod>,
    /// The public `ip:port` of the lobby, which with the name finds the
    /// server. No field keeps the stored address, an app before the field
    /// sends none and must not wipe it. An empty text removes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
}

/// One row of `GET /api/servers`, and the answer of a save.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Server {
    pub id: String,
    pub name: String,
    pub game: String,
    /// The username of who registered it.
    pub owner: String,
    pub mods: Vec<ServerMod>,
    /// Unix seconds of the last save.
    pub updated: i64,
    /// Set only for a server of `JOIN_ADMIN`, see `SaveServer::address`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum ServerError {
    #[error("a server needs a name")]
    NoName,
    #[error("a server name has at most {NAME_MAX} characters")]
    NameTooLong,
    #[error("a server needs at least one mod")]
    NoMods,
    #[error("a server lists at most {MODS_MAX} mods")]
    TooManyMods,
    #[error("a mod of the server has no id or no version")]
    BadMod,
    #[error("a join address is an ipv4 address and a port, like 86.100.76.6:2456")]
    BadAddress,
}

/// The name as it is stored, trimmed, or why the text cannot be one.
pub fn normalize_name(name: &str) -> Result<String, ServerError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(ServerError::NoName);
    }
    if name.chars().count() > NAME_MAX {
        return Err(ServerError::NameTooLong);
    }
    Ok(name.to_owned())
}

/// The address as it is stored, trimmed, `None` for an empty text. The game
/// writes the lobby address as `ip:port`, and a server join matches that
/// text exactly, so only that shape is taken.
pub fn normalize_address(address: &str) -> Result<Option<String>, ServerError> {
    let address = address.trim();
    if address.is_empty() {
        return Ok(None);
    }
    address
        .parse::<SocketAddrV4>()
        .map(|parsed| Some(parsed.to_string()))
        .map_err(|_| ServerError::BadAddress)
}

/// The same check on both sides, the window before it sends and the server
/// before it stores.
pub fn validate(save: &SaveServer) -> Result<(), ServerError> {
    normalize_name(&save.name)?;
    if save.mods.is_empty() {
        return Err(ServerError::NoMods);
    }
    if save.mods.len() > MODS_MAX {
        return Err(ServerError::TooManyMods);
    }
    if save
        .mods
        .iter()
        .any(|server_mod| server_mod.id.trim().is_empty() || server_mod.version.trim().is_empty())
    {
        return Err(ServerError::BadMod);
    }
    if let Some(address) = &save.address {
        normalize_address(address)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::{from_str, to_string};

    use super::{
        SaveServer, Server, ServerError, ServerMod, normalize_address, normalize_name, validate,
    };

    fn valheim_plus() -> ServerMod {
        ServerMod {
            id: "Grantapher-ValheimPlus_Grantapher_Temporary".to_owned(),
            version: "10.2.0".to_owned(),
        }
    }

    #[test]
    fn a_name_is_trimmed_and_bounded() {
        assert_eq!(normalize_name("  Durka "), Ok("Durka".to_owned()));
        assert_eq!(normalize_name("   "), Err(ServerError::NoName));
        assert_eq!(
            normalize_name(&"x".repeat(41)),
            Err(ServerError::NameTooLong)
        );
    }

    #[test]
    fn a_server_needs_a_name_and_real_mods() {
        let good = SaveServer {
            name: "Durka".to_owned(),
            game: "valheim".to_owned(),
            mods: vec![valheim_plus()],
            address: None,
        };
        assert_eq!(validate(&good), Ok(()));

        let mut no_mods = good.clone();
        no_mods.mods.clear();
        assert_eq!(validate(&no_mods), Err(ServerError::NoMods));

        let mut bad_mod = good.clone();
        bad_mod.mods[0].version = " ".to_owned();
        assert_eq!(validate(&bad_mod), Err(ServerError::BadMod));

        let mut no_name = good;
        no_name.name = String::new();
        assert_eq!(validate(&no_name), Err(ServerError::NoName));
    }

    #[test]
    fn server_shape() {
        let server = Server {
            id: "6d5c".to_owned(),
            name: "Durka".to_owned(),
            game: "valheim".to_owned(),
            owner: "vladas".to_owned(),
            mods: vec![valheim_plus()],
            updated: 1_790_090_031,
            address: None,
        };
        let json = to_string(&server).unwrap();
        assert_eq!(
            json,
            r#"{"id":"6d5c","name":"Durka","game":"valheim","owner":"vladas","mods":[{"id":"Grantapher-ValheimPlus_Grantapher_Temporary","version":"10.2.0"}],"updated":1790090031}"#
        );
        assert_eq!(from_str::<Server>(&json).unwrap(), server);
    }

    #[test]
    fn a_join_address_is_an_ipv4_and_a_port() {
        assert_eq!(
            normalize_address(" 86.100.76.6:2456 "),
            Ok(Some("86.100.76.6:2456".to_owned()))
        );
        assert_eq!(normalize_address("  "), Ok(None));
        assert_eq!(
            normalize_address("86.100.76.6"),
            Err(ServerError::BadAddress)
        );
        assert_eq!(
            normalize_address("durka.vladas.xyz:2456"),
            Err(ServerError::BadAddress)
        );

        let save = SaveServer {
            name: "Durka".to_owned(),
            game: "valheim".to_owned(),
            mods: vec![valheim_plus()],
            address: Some("2456".to_owned()),
        };
        assert_eq!(validate(&save), Err(ServerError::BadAddress));
    }

    #[test]
    fn an_old_save_has_no_address_field() {
        let save: SaveServer = from_str(r#"{"name":"Durka","game":"valheim","mods":[]}"#).unwrap();
        assert_eq!(save.address, None);

        let server = Server {
            id: "6d5c".to_owned(),
            name: "Durka".to_owned(),
            game: "valheim".to_owned(),
            owner: "vladas".to_owned(),
            mods: Vec::new(),
            updated: 1,
            address: Some("86.100.76.6:2456".to_owned()),
        };
        let json = to_string(&server).unwrap();
        assert!(json.ends_with(r#","address":"86.100.76.6:2456"}"#));
        assert_eq!(from_str::<Server>(&json).unwrap(), server);
    }
}
