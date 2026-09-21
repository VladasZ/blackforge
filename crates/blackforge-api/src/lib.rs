//! What travels between the blackforge app and its server. Both sides build
//! against this one crate, so a request and its answer cannot drift apart.
//!
//! Every route wants `Authorization: Bearer <session token>` from the Google
//! login of the engine.

pub mod username;

use serde::{Deserialize, Serialize};

/// `GET /api/me`. A user with no username yet has just logged in for the first
/// time and has to pick one before anything else works.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Me {
    pub username: Option<String>,
}

/// `POST /api/me/username`. The name is fixed once it is set.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetUsername {
    pub username: String,
}

/// `GET /api/friends`, what the Friends page polls.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Friends {
    pub friends: Vec<Friend>,
    /// Requests other people sent to me.
    pub incoming: Vec<String>,
    /// Requests I sent that nobody answered yet.
    pub outgoing: Vec<String>,
}

/// A friend never shows their email or Google name, only what they picked.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Friend {
    pub username: String,
    pub in_game: bool,
}

/// The body of `POST /api/friends/request`, `/accept`, `/decline` and
/// `/remove`. Remove also takes back a request of my own.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FriendName {
    pub username: String,
}

/// `PUT /api/profile` uploads mine, `GET /api/friends/{username}/profile`
/// reads a friend's.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedProfile {
    pub mods: Vec<SharedMod>,
    pub configs: Vec<SharedConfig>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedMod {
    /// Thunderstore `Owner-Name`.
    pub id: String,
    pub version: String,
    pub enabled: bool,
}

/// The settings of one `.cfg` file that differ from their defaults. A
/// setting that looks like a secret never gets in here.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedConfig {
    /// Relative to `BepInEx/config`, with `/` between folders.
    pub file: String,
    pub settings: Vec<SharedSetting>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedSetting {
    pub section: String,
    pub key: String,
    pub value: String,
}

/// `POST /api/status`. The app sends it at game start, once a minute while
/// the game runs, and at exit. Silence for two minutes reads as not in game.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Status {
    pub in_game: bool,
}

/// The body of every answer that is not a success.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiError {
    pub error: String,
}

#[cfg(test)]
mod tests {
    use serde_json::{from_str, to_string};

    use super::*;

    #[test]
    fn friends_shape() {
        let friends = Friends {
            friends: vec![Friend {
                username: "anna".to_owned(),
                in_game: true,
            }],
            incoming: vec!["bob".to_owned()],
            outgoing: Vec::new(),
        };
        let json = to_string(&friends).unwrap();
        assert_eq!(
            json,
            r#"{"friends":[{"username":"anna","in_game":true}],"incoming":["bob"],"outgoing":[]}"#
        );
        assert_eq!(from_str::<Friends>(&json).unwrap(), friends);
    }

    #[test]
    fn profile_round_trip() {
        let profile = SharedProfile {
            mods: vec![SharedMod {
                id: "denikson-BepInExPack_Valheim".to_owned(),
                version: "5.4.2202".to_owned(),
                enabled: true,
            }],
            configs: vec![SharedConfig {
                file: "owner.mod.cfg".to_owned(),
                settings: vec![SharedSetting {
                    section: "General".to_owned(),
                    key: "Count".to_owned(),
                    value: "9".to_owned(),
                }],
            }],
        };
        let json = to_string(&profile).unwrap();
        assert_eq!(from_str::<SharedProfile>(&json).unwrap(), profile);
    }
}
