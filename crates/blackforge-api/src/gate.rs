//! Who may join the game servers of an owner, and the one time codes that let
//! them in. Blackforge is the only way onto such a server, see `docs/gate.md`.
//!
//! An owner keeps one member list for all of their servers.
//! `GET /api/members` lists it, `POST /api/members` adds a username and
//! `DELETE /api/members/{username}` removes one. The owner is always let in
//! and is never on the list.
//!
//! At a click on a join button the app asks `POST /api/servers/{id}/join` for
//! a code. The game sends it to the server, and the plugin in the server trades
//! it at `POST /api/gate/verify` for the username behind it. A code works once
//! and only for a short time.

use serde::{Deserialize, Serialize};

use crate::competitive::Forbidden;

/// How long a join code lives. The game uses it right after the click.
pub const CODE_SECONDS: i64 = 120;

/// One row of `GET /api/members`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Member {
    pub username: String,
    /// The link to the Google picture. A backend before the field sends none.
    #[serde(default)]
    pub picture: Option<String>,
}

/// The body of `POST /api/members`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddMember {
    pub username: String,
}

/// The answer of `POST /api/servers/{id}/join`. The app hands all of it to
/// the join plugin, which checks the character before the join.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JoinCode {
    pub code: String,
    /// See `competitive`. A backend before the field sends none.
    #[serde(default)]
    pub competitive: bool,
    /// The world the server runs now, items found in it carry this tag. None
    /// until the server reported its progress once.
    #[serde(default)]
    pub world: Option<String>,
    /// The materials the character may not carry, empty unless competitive.
    #[serde(default)]
    pub forbidden: Vec<Forbidden>,
    /// Items never forbidden, see `competitive::GameTiers::allow`.
    #[serde(default)]
    pub allowed: Vec<String>,
}

/// The body of `POST /api/gate/verify`. The route wants
/// `Authorization: Bearer <gate secret>`, only the game servers have it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Verify {
    /// The name the game server runs with, a code is for one server only.
    pub server: String,
    pub code: String,
}

/// The answer of a code that let somebody in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Verified {
    pub username: String,
}

#[cfg(test)]
mod tests {
    use serde_json::{from_str, to_string};

    use super::{JoinCode, Verified, Verify};
    use crate::competitive::Forbidden;

    // The server plugin reads and writes these by hand in C#, so the shape is
    // fixed here.
    #[test]
    fn verify_shape() {
        let verify = Verify {
            server: "Durka".to_owned(),
            code: "abc".to_owned(),
        };
        assert_eq!(
            to_string(&verify).unwrap(),
            r#"{"server":"Durka","code":"abc"}"#
        );
        let verified = Verified {
            username: "vladas".to_owned(),
        };
        assert_eq!(to_string(&verified).unwrap(), r#"{"username":"vladas"}"#);
    }

    // The join plugin reads this in C#, and a backend before competitive
    // servers sends only the code.
    #[test]
    fn join_code_shape() {
        let code = JoinCode {
            code: "abc".to_owned(),
            competitive: true,
            world: Some("-123".to_owned()),
            forbidden: vec![Forbidden {
                item: "IronScrap".to_owned(),
                boss: "The Elder".to_owned(),
                tier: 1,
            }],
            allowed: vec!["Bread".to_owned()],
        };
        assert_eq!(
            to_string(&code).unwrap(),
            r#"{"code":"abc","competitive":true,"world":"-123","forbidden":[{"item":"IronScrap","boss":"The Elder","tier":1}],"allowed":["Bread"]}"#
        );
        let old: JoinCode = from_str(r#"{"code":"abc"}"#).unwrap();
        assert!(!old.competitive);
        assert!(old.forbidden.is_empty());
    }
}
