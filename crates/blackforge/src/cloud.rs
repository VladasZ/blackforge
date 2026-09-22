//! Sync runs by itself. It installs what the cloud holds first, then uploads
//! what this machine changed. Only a real conflict asks the user.
use std::sync::{
    Mutex as StdMutex, PoisonError,
    atomic::{AtomicBool, Ordering},
};

use anyhow::{Result, bail};
use blackforge_api::setup::{HistoryRow, Restore, Revision, Save, Saved, Setup, Summary};
use blackforge_core::{
    Error,
    cloud::{Baseline, Step, capture, plan, restore, snapshot},
    forge::Forge,
    profile::Profile,
    progress::Progress,
};
use chrono::Utc;
use gethostname::gethostname;
use hilen::{
    dispatch::{on_main, sleep, spawn},
    login::GoogleLogin,
};

use crate::{
    backend, launcher, social,
    ui::{
        conflict_dialog::{self, ConflictInput},
        sync_panel, time, toast,
    },
};

const CHECK_SECONDS: f32 = 60.0;
/// Another machine can save between the read of the head and the upload.
const ATTEMPTS: usize = 3;

static QUEUED: AtomicBool = AtomicBool::new(false);
/// The conflict dialog is open, a second one must not stack on it.
static ASKING: AtomicBool = AtomicBool::new(false);
static STATUS: StdMutex<Status> = StdMutex::new(Status::Idle);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Idle,
    Syncing,
    Installing,
    /// Seconds since the Unix epoch.
    Synced(i64),
    Offline,
    Waiting,
    Failed(String),
}

impl Status {
    pub fn text(&self) -> String {
        match self {
            Self::Idle => String::new(),
            Self::Syncing => "Syncing".to_owned(),
            Self::Installing => "Installing the setup from the cloud".to_owned(),
            Self::Synced(at) => format!("Synced {}", time::ago(*at)),
            Self::Offline => "Offline, will retry".to_owned(),
            Self::Waiting => {
                "Waiting for your choice between this machine and the cloud".to_owned()
            }
            Self::Failed(reason) => format!("Not synced: {reason}. Will retry"),
        }
    }
}

