use std::path::PathBuf;

use anyhow::{Result, bail};
use blackforge_core::{
    doctor::Status,
    launch::{LaunchInput, Os, doorstop_major, inherited_env, plan, spawn},
};

use crate::{
    Context,
    ui::{hint, print_table},
};

pub async fn run(context: &Context, game_dir: Option<PathBuf>, game_args: &[String]) -> Result<()> {
    let profile = context.profile().await?;
    let manifest = profile.manifest().await?;
    let game = context.forge.game(&manifest).await?;
    let install = context.forge.locate_game(&game, game_dir).await?;

    let plan = plan(&LaunchInput {
        os: Os::current()?,
        game: &game,
        install: &install,
        profile_dir: profile.dir(),
        doorstop_major: doorstop_major(profile.dir()).await,
        game_args,
        inherited: &inherited_env,
    })?;
    println!(
        "starting {} {} with profile {}",
        game.display_name,
        manifest.target,
        profile.name()
    );
    let status = spawn(&plan, profile.dir()).await?.wait().await?;
    if !status.success() {
        bail!("the game exited with {status}");
    }
    Ok(())
}

pub async fn doctor(context: &Context) -> Result<()> {
    let checks = context.forge.doctor().await?;
    let mut rows = Vec::new();
    for check in &checks {
        let status = match check.status {
            Status::Ok => "ok",
            Status::Warning => "warning",
            Status::Problem => "problem",
        };
        let detail = match check.fix {
            Some(fix) => format!("{}, {}", check.detail, hint(fix)),
            None => check.detail.clone(),
        };
        rows.push(vec![status.to_owned(), check.name.to_owned(), detail]);
    }
    print_table(&rows);
    if checks.iter().any(|check| check.status == Status::Problem) {
        bail!("some checks found a problem");
    }
    Ok(())
}
