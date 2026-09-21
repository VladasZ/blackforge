//! Sync saves portable data, but only an explicit Apply changes local files.
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Result, anyhow, bail};
use blackforge_api::setup::{SaveResult, SaveSetup, SavedSetup, Setup};
use blackforge_core::{
    Error,
    cloud::{History, Review, capture, restore, review, snapshot, validate},
    progress::Progress,
};
use hilen::{
    dispatch::{on_main, sleep, spawn},
    login::GoogleLogin,
};
use tokio::sync::Mutex;

use crate::{backend, launcher, social, ui::toast};

static CHECKING: AtomicBool = AtomicBool::new(false);
static SYNC: Mutex<()> = Mutex::const_new(());
static MESSAGE: Mutex<String> = Mutex::const_new(String::new());

#[derive(Clone)]
pub struct Preview {
    token: String,
    account: String,
    revision: i64,
    pub review: Review,
}

impl Preview {
    pub fn message(&self) -> String {
        let conflicts = self.review.conflicts();
        if self.review.changes.is_empty() {
            "Your main setup is saved and up to date.".to_owned()
        } else if conflicts > 0 {
            format!(
                "{} differences, {conflicts} conflicts. Review the choices before applying.",
                self.review.changes.len()
            )
        } else {
            format!(
                "{} differences ready to review. Your local setup stays unchanged until Apply.",
                self.review.changes.len()
            )
        }
    }
}

pub fn init() {
    spawn(async {
        loop {
            sleep(60.0).await;
            schedule();
        }
    });
}

pub fn schedule() {
    if !social::signed_in() || launcher::running() || CHECKING.swap(true, Ordering::SeqCst) {
        return;
    }
    spawn(async {
        let result = check().await;
        let message = match &result {
            Ok(preview) => preview.message(),
            Err(error) => format!("Cloud sync is waiting: {error:#}"),
        };
        let mut last = MESSAGE.lock().await;
        if *last != message {
            last.clone_from(&message);
            if result
                .as_ref()
                .is_ok_and(|preview| !preview.review.changes.is_empty())
            {
                on_main(|| {
                    toast::info("Your cloud setup has changes. Review them on the Mods page.");
                });
            }
            log::info!("{message}");
        }
        CHECKING.store(false, Ordering::SeqCst);
    });
}

pub async fn check() -> Result<Preview> {
    let held = backend::PROFILE_IO.lock().await;
    let sync = SYNC.lock().await;
    if launcher::running() {
        bail!("close the game before syncing its settings");
    }
    let token = GoogleLogin::token().ok_or(Error::NotSignedIn)?;
    let client = social::client()?;
    // Fetch before inspecting the local profile. A fresh machine must not
    // replace the saved setup with its automatically created default.
    let account = client.setup().await?;
    let forge = backend::forge()?;
    let profile = backend::profile(forge, &Progress::silent()).await?;
    let captured = snapshot(&profile).await?;
    let local = captured.setup;
    let history = History::read(forge.data().root(), &account.account).await?;
    let SavedSetup {
        mut revision,
        setup: remote,
    } = account.saved.unwrap_or(SavedSetup {
        revision: 0,
        setup: Setup::default(),
    });
    validate(&remote)?;
    let mut comparison = review(&history.local, &history.cloud, &local, &remote);
    comparison.keep_local_only(&remote, &captured.local_only);
    if let Some(merged) = comparison.merged()
        && validate(&merged).is_ok()
    {
        ensure_account(&token)?;
        if merged != remote || revision == 0 {
            match client
                .save_setup(&SaveSetup {
                    revision,
                    setup: merged.clone(),
                })
                .await?
            {
                SaveResult::Saved(saved) => revision = saved.revision,
                SaveResult::Conflict => bail!("another machine just saved changes; check again"),
            }
        }
        ensure_account(&token)?;
        History {
            local: local.clone(),
            cloud: merged.clone(),
        }
        .save(forge.data().root(), &account.account)
        .await?;
        comparison = review(&local, &merged, &local, &merged);
        comparison.keep_local_only(&merged, &captured.local_only);
    }
    drop(sync);
    drop(held);
    Ok(Preview {
        token,
        account: account.account,
        revision,
        review: comparison,
    })
}

/// Called through `backend::change`, which holds the profile lock throughout.
pub async fn apply(preview: Preview, progress: &Progress) -> Result<()> {
    let held = SYNC.lock().await;
    if launcher::running() {
        bail!("close the game before applying a setup");
    }
    ensure_account(&preview.token)?;
    let forge = backend::forge()?;
    let profile = backend::profile(forge, progress).await?;
    if capture(&profile).await? != preview.review.local {
        bail!("local mods or settings changed; review again");
    }
    let setup = preview
        .review
        .merged()
        .ok_or_else(|| anyhow!("choose a side for every conflict"))?;
    validate(&setup)?;
    let client = social::client()?;
    let account = client.setup().await?;
    if account.account != preview.account
        || account.saved.as_ref().map_or(0, |s| s.revision) != preview.revision
    {
        bail!("the cloud setup changed; review again");
    }
    // Publish the user's decisions with compare-and-swap before installing.
    // If installation fails the cloud still holds the intended setup, while
    // the original local profile and its history remain intact for retry.
    match client
        .save_setup(&SaveSetup {
            revision: preview.revision,
            setup: setup.clone(),
        })
        .await?
    {
        SaveResult::Saved(_) => {}
        SaveResult::Conflict => bail!("another machine saved changes; review again"),
    }
    ensure_account(&preview.token)?;
    restore(forge, &profile, &setup, progress).await?;
    let local = capture(&profile).await?;
    History {
        local,
        cloud: setup,
    }
    .save(forge.data().root(), &preview.account)
    .await?;
    drop(held);
    Ok(())
}

fn ensure_account(token: &str) -> Result<()> {
    if GoogleLogin::token().as_deref() != Some(token) {
        bail!("the signed-in account changed; review again");
    }
    Ok(())
}
