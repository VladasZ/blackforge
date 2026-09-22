//! Reads and edits the `.cfg` files that `BepInEx` mods write.
//!
//! An edit changes one line and leaves every other byte alone, comments and
//! Windows line ends included, because the mod rewrites the file on its next
//! start and users diff these files.

use std::{
    mem::take,
    path::{Path, PathBuf},
};

use tokio::{fs, task::spawn_blocking};

use crate::{
    error::{Error, IoContext, Result},
    ident::PackageId,
    profile::Profile,
    util::{exists, walk_files},
};

/// A shorter shared part matches by chance too often.
const MIN_SHARED: usize = 4;

const TYPE_PREFIX: &str = "# Setting type:";
const DEFAULT_PREFIX: &str = "# Default value:";
const ACCEPTABLE_PREFIX: &str = "# Acceptable value";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Setting {
    pub section: String,
    pub key: String,
    pub value: String,
    pub description: Vec<String>,
    pub setting_type: Option<String>,
    pub default: Option<String>,
    pub acceptable: Option<String>,
    line: usize,
}

impl Setting {
    /// The value as on or off, `None` when the setting is not a boolean.
    /// `BepInEx` names the type in a comment. A file written by hand has no
    /// such comment, there the value alone decides.
    pub fn as_bool(&self) -> Option<bool> {
        let boolean = self
            .setting_type
            .as_deref()
            .is_none_or(|name| name.eq_ignore_ascii_case("Boolean"));
        if !boolean {
            return None;
        }
        if self.value.eq_ignore_ascii_case("true") {
            Some(true)
        } else if self.value.eq_ignore_ascii_case("false") {
            Some(false)
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigFile {
    lines: Vec<String>,
}

impl ConfigFile {
    pub fn parse(text: &str) -> Self {
        // Splitting on `\n` alone keeps a `\r` at the end of each line, so a
        // file with Windows line ends is written back unchanged.
        Self {
            lines: text.split('\n').map(str::to_owned).collect(),
        }
    }

    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    pub fn settings(&self) -> Vec<Setting> {
        let mut settings = Vec::new();
        let mut section = String::new();
        let mut pending = Pending::default();
        for (at, raw) in self.lines.iter().enumerate() {
            let line = raw.trim();
            if let Some(name) = line
                .strip_prefix('[')
                .and_then(|rest| rest.strip_suffix(']'))
            {
                name.trim().clone_into(&mut section);
                pending = Pending::default();
            } else if let Some(text) = line.strip_prefix("##") {
                pending.description.push(text.trim().to_owned());
            } else if line.starts_with('#') {
                pending.read_comment(line);
            } else if let Some((key, value)) = line.split_once('=') {
                let Pending {
                    description,
                    setting_type,
                    default,
                    acceptable,
                } = take(&mut pending);
                settings.push(Setting {
                    section: section.clone(),
                    key: key.trim().to_owned(),
                    value: value.trim().to_owned(),
                    description,
                    setting_type,
                    default,
                    acceptable,
                    line: at,
                });
            } else if line.is_empty() {
                pending = Pending::default();
            }
        }
        settings
    }

    pub fn get(&self, section: &str, key: &str) -> Result<Setting> {
        self.settings()
            .into_iter()
            .find(|setting| {
                setting.section.eq_ignore_ascii_case(section)
                    && setting.key.eq_ignore_ascii_case(key)
            })
            .ok_or_else(|| Error::Invalid(format!("no setting '{key}' in section '{section}'")))
    }

    pub fn set(&mut self, section: &str, key: &str, value: &str) -> Result<()> {
        let setting = self.get(section, key)?;
        let line = &self.lines[setting.line];
        let line_end = if line.ends_with('\r') { "\r" } else { "" };
        self.lines[setting.line] = format!("{} = {value}{line_end}", setting.key);
        Ok(())
    }

    /// Insert a synced setting before the next section, or update it in place.
    pub fn upsert(&mut self, section: &str, key: &str, value: &str) -> Result<()> {
        if self.get(section, key).is_ok() {
            return self.set(section, key, value);
        }
        let mut at = if section.is_empty() { Some(0) } else { None };
        for (index, line) in self.lines.iter().enumerate() {
            if let Some(name) = line
                .trim()
                .strip_prefix('[')
                .and_then(|s| s.strip_suffix(']'))
            {
                if at.is_some() {
                    break;
                }
                if name.trim().eq_ignore_ascii_case(section) {
                    at = Some(index + 1);
                }
            } else if at.is_some() {
                at = Some(index + 1);
            }
        }
        let at = at.unwrap_or_else(|| {
            self.lines.push(format!("\n[{section}]"));
            self.lines.len()
        });
        self.lines.insert(at, format!("{key} = {value}"));
        Ok(())
    }

    pub fn remove(&mut self, section: &str, key: &str) {
        if let Ok(setting) = self.get(section, key) {
            self.lines.remove(setting.line);
        }
    }
}

#[derive(Default)]
struct Pending {
    description: Vec<String>,
    setting_type: Option<String>,
    default: Option<String>,
    acceptable: Option<String>,
}

impl Pending {
    fn read_comment(&mut self, line: &str) {
        if let Some(text) = line.strip_prefix(TYPE_PREFIX) {
            self.setting_type = Some(text.trim().to_owned());
        } else if let Some(text) = line.strip_prefix(DEFAULT_PREFIX) {
            self.default = Some(text.trim().to_owned());
        } else if line.starts_with(ACCEPTABLE_PREFIX) {
            self.acceptable = line.split_once(':').map(|(_, text)| text.trim().to_owned());
        }
    }
}

/// Letters and digits only, in lower case, so `valheim_mod.HudCompass` and
/// `HUDCompass` compare as equal text.
fn plain(text: &str) -> String {
    text.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|letter| letter.to_ascii_lowercase())
        .collect()
}

/// The package a config file most likely belongs to. A mod names its config
/// after its plugin id, `owner.mod_name.cfg` or close to it, or keeps it in a
/// folder with its name, and no file says which package that is, so the
/// names are compared. Every folder on the path and the file name without
/// its extension count. The longest shared name wins, the owner in the name
/// breaks a tie.
pub fn package_of<'a>(
    file: &str,
    packages: impl IntoIterator<Item = &'a PackageId>,
) -> Option<&'a PackageId> {
    let mut parts: Vec<&str> = file.split('/').collect();
    if let Some(name) = parts.pop() {
        parts.push(name.rsplit_once('.').map_or(name, |(stem, _)| stem));
    }
    let parts: Vec<String> = parts.into_iter().map(plain).collect();
    packages
        .into_iter()
        .filter_map(|id| {
            let package = plain(id.name());
            let owner = plain(id.owner());
            parts
                .iter()
                .filter_map(|part| {
                    let shared = if part.contains(&package) {
                        package.len()
                    } else if package.contains(part) {
                        part.len()
                    } else {
                        return None;
                    };
                    (shared >= MIN_SHARED).then(|| (shared, part.contains(&owner)))
                })
                .max()
                .map(|score| (score, id))
        })
        .max_by_key(|(score, _)| *score)
        .map(|(_, id)| id)
}

