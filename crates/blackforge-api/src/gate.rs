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

/// How long a join code lives. The game uses it right after the click.
pub const CODE_SECONDS: i64 = 120;

/// One row of `GET /api/members`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Member {
    pub username: String,
}

/// The body of `POST /api/members`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddMember {
    pub username: String,
}

/// The answer of `POST /api/servers/{id}/join`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JoinCode {
    pub code: String,
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
    use serde_json::to_string;

    use super::{Verified, Verify};

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
}
