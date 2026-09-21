//! What travels between the blackforge app and its server. Both sides build
//! against this one crate, so a request and its answer cannot drift apart.
//!
//! Every route wants `Authorization: Bearer <session token>` from the Google
//! login of the engine.

pub mod username;

use std::collections::BTreeMap;

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
    /// The link to the Google picture of everybody named above, by username.
    /// Somebody with no picture is not in it. It is a map next to the lists
    /// because the released apps read `incoming` and `outgoing` as plain names.
    #[serde(default)]
    pub pictures: BTreeMap<String, String>,
}

/// A friend never shows their email or Google name, only what they picked
/// and the picture.
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

pub const SEARCH_LIMIT: u8 = 20;

/// The query of `GET /api/users/search`. `q` is the start of a username, one
/// letter is enough, the rule is `username::normalize_start`. An empty `q`
/// finds nobody.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Search {
    pub q: String,
}

/// One answer of the search. Any signed in user can find any other one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FoundUser {
    pub username: String,
    pub picture: Option<String>,
    pub relation: Relation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Relation {
    Stranger,
    Friend,
    /// I sent a request and wait for the answer.
    Asked,
    /// They sent me a request.
    AskedMe,
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
            pictures: BTreeMap::from([("anna".to_owned(), "https://p/anna.png".to_owned())]),
        };
        let json = to_string(&friends).unwrap();
        assert_eq!(
            json,
            r#"{"friends":[{"username":"anna","in_game":true}],"incoming":["bob"],"outgoing":[],"pictures":{"anna":"https://p/anna.png"}}"#
        );
        assert_eq!(from_str::<Friends>(&json).unwrap(), friends);
    }

    /// A server older than the pictures still answers a new app.
    #[test]
    fn friends_without_pictures_still_read() {
        let friends: Friends =
            from_str(r#"{"friends":[],"incoming":["bob"],"outgoing":[]}"#).unwrap();
        assert_eq!(friends.incoming, ["bob"]);
        assert!(friends.pictures.is_empty());
    }

    #[test]
    fn found_user_shape() {
        let found = FoundUser {
            username: "anna".to_owned(),
            picture: None,
            relation: Relation::AskedMe,
        };
        let json = to_string(&found).unwrap();
        assert_eq!(
            json,
            r#"{"username":"anna","picture":null,"relation":"asked_me"}"#
        );
        assert_eq!(from_str::<FoundUser>(&json).unwrap(), found);
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
