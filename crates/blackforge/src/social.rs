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
    dispatch::{on_main, sleep, spawn},
    login::GoogleLogin,
    store::OnDisk,
};
use serde_json::to_string;

use crate::{
    backend,
    ui::{nav_item::Badge, page::Page, sidebar},
};

/// The app says it is still in game this often. The server reads two missed
/// reports as not in game.
const REPORT_SECONDS: f32 = 60.0;
/// Friend requests are asked for this often while the app runs.
const REQUESTS_SECONDS: f32 = 300.0;

/// The JSON of the last profile the server took, so an equal one is not sent
/// again. It is a file because most starts of the app change nothing.
static LAST_UPLOAD: LazyLock<OnDisk<String>> =
    LazyLock::new(|| OnDisk::new("gui-last-shared-profile.json"));

/// Every game start and exit moves it on, which ends the report loop of the
/// run before.
static GAME_RUN: AtomicU64 = AtomicU64::new(0);

pub fn init() {
    GoogleLogin::set_server(SERVER);
    crate::cloud::init();
}

pub fn signed_in() -> bool {
    GoogleLogin::token().is_some()
}

pub fn client() -> Result<SocialClient> {
    let token = GoogleLogin::token().ok_or(Error::NotSignedIn)?;
    Ok(SocialClient::new(token)?)
}

/// After a sign out the next account starts clean, its first upload must not
/// be skipped because the account before sent the same mods, and the badge
/// of friend requests goes away.
pub fn signed_out() {
    if LAST_UPLOAD.get().is_some() {
        LAST_UPLOAD.reset();
    }
    show_requests(0);
}

/// Sends the mods and the changed settings when they differ from the last
/// upload. Runs by itself at launch, after every change of the profile, after
/// a config edit and after the game exits. A failure only goes to the log,
/// the user did not ask for this and the next run tries again.
pub fn share_profile() {
    crate::cloud::schedule();
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

/// Asks for friend requests now and then while signed in, so the sidebar
/// counts the ones that wait also while the Friends page is closed. Runs for
/// the whole session, a signed out round only clears the badge.
pub fn watch_requests() {
    spawn(async {
        loop {
            let waiting = if signed_in() {
                match client() {
                    Ok(client) => match client.friends().await {
                        Ok(friends) => Some(friends.incoming.len()),
                        Err(error) => {
                            log::warn!("the friend requests did not load: {error:#}");
                            None
                        }
                    },
                    Err(error) => {
                        log::warn!("no client for the friend requests: {error:#}");
                        None
                    }
                }
            } else {
                Some(0)
            };
            if let Some(waiting) = waiting {
                on_main(move || show_requests(waiting));
            }
            sleep(REQUESTS_SECONDS).await;
        }
    });
}

/// The count of friend requests that wait for me, on the sidebar entry.
pub fn show_requests(waiting: usize) {
    sidebar::set_badge(Page::Friends, Badge::Count(waiting));
}
