mod cli;
mod commands;
mod ui;

use std::process::ExitCode;

use anyhow::Result;
use blackforge_core::{Error, forge::Forge, profile::Profile};
use clap::Parser;

use crate::{
    cli::{Cli, Command},
    ui::Ui,
};

/// What every command gets: the core and the profile to work on.
pub struct Context {
    pub forge: Forge,
    chosen: Option<String>,
}

impl Context {
    /// The profile named with `--profile`, the active one otherwise. On a fresh
    /// machine the default profile is created here, so no setup step exists.
    pub async fn profile(&self) -> Result<Profile> {
        if let Some(name) = &self.chosen {
            return Ok(self.forge.store().get(name).await?);
        }
        let ui = Ui::start();
        let found = self.forge.active_or_default(&ui.progress).await;
        ui.finish().await?;
        let (profile, created) = found?;
        if created {
            println!(
                "created profile {} in {}",
                profile.name(),
                profile.dir().display()
            );
        }
        Ok(profile)
    }

    /// Like `profile`, but it never creates one. For commands that only read.
    pub async fn existing_profile(&self) -> Result<Option<Profile>> {
        match &self.chosen {
            Some(name) => Ok(Some(self.forge.store().get(name).await?)),
            None => match self.forge.store().active().await {
                Ok(profile) => Ok(Some(profile)),
                Err(Error::NoActiveProfile) => Ok(None),
                Err(error) => Err(error.into()),
            },
        }
    }
}

async fn run(cli: Cli) -> Result<()> {
    let context = Context {
        forge: Forge::open()?,
        chosen: cli.profile,
    };
    match cli.command {
        Command::Add { mods } => commands::mods::add(&context, &mods).await,
        Command::Remove { mods } => commands::mods::remove(&context, &mods).await,
        Command::Sync => commands::mods::sync(&context).await,
        Command::Update { mods } => commands::mods::update(&context, &mods).await,
        Command::Outdated => commands::mods::outdated(&context).await,
        Command::Run {
            game_dir,
            game_args,
        } => commands::game::run(&context, game_dir, &game_args).await,
        Command::Search { words, limit } => {
            commands::mods::search(&context, &words.join(" "), limit).await
        }
        Command::Info { name } => commands::mods::info(&context, &name).await,
        Command::List => commands::mods::list(&context).await,
        Command::Enable { name } => commands::mods::set_enabled(&context, &name, true).await,
        Command::Disable { name } => commands::mods::set_enabled(&context, &name, false).await,
        Command::Profile(command) => commands::profiles::profile(&context, command).await,
        Command::Import {
            source,
            name,
            server,
        } => commands::profiles::import(&context, &source, name.as_deref(), server).await,
        Command::Export { file } => commands::profiles::export(&context, file).await,
        Command::Deploy {
            path,
            overwrite_configs,
        } => commands::profiles::deploy(&context, &path, overwrite_configs).await,
        Command::Doctor => commands::game::doctor(&context).await,
        Command::Config(command) => commands::config::config(&context, command).await,
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    match run(Cli::parse()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}
