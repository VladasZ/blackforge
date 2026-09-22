//! Game servers people register, so a player installs the mods a server
//! requires with one click instead of by hand. A server is a name and a mod
//! list, nothing else. The address and the password never reach blackforge,
//! players join through the game as before.
//!
//! `GET /api/servers` lists every server to anybody, no login needed.
//! `POST /api/servers` registers one, `PUT /api/servers/{id}` and
//! `DELETE /api/servers/{id}` change or remove an own one.

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const NAME_MAX: usize = 40;
/// More mods than any real server list, a guard against a runaway upload.
pub const MODS_MAX: usize = 200;

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
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::{from_str, to_string};

    use super::{SaveServer, Server, ServerError, ServerMod, normalize_name, validate};

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
        };
        let json = to_string(&server).unwrap();
        assert_eq!(
            json,
            r#"{"id":"6d5c","name":"Durka","game":"valheim","owner":"vladas","mods":[{"id":"Grantapher-ValheimPlus_Grantapher_Temporary","version":"10.2.0"}],"updated":1790090031}"#
        );
        assert_eq!(from_str::<Server>(&json).unwrap(), server);
    }
}
