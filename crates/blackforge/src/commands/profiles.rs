use std::path::{Path, PathBuf};

use anyhow::Result;
use blackforge_core::game::{Target, VALHEIM};
use tokio::fs;

use crate::{
    Context,
    cli::{NewProfile, ProfileCommand},
    commands::mods::sync,
    ui::{Ui, print_sync_report},
};

fn target(server: bool) -> Target {
    if server {
        Target::Server
    } else {
        Target::Client
    }
}

async fn create(context: &Context, new: &NewProfile) -> Result<()> {
    let ui = Ui::start();
    let profile = context
        .forge
        .create_profile(&new.name, VALHEIM, target(new.server), &ui.progress)
        .await;
    ui.finish().await?;
    let profile = profile?;
    println!(
        "created profile {} in {}",
        profile.name(),
        profile.dir().display()
    );
    Ok(())
}

pub async fn profile(context: &Context, command: ProfileCommand) -> Result<()> {
    let store = context.forge.store();
    match command {
        ProfileCommand::List => {
            let active = store.state().await?.active_profile;
            let names = store.list().await?;
            if names.is_empty() {
                println!(
                    "no profiles yet, the first 'blackforge add <mod>' or 'blackforge run' creates one"
                );
            }
            for name in names {
                let mark = if active.as_deref() == Some(name.as_str()) {
                    "*"
                } else {
                    " "
                };
                println!("{mark} {name}");
            }
        }
        ProfileCommand::Create(new) => create(context, &new).await?,
        ProfileCommand::Switch { name } => {
            let profile = store.switch(&name).await?;
            println!("{} is now the active profile", profile.name());
        }
        ProfileCommand::Delete { name } => {
            store.delete(&name).await?;
            println!("deleted profile {name}");
        }
    }
    Ok(())
}

pub async fn import(
    context: &Context,
    source: &str,
    name: Option<&str>,
    server: bool,
) -> Result<()> {
    let ui = Ui::start();
    let profile = context
        .forge
        .import_r2(source, name, target(server), &ui.progress)
        .await;
    ui.finish().await?;
    let profile = profile?;
    let lock = profile.lock().await?;
    println!(
        "imported profile {} with {} mods",
        profile.name(),
        lock.packages.len()
    );

    let ui = Ui::start();
    let report = context.forge.sync(&profile, &ui.progress).await;
    ui.finish().await?;
    print_sync_report(&report?);
    println!(
        "switch to it with 'blackforge profile switch {}'",
        profile.name()
    );
    Ok(())
}

pub async fn export(context: &Context, file: Option<PathBuf>) -> Result<()> {
    let profile = context.profile().await?;
    if let Some(path) = file {
        let zip_bytes = context.forge.export_r2_bytes(&profile).await?;
        fs::write(&path, zip_bytes).await?;
        println!("wrote {}", path.display());
    } else {
        let code = context.forge.export_r2_code(&profile).await?;
        println!("{code}");
        println!("the code works in r2modman, Gale and blackforge, and it expires within hours");
    }
    Ok(())
}

pub async fn deploy(context: &Context, path: &Path, overwrite_configs: bool) -> Result<()> {
    let profile = context.profile().await?;
    // The lock decides what is deployed, so the profile itself is synced
    // first and a broken lock shows up here and not on the server.
    sync(context).await?;
    let ui = Ui::start();
    let report = context
        .forge
        .deploy(&profile, path, overwrite_configs, &ui.progress)
        .await;
    ui.finish().await?;
    let report = report?;
    println!("deployed to {}", path.display());
    print_sync_report(&report.sync);
    println!(
        "{} configs written, {} kept as they are at the target",
        report.configs_written.len(),
        report.configs_kept.len()
    );
    Ok(())
}
