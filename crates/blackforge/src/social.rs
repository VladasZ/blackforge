//! The part of the friends feature that has no view: who is signed in, the
//! uploads of the profile, and the in game reports. All of it does nothing
//! while nobody is signed in, the app works without an account.

use std::sync::{
    LazyLock,
    atomic::{AtomicU64, Ordering},
};

use anyhow::Result;
use blackforge_core::{
    Error,
    progress::Progress,
    social::{
        client::{SERVER, SocialClient},
        shared::shared_profile,
    },
};
use hilen::{
    dispatch::{sleep, spawn},
    login::GoogleLogin,
    store::OnDisk,
};
use serde_json::to_string;

use crate::backend;

/// The app says it is still in game this often. The server reads two missed
/// reports as not in game.
const REPORT_SECONDS: f32 = 60.0;

/// The JSON of the last profile the server took, so an equal one is not sent
/// again. It is a file because most starts of the app change nothing.
static LAST_UPLOAD: LazyLock<OnDisk<String>> =
    LazyLock::new(|| OnDisk::new("gui-last-shared-profile.json"));

/// Every game start and exit moves it on, which ends the report loop of the
/// run before.
static GAME_RUN: AtomicU64 = AtomicU64::new(0);

pub fn init() {
    GoogleLogin::set_server(SERVER);
}

pub fn signed_in() -> bool {
    GoogleLogin::token().is_some()
}

pub fn client() -> Result<SocialClient> {
    let token = GoogleLogin::token().ok_or(Error::NotSignedIn)?;
    Ok(SocialClient::new(token)?)
}

/// After a sign out the next account starts clean, its first upload must not
/// be skipped because the account before sent the same mods.
pub fn forget_upload() {
    if LAST_UPLOAD.get().is_some() {
        LAST_UPLOAD.reset();
    }
}

/// Sends the mods and the changed settings when they differ from the last
/// upload. Runs by itself at launch, after every change of the profile, after
/// a config edit and after the game exits. A failure only goes to the log,
/// the user did not ask for this and the next run tries again.
pub fn share_profile() {
    if !signed_in() {
        return;
    }

    spawn(async {
        if let Err(error) = upload_if_changed().await {
            log::warn!("the profile was not shared: {error:#}");
        }
    });
}

async fn upload_if_changed() -> Result<()> {
    let forge = backend::forge()?;
    let profile = backend::profile(forge, &Progress::silent()).await?;

    let shared = shared_profile(&profile).await?;
    let json = to_string(&shared)?;
    if LAST_UPLOAD.get().as_deref() == Some(json.as_str()) {
        return Ok(());
    }

    client()?.upload_profile(&shared).await?;
    LAST_UPLOAD.set(json);
    log::info!(
        "shared {} mods and {} config files",
        shared.mods.len(),
        shared.configs.len()
    );
    Ok(())
}

/// Tells the friends the game runs, now and once a minute until it exits.
pub fn game_started() {
    let run = GAME_RUN.fetch_add(1, Ordering::SeqCst) + 1;
    if !signed_in() {
        return;
    }

    spawn(async move {
        while GAME_RUN.load(Ordering::SeqCst) == run {
            report(true).await;
            sleep(REPORT_SECONDS).await;
        }
    });
}

pub fn game_exited() {
    GAME_RUN.fetch_add(1, Ordering::SeqCst);
    if !signed_in() {
        return;
    }

    spawn(report(false));
    // The game writes its config files while it runs.
    share_profile();
}

async fn report(in_game: bool) {
    let sent = match client() {
        Ok(client) => client.set_status(in_game).await.map_err(Into::into),
        Err(error) => Err(error),
    };
    if let Err(error) = sent {
        log::warn!("the in game status was not sent: {error:#}");
    }
}
