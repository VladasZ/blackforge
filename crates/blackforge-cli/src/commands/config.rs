use anyhow::Result;
use blackforge_core::config::{find, list, read, write};

use crate::{Context, cli::ConfigCommand};

pub async fn config(context: &Context, command: ConfigCommand) -> Result<()> {
    let profile = context.profile().await?;
    match command {
        ConfigCommand::List => {
            let names = list(&profile).await?;
            if names.is_empty() {
                println!(
                    "no config files yet, a mod writes its config on the first start of the game"
                );
            }
            for name in names {
                println!("{name}");
            }
        }
        ConfigCommand::Show { file } => {
            let config = read(&find(&profile, &file).await?).await?;
            let mut section = None;
            for setting in config.settings() {
                if section.as_ref() != Some(&setting.section) {
                    println!("[{}]", setting.section);
                    section = Some(setting.section.clone());
                }
                let default = setting
                    .default
                    .map(|default| format!("  (default {default})"))
                    .unwrap_or_default();
                println!("  {} = {}{default}", setting.key, setting.value);
            }
        }
        ConfigCommand::Get { file, section, key } => {
            let setting = read(&find(&profile, &file).await?)
                .await?
                .get(&section, &key)?;
            println!("{}", setting.value);
        }
        ConfigCommand::Set {
            file,
            section,
            key,
            value,
        } => {
            let path = find(&profile, &file).await?;
            let mut config = read(&path).await?;
            let before = config.get(&section, &key)?;
            config.set(&section, &key, &value)?;
            write(&path, &config).await?;
            println!(
                "{}.{}: {} -> {value}",
                before.section, before.key, before.value
            );
        }
    }
    Ok(())
}