pub fn status() -> Status {
    STATUS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

fn set_status(status: Status) {
    *STATUS.lock().unwrap_or_else(PoisonError::into_inner) = status;
    on_main(sync_panel::refresh);
}

/// Both sides changed the same thing. Nothing moves until the user picks.
#[derive(Clone)]
pub struct Pending {
    token: String,
    account: String,
    head: Revision,
    /// The profile as it was read, a change under the open dialog voids it.
    captured: Setup,
    /// What picking this machine uploads.
    local: Setup,
}

enum Outcome {
    Done,
    /// Another machine saved in between, the next read sees its head.
    Again,
    Conflict(Box<Pending>),
}

pub enum Launch {
    Ready,
    Conflict(Box<Pending>),
}

pub fn init() {
    spawn(async {
        loop {
            sleep(CHECK_SECONDS).await;
            // The age in the status line grows also while nothing syncs.
            on_main(sync_panel::refresh);
            schedule();
        }
    });
}

pub fn schedule() {
    if !social::signed_in() {
        set_status(Status::Idle);
        return;
    }
    if launcher::running()
        || ASKING.load(Ordering::SeqCst)
        || backend::busy()
        || QUEUED.swap(true, Ordering::SeqCst)
    {
        return;
    }
    spawn(async {
        let result = sync(None).await;
        QUEUED.store(false, Ordering::SeqCst);
        match result {
            Ok(None) => {}
            Ok(Some(pending)) => on_main(move || ask(*pending, |_| {})),
            Err(error) => {
                if matches!(status(), Status::Failed(_)) {
                    on_main(move || toast::failure(&error));
                }
            }
        }
    });
}

/// The launcher calls this with the game marked as running already. A sync
/// that cannot finish never blocks the game, it starts on the local setup.
pub async fn before_launch(progress: &Progress) -> Launch {
    if !social::signed_in() {
        return Launch::Ready;
    }
    match sync(Some(progress)).await {
        Ok(Some(pending)) => Launch::Conflict(pending),
        Ok(None) => Launch::Ready,
        Err(error) => {
            log::warn!("the game starts on the local setup: {error:#}");
            Launch::Ready
        }
    }
}

/// `progress` is given by the launcher. Without it the install step of a
/// background run makes its own entry in the status bar.
async fn sync(progress: Option<&Progress>) -> Result<Option<Box<Pending>>> {
    let before = status();
    set_status(Status::Syncing);
    let mut result = Ok(Outcome::Again);
    for _ in 0..ATTEMPTS {
        result = attempt(progress).await;
        if !matches!(result, Ok(Outcome::Again)) {
            break;
        }
    }
    let (status, result) = match result {
        Ok(Outcome::Done) => (Status::Synced(Utc::now().timestamp()), Ok(None)),
        Ok(Outcome::Conflict(pending)) => (Status::Waiting, Ok(Some(pending))),
        Ok(Outcome::Again) => (
            Status::Failed("other machines keep saving changes".to_owned()),
            Ok(None),
        ),
        Err(error) if matches!(error.downcast_ref(), Some(Error::Http(_))) => {
            (Status::Offline, Err(error))
        }
        Err(error) => (Status::Failed(format!("{error:#}")), Err(error)),
    };
    // The same failure once a minute is one failure, the toast shows it once.
    let repeated = status == before;
    set_status(status);
    match result {
        Err(error) if repeated => {
            log::warn!("sync failed again: {error:#}");
            Ok(None)
        }
        other => other,
    }
}

async fn attempt(progress: Option<&Progress>) -> Result<Outcome> {
    let token = GoogleLogin::token().ok_or(Error::NotSignedIn)?;
    let client = social::client()?;
    // The head is read under the lock. A head from before another sync or a
    // settled conflict looks like a change of the cloud, and its install
    // would put an old setup over the new one.
    let profile_io = backend::PROFILE_IO.lock().await;
    if progress.is_none() && launcher::running() {
        bail!("the game is running");
    }
    let account = client.sync_head().await?;
    let forge = backend::forge()?;
    let silent = Progress::silent();
    let profile = backend::profile(forge, progress.unwrap_or(&silent)).await?;
    let captured = snapshot(&profile).await?;
    let root = forge.data().root();
    let baseline = Baseline::read(root, &account.account).await?;
    let game = forge.game(&profile.manifest().await?).await?;
    let loaders: Vec<_> = game
        .loader_packages
        .iter()
        .map(|loader| loader.id.to_string())
        .collect();
    let step = plan(
        baseline.as_ref().map(|baseline| &baseline.setup),
        &captured,
        account.head.as_ref().map(|head| &head.setup),
        &loaders,
    )?;
    let head = account.head;
    let agreed = match step {
        Step::Settled => head.map(|head| head.setup),
        Step::Conflict { local } => {
            let Some(head) = head else {
                bail!("a conflict needs a cloud setup");
            };
            return Ok(Outcome::Conflict(Box::new(Pending {
                token,
                account: account.account,
                head,
                captured: captured.setup,
                local,
            })));
        }
        Step::Merge {
            merged,
            install: installs,
            upload,
        } => {
            if installs {
                set_status(Status::Installing);
                install(forge, &profile, &merged, progress).await?;
                ensure_account(&token)?;
                // The cloud side is on disk now. With it as the baseline a
                // failed upload comes back as a plain local change.
                if let Some(head) = &head {
                    Baseline {
                        setup: head.setup.clone(),
                    }
                    .save(root, &account.account)
                    .await?;
                }
                let summary = Summary::between(&captured.setup, &merged);
                on_main(move || {
                    toast::success(format!("Installed from the cloud: {summary}"));
                    sync_panel::applied();
                });
            }
            if upload {
                ensure_account(&token)?;
                let saved = client
                    .sync_save(&Save {
                        base: head.as_ref().map_or(0, |head| head.revision),
                        machine: machine(),
                        setup: merged.clone(),
                        applied: true,
                    })
                    .await?;
                if saved == Saved::Conflict {
                    return Ok(Outcome::Again);
                }
            }
            Some(merged)
        }
    };
    ensure_account(&token)?;
    if let Some(setup) = agreed
        && baseline.is_none_or(|baseline| baseline.setup != setup)
    {
        Baseline { setup }.save(root, &account.account).await?;
    }
    drop(profile_io);
    Ok(Outcome::Done)
}

async fn install(
    forge: &Forge,
    profile: &Profile,
    setup: &Setup,
    progress: Option<&Progress>,
) -> Result<()> {
    match progress {
        Some(progress) => restore(forge, profile, setup, progress).await?,
        None => {
            backend::tracked(
                "installing the setup from the cloud",
                |progress| async move { Ok(restore(forge, profile, setup, &progress).await?) },
            )
            .await?;
        }
    }
    Ok(())
}

/// Opens the conflict dialog. `then` hears whether the conflict is settled.
pub fn ask(pending: Pending, then: impl FnOnce(bool) + Send + 'static) {
    if ASKING.swap(true, Ordering::SeqCst) {
        then(false);
        return;
    }
    let input = ConflictInput {
        machine: pending.head.machine.clone(),
        created: pending.head.created,
        summary: Summary::between(&pending.local, &pending.head.setup),
    };
    conflict_dialog::show(input, move |keep_local| {
        // The dialog blocks the page, but a change that started before it
        // opened can still run. The next sync asks again.
        if backend::busy() {
            ASKING.store(false, Ordering::SeqCst);
            toast::info("wait for the running operation to finish");
            then(false);
            return;
        }
        backend::change(
            "settling the sync conflict",
            move |forge, progress| async move { resolve(forge, pending, keep_local, &progress).await },
            move |result| {
                ASKING.store(false, Ordering::SeqCst);
                match &result {
                    Ok(()) if keep_local => toast::success("This machine is now the cloud setup."),
                    Ok(()) => {
                        toast::success("The cloud setup is installed.");
                        sync_panel::applied();
                    }
                    Err(error) => toast::failure(error),
                }
                then(result.is_ok());
            },
        );
    });
}

/// Called through `backend::change`, which holds the profile lock throughout.
async fn resolve(
    forge: &Forge,
    pending: Pending,
    keep_local: bool,
    progress: &Progress,
) -> Result<()> {
    ensure_account(&pending.token)?;
    let profile = backend::profile(forge, progress).await?;
    if capture(&profile).await? != pending.captured {
        bail!("local mods or settings changed meanwhile, sync checks again");
    }
    let client = social::client()?;
    let account = client.sync_head().await?;
    if account.account != pending.account
        || account.head.map(|head| head.revision) != Some(pending.head.revision)
    {
        bail!("the cloud setup changed meanwhile, sync checks again");
    }
    // Picking the cloud parks the local side in the history first, so a wrong
    // pick can be restored from there.
    let saved = client
        .sync_save(&Save {
            base: pending.head.revision,
            machine: machine(),
            setup: pending.local.clone(),
            applied: keep_local,
        })
        .await?;
    if saved == Saved::Conflict {
        bail!("another machine just saved changes, sync checks again");
    }
    let agreed = if keep_local {
        pending.local
    } else {
        restore(forge, &profile, &pending.head.setup, progress).await?;
        pending.head.setup
    };
    ensure_account(&pending.token)?;
    Baseline { setup: agreed }
        .save(forge.data().root(), &pending.account)
        .await?;
    Ok(())
}

pub async fn history() -> Result<Vec<HistoryRow>> {
    Ok(social::client()?.sync_history().await?)
}

/// The old setup becomes a new head on the server. This machine then installs
/// it like a change from any other machine.
pub async fn rollback(revision: i64) -> Result<()> {
    let client = social::client()?;
    let head = client.sync_head().await?.head;
    let saved = client
        .sync_restore(&Restore {
            revision,
            base: head.map_or(0, |head| head.revision),
            machine: machine(),
        })
        .await?;
    if saved == Saved::Conflict {
        bail!("another machine just saved changes, try again");
    }
    Ok(())
}

/// Revisions of releases up to 0.1.8 carry no machine name.
pub fn machine_label(machine: &str) -> &str {
    if machine.is_empty() {
        "an unnamed machine"
    } else {
        machine
    }
}

fn machine() -> String {
    gethostname().to_string_lossy().into_owned()
}

fn ensure_account(token: &str) -> Result<()> {
    if GoogleLogin::token().as_deref() != Some(token) {
        bail!("the signed-in account changed");
    }
    Ok(())
}