fn config_dir(profile: &Profile) -> PathBuf {
    profile.dir().join("BepInEx").join("config")
}

/// Names of the `.cfg` files of a profile, relative to `BepInEx/config`.
pub async fn list(profile: &Profile) -> Result<Vec<String>> {
    let dir = config_dir(profile);
    if !exists(&dir).await {
        return Ok(Vec::new());
    }
    let files = spawn_blocking(move || walk_files(&dir)).await??;
    Ok(files
        .into_iter()
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("cfg"))
        })
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .collect())
}

/// Finds one config by its full name, its name without `.cfg`, or a part of
/// the name when only one file has it.
pub async fn find(profile: &Profile, query: &str) -> Result<PathBuf> {
    let names = list(profile).await?;
    let wanted = query.to_lowercase();
    let exact = names.iter().find(|name| {
        let lower = name.to_lowercase();
        lower == wanted || lower.strip_suffix(".cfg") == Some(wanted.as_str())
    });
    let name = if let Some(name) = exact {
        name
    } else {
        let mut partial = names
            .iter()
            .filter(|name| name.to_lowercase().contains(&wanted));
        let first = partial
            .next()
            .ok_or_else(|| Error::Invalid(format!("no config file matches '{query}'")))?;
        if partial.next().is_some() {
            return Err(Error::Invalid(format!(
                "several config files match '{query}', use a longer name"
            )));
        }
        first
    };
    Ok(name
        .split('/')
        .fold(config_dir(profile), |path, part| path.join(part)))
}

