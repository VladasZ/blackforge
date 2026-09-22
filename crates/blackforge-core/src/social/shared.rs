//! What a profile shows to friends: the mods of the lock, each marked when
//! it is only a dependency, and the settings the user changed.

use blackforge_api::{SharedConfig, SharedMod, SharedProfile, SharedSetting};

use crate::{
    config::{self, ConfigFile},
    error::Result,
    profile::Profile,
    social::secret::looks_secret,
};

/// The settings of one file that differ from their default, secrets left
/// out. A setting with no `# Default value:` comment is left out too, nothing
/// says if it was changed.
pub fn changed_settings(config: &ConfigFile) -> Vec<SharedSetting> {
    config
        .settings()
        .into_iter()
        .filter(|setting| {
            setting
                .default
                .as_deref()
                .is_some_and(|default| default != setting.value)
        })
        .filter(|setting| !looks_secret(&setting.key, &setting.value))
        .map(|setting| SharedSetting {
            section: setting.section,
            key: setting.key,
            value: setting.value,
        })
        .collect()
}

/// Everything the server gets about the profile. Equal profiles give equal
/// values, so the caller can compare with the last upload and skip the same
/// one.
pub async fn shared_profile(profile: &Profile) -> Result<SharedProfile> {
    let manifest = profile.manifest().await?;
    let lock = profile.lock().await?;

    let mods = lock
        .packages
        .iter()
        .map(|package| SharedMod {
            id: package.id.to_string(),
            version: package.version.to_string(),
            enabled: manifest.is_enabled(&package.id),
            dependency: !manifest.mods.contains_key(&package.id),
        })
        .collect();

    let mut configs = Vec::new();
    for file in config::list(profile).await? {
        let path = config::find(profile, &file).await?;
        let settings = changed_settings(&config::read(&path).await?);
        if !settings.is_empty() {
            configs.push(SharedConfig { file, settings });
        }
    }
    configs.sort_by(|a, b| a.file.cmp(&b.file));

    Ok(SharedProfile { mods, configs })
}

#[cfg(test)]
mod tests {
    use super::changed_settings;
    use crate::config::ConfigFile;

    fn config() -> ConfigFile {
        ConfigFile::parse(
            &[
                "[General]",
                "",
                "# Setting type: Boolean",
                "# Default value: true",
                "Enabled = true",
                "",
                "# Setting type: Int32",
                "# Default value: 5",
                "Count = 9",
                "",
                "# Setting type: String",
                "# Default value: ",
                "Webhook URL = https://discord.com/api/webhooks/1/abc",
                "",
                "# Setting type: String",
                "# Default value: ",
                "Server Password = hunter2",
                "",
                "Hand Written = 1",
                "",
            ]
            .join("\r\n"),
        )
    }

    #[test]
    fn only_changed_and_harmless_settings_are_shared() {
        let shared = changed_settings(&config());
        assert_eq!(shared.len(), 1);
        assert_eq!(shared[0].section, "General");
        assert_eq!(shared[0].key, "Count");
        assert_eq!(shared[0].value, "9");
    }
}
