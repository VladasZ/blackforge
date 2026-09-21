//! The merge behind the "copy config" dialog. One row per setting where my
//! value and the friend's differ, the user picks a side per row, and only the
//! rows set to the friend's side are written.

use blackforge_api::SharedSetting;

use crate::{
    config::{ConfigFile, Setting},
    error::Result,
    social::secret::looks_secret,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    Pending,
    Mine,
    Friend,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickerRow {
    pub section: String,
    pub key: String,
    pub mine: String,
    pub friend: String,
    /// The friend never touched this setting, their side shows its default.
    pub friend_has_default: bool,
    pub pick: Pick,
}

/// `friend` holds only what the friend changed, so a setting missing there is
/// at its default on their side, and my own file knows that default.
///
/// The friend's value starts picked where the friend changed the setting,
/// mine where only I did, so a plain apply never loses anything of mine.
pub fn rows(mine: &ConfigFile, friend: &[SharedSetting]) -> Vec<PickerRow> {
    mine.settings()
        .into_iter()
        .filter_map(|setting| {
            let theirs = friend.iter().find(|theirs| same_setting(theirs, &setting));
            match theirs {
                Some(theirs) => changed_by_friend(setting, theirs),
                None => changed_by_me_only(setting),
            }
        })
        .collect()
}

fn same_setting(theirs: &SharedSetting, mine: &Setting) -> bool {
    theirs.section.eq_ignore_ascii_case(&mine.section) && theirs.key.eq_ignore_ascii_case(&mine.key)
}

fn changed_by_friend(mine: Setting, theirs: &SharedSetting) -> Option<PickerRow> {
    (mine.value != theirs.value).then(|| PickerRow {
        section: mine.section,
        key: mine.key,
        mine: mine.value,
        friend: theirs.value.clone(),
        friend_has_default: false,
        pick: Pick::Friend,
    })
}

fn changed_by_me_only(mine: Setting) -> Option<PickerRow> {
    let default = mine.default?;
    if default == mine.value {
        return None;
    }
    // A secret of the friend never came over, so a missing entry does not
    // mean default here. Offering to reset it would wipe my own secret.
    if looks_secret(&mine.key, &mine.value) {
        return None;
    }

    Some(PickerRow {
        section: mine.section,
        key: mine.key,
        mine: mine.value,
        friend: default,
        friend_has_default: true,
        pick: Pick::Mine,
    })
}

/// Writes the rows set to the friend's side, each one a one line edit of the
/// file. Returns how many settings changed.
pub fn apply(config: &mut ConfigFile, rows: &[PickerRow]) -> Result<usize> {
    let picked = rows.iter().filter(|row| row.pick == Pick::Friend);
    let mut changed = 0;
    for row in picked {
        config.set(&row.section, &row.key, &row.friend)?;
        changed += 1;
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use blackforge_api::SharedSetting;

    use super::{Pick, apply, rows};
    use crate::{config::ConfigFile, error::Result};

    fn mine() -> ConfigFile {
        ConfigFile::parse(
            &[
                "[General]",
                "",
                "# Default value: 5",
                "Count = 5",
                "",
                "# Default value: 1",
                "Speed = 3",
                "",
                "# Default value: F1",
                "Hotkey = F7",
                "",
                "# Default value: true",
                "Same = false",
                "",
                "# Default value: ",
                "Server Password = hunter2",
                "",
            ]
            .join("\r\n"),
        )
    }

    fn setting(key: &str, value: &str) -> SharedSetting {
        SharedSetting {
            section: "general".to_owned(),
            key: key.to_owned(),
            value: value.to_owned(),
        }
    }

    fn friend() -> Vec<SharedSetting> {
        vec![
            setting("Count", "9"),
            setting("Speed", "2"),
            setting("Same", "false"),
            setting("Not In My File", "1"),
        ]
    }

    #[test]
    fn one_row_per_difference_in_all_three_cases() {
        let rows = rows(&mine(), &friend());
        let keys: Vec<&str> = rows.iter().map(|row| row.key.as_str()).collect();
        // `Same` is equal, `Not In My File` has no place to go, and the
        // password never shows.
        assert_eq!(keys, ["Count", "Speed", "Hotkey"]);

        // Only the friend changed it.
        assert_eq!((rows[0].mine.as_str(), rows[0].friend.as_str()), ("5", "9"));
        assert_eq!(rows[0].pick, Pick::Friend);
        // Both changed it.
        assert_eq!((rows[1].mine.as_str(), rows[1].friend.as_str()), ("3", "2"));
        assert_eq!(rows[1].pick, Pick::Friend);
        // Only I changed it, their side is the default and mine stays picked.
        assert_eq!(
            (rows[2].mine.as_str(), rows[2].friend.as_str()),
            ("F7", "F1")
        );
        assert!(rows[2].friend_has_default);
        assert_eq!(rows[2].pick, Pick::Mine);
    }

    #[test]
    fn a_plain_apply_takes_the_friends_changes_and_keeps_mine() -> Result<()> {
        let mut config = mine();
        let rows = rows(&config, &friend());

        assert_eq!(apply(&mut config, &rows)?, 2);
        assert_eq!(config.get("General", "Count")?.value, "9");
        assert_eq!(config.get("General", "Speed")?.value, "2");
        assert_eq!(config.get("General", "Hotkey")?.value, "F7");
        assert_eq!(config.get("General", "Server Password")?.value, "hunter2");
        Ok(())
    }

    #[test]
    fn a_row_flipped_to_the_friend_resets_to_the_default() -> Result<()> {
        let mut config = mine();
        let mut rows = rows(&config, &friend());
        rows[0].pick = Pick::Mine;
        rows[2].pick = Pick::Friend;

        assert_eq!(apply(&mut config, &rows)?, 2);
        assert_eq!(config.get("General", "Count")?.value, "5");
        assert_eq!(config.get("General", "Hotkey")?.value, "F1");
        Ok(())
    }
}