pub async fn read(path: &Path) -> Result<ConfigFile> {
    Ok(ConfigFile::parse(&fs::read_to_string(path).await.at(path)?))
}

pub async fn write(path: &Path, config: &ConfigFile) -> Result<()> {
    fs::write(path, config.text()).await.at(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> String {
        [
            "## Settings file was created by plugin Example v1.0.0",
            "## Plugin GUID: owner.example",
            "",
            "[General]",
            "",
            "## Turns the mod on.",
            "## Second line.",
            "# Setting type: Boolean",
            "# Default value: true",
            "Enabled = true",
            "",
            "[Limits]",
            "",
            "## How many.",
            "# Setting type: Int32",
            "# Default value: 5",
            "# Acceptable value range: From 1 to 10",
            "Count = 5",
            "",
        ]
        .join("\r\n")
    }

    #[test]
    fn reads_settings_with_their_comments() -> Result<()> {
        let config = ConfigFile::parse(&sample());
        let settings = config.settings();
        assert_eq!(settings.len(), 2);
        let enabled = config.get("general", "enabled")?;
        assert_eq!(enabled.value, "true");
        assert_eq!(enabled.description, ["Turns the mod on.", "Second line."]);
        assert_eq!(enabled.setting_type.as_deref(), Some("Boolean"));
        let count = config.get("Limits", "Count")?;
        assert_eq!(count.default.as_deref(), Some("5"));
        assert_eq!(count.acceptable.as_deref(), Some("From 1 to 10"));
        Ok(())
    }

    #[test]
    fn a_file_matches_its_package_by_name_or_folder() -> Result<()> {
        let packages: Vec<PackageId> = [
            "Neobotics-HUDCompass",
            "ZenDragon-Zen_ModLib",
            "ZenDragon-ZenItemStands",
            "denikson-BepInExPack_Valheim",
        ]
        .iter()
        .map(|id| id.parse())
        .collect::<Result<_>>()?;
        let of = |file: &str| package_of(file, &packages).map(ToString::to_string);
        assert_eq!(
            of("neobotics.valheim_mod.hudcompass.cfg").as_deref(),
            Some("Neobotics-HUDCompass")
        );
        assert_eq!(
            of("Neobotics/HUDCompass/compass.png").as_deref(),
            Some("Neobotics-HUDCompass")
        );
        assert_eq!(
            of("ZenDragon.Zen.ModLib.cfg").as_deref(),
            Some("ZenDragon-Zen_ModLib")
        );
        assert_eq!(
            of("ZenDragon.ZenItemStands.cfg").as_deref(),
            Some("ZenDragon-ZenItemStands")
        );
        assert_eq!(
            of("BepInEx.cfg").as_deref(),
            Some("denikson-BepInExPack_Valheim")
        );
        assert_eq!(of("QuickStackStore_player_2197692124.dat"), None);
        Ok(())
    }

    #[test]
    fn an_edit_changes_one_line_only() -> Result<()> {
        let mut config = ConfigFile::parse(&sample());
        config.set("Limits", "Count", "9")?;
        assert_eq!(
            config.text(),
            sample().replace("Count = 5\r\n", "Count = 9\r\n")
        );
        assert!(config.set("Limits", "Missing", "1").is_err());
        Ok(())
    }
}
