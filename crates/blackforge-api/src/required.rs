//! `GET /api/required`, the mods every profile of a game must have. The list
//! is kept by hand on the server, like the broken one, and anybody may read
//! it. Each mod is held at one exact version, the one the game servers run.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The server name of the pin a required mod gets in a profile. The Mods page
/// and the core know a required mod by it.
pub const REQUIRED_PIN_SERVER: &str = "Blackforge";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequiredList {
    /// By the game label of the Thunderstore schema, `valheim` for example.
    pub games: BTreeMap<String, Vec<RequiredMod>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequiredMod {
    /// `Owner-Name`.
    pub package: String,
    /// The exact version, `1.1.8`.
    pub version: String,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::{from_str, to_string};

    use super::{RequiredList, RequiredMod};

    #[test]
    fn required_list_shape() {
        let list = RequiredList {
            games: BTreeMap::from([(
                "valheim".to_owned(),
                vec![RequiredMod {
                    package: "Smoothbrain-Sailing".to_owned(),
                    version: "1.1.8".to_owned(),
                }],
            )]),
        };
        let json = to_string(&list).unwrap();
        assert_eq!(
            json,
            r#"{"games":{"valheim":[{"package":"Smoothbrain-Sailing","version":"1.1.8"}]}}"#
        );
        assert_eq!(from_str::<RequiredList>(&json).unwrap(), list);
    }
}
