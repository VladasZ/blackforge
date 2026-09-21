//! Starts the game with the mods of the active profile.

use std::{
    path::PathBuf,
    sync::{
        LazyLock,
        atomic::{AtomicBool, Ordering},
    },
};

use blackforge_core::{
    Error,
    launch::{LaunchInput, Os, doorstop_major, inherited_env, plan, spawn as spawn_game},
};
use hilen::{
    dispatch::{on_main, spawn},
    filesystem::Paths,
    store::OnDisk,
    ui::Question,
};
use tokio::process::Child;

use crate::{backend, social, ui::toast};

static RUNNING: AtomicBool = AtomicBool::new(false);

static GAME_ARGS: LazyLock<OnDisk<String>> = LazyLock::new(|| OnDisk::new("gui-game-args.json"));

/// Extra arguments for the game itself, split on spaces at launch.
pub fn game_args() -> String {
    GAME_ARGS.get().unwrap_or_default()
}

pub fn set_game_args(args: &str) {
    GAME_ARGS.set(args.to_owned());
}

enum Started {
    Running {
        child: Box<Child>,
        label: String,
    },
    /// Steam does not know the game, the user has to point at the folder.
    NotFound {
        game: String,
    },
}

pub fn run_game() {
    start(None);
}

/// A folder given here wins over the Steam lookup and is remembered.
pub fn run_game_from(game_dir: PathBuf) {
    start(Some(game_dir));
}

fn start(game_dir: Option<PathBuf>) {
    if RUNNING.swap(true, Ordering::SeqCst) {
        toast::info("the game is already running");
        return;
    }
    let game_args: Vec<String> = game_args().split_whitespace().map(str::to_owned).collect();

    backend::load(
        "starting the game",
        move |forge, progress| async move {
            let profile = backend::profile(forge, &progress).await?;
            let manifest = profile.manifest().await?;
            let game = forge.game(&manifest).await?;
            let install = match forge.locate_game(&game, game_dir).await {
                Ok(install) => install,
                Err(Error::GameNotInstalled(game)) => return Ok(Started::NotFound { game }),
                Err(error) => return Err(error.into()),
            };
            let doorstop_major = doorstop_major(profile.dir()).await;
            let plan = plan(&LaunchInput {
                os: Os::current()?,
                game: &game,
                install: &install,
                profile_dir: profile.dir(),
                doorstop_major,
                game_args: &game_args,
                keep_achievements: forge.keep_achievements().await?,
                inherited: &inherited_env,
            })?;
            let child = spawn_game(&plan, profile.dir()).await?;
            let label = format!("{} {}", game.display_name, manifest.target);
            Ok(Started::Running {
                child: Box::new(child),
                label,
            })
        },
        |result| match result {
            Ok(Started::Running { child, label }) => {
                toast::success(format!("started {label}"));
                social::game_started();
                wait_for_exit(*child);
            }
            Ok(Started::NotFound { game }) => {
                RUNNING.store(false, Ordering::SeqCst);
                ask_for_folder(&game);
            }
            Err(error) => {
                RUNNING.store(false, Ordering::SeqCst);
                toast::failure(&error);
            }
        },
    );
}

fn wait_for_exit(mut child: Child) {
    spawn(async move {
        let status = child.wait().await;
        on_main(move || {
            RUNNING.store(false, Ordering::SeqCst);
            social::game_exited();
            match status {
                Ok(status) if status.success() => log::info!("the game exited with {status}"),
                Ok(status) => toast::error(format!("the game exited with {status}")),
                Err(error) => toast::error(format!("cannot wait for the game: {error}")),
            }
        });
    });
}

fn ask_for_folder(game: &str) {
    Question::ask(format!(
        "{game} was not found through Steam. Pick the game folder by hand?"
    ))
    .on_yes(pick_folder_and_run);
}

fn pick_folder_and_run() {
    spawn(async {
        let picked = Paths::pick_folder().await;
        on_main(move || {
            if let Some(game_dir) = picked {
                run_game_from(game_dir);
            }
        });
    });
}
