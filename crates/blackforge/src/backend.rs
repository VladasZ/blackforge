//! The bridge between the views and the core. A core operation is async and
//! runs on the tokio runtime of the engine. Its progress goes to the status
//! bar and its result comes back on the main thread. Every operation leaves
//! its start, its time and its error in the log file of the engine.

use std::{
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

use anyhow::Result;
use blackforge_core::{
    Error,
    forge::{DEFAULT_PROFILE, Forge},
    game::{Target, VALHEIM},
    profile::Profile,
    progress::Progress,
    thunderstore::{FRESH_ENOUGH, PackageIndex},
};
use hilen::dispatch::{on_main, spawn};
use tokio::sync::Mutex;

use crate::{
    social,
    ui::{status, toast},
};

static FORGE: OnceLock<Forge> = OnceLock::new();

struct KeptIndex {
    loaded: Instant,
    index: Arc<PackageIndex>,
}

static INDEX: Mutex<Option<KeptIndex>> = Mutex::const_new(None);

/// Held while the default profile is looked up or made, so two operations
/// that start together on a fresh machine do not both try to create it.
static DEFAULT: Mutex<()> = Mutex::const_new(());

/// Two operations that change a profile must not overlap, the second would
/// read a manifest and a lock the first is about to replace.
static CHANGING: AtomicBool = AtomicBool::new(false);

pub fn forge() -> Result<&'static Forge> {
    if let Some(forge) = FORGE.get() {
        return Ok(forge);
    }
    let forge = Forge::open()?;
    Ok(FORGE.get_or_init(|| forge))
}

/// The window has no profiles. It always works on the one named `default`,
/// whatever the command line made the active one, and creates it on a fresh
/// machine, so no setup step exists.
pub async fn profile(forge: &Forge, progress: &Progress) -> Result<Profile> {
    let held = DEFAULT.lock().await;
    let profile = match forge.store().get(DEFAULT_PROFILE).await {
        Ok(profile) => profile,
        Err(Error::ProfileNotFound(_)) => {
            forge
                .create_profile(DEFAULT_PROFILE, VALHEIM, Target::Client, progress)
                .await?
        }
        Err(error) => return Err(error.into()),
    };
    drop(held);
    Ok(profile)
}

/// The package list of the game. Reading it from disk
/// parses the whole Thunderstore list, so one copy stays in memory.
pub async fn index(forge: &Forge, progress: &Progress) -> Result<Arc<PackageIndex>> {
    if let Some(kept) = INDEX.lock().await.as_ref()
        && kept.loaded.elapsed() < FRESH_ENOUGH
    {
        return Ok(kept.index.clone());
    }

    let profile = profile(forge, progress).await?;
    let game = forge.game(&profile.manifest().await?).await?;
    let index = Arc::new(forge.index(&game, FRESH_ENOUGH, progress).await?);
    *INDEX.lock().await = Some(KeptIndex {
        loaded: Instant::now(),
        index: index.clone(),
    });
    Ok(index)
}

/// Drops the copy in memory. Call it after an operation that downloads a
/// fresh list, so the next reader sees it too.
pub async fn forget_index() {
    *INDEX.lock().await = None;
}

/// Runs an operation that only reads. Any number of them can overlap.
pub fn load<T, Fut>(
    title: &str,
    work: impl FnOnce(&'static Forge, Progress) -> Fut + Send + 'static,
    done: impl FnOnce(Result<T>) + Send + 'static,
) where
    T: Send + 'static,
    Fut: Future<Output = Result<T>> + Send + 'static,
{
    start(title, work, done);
}

/// Runs an operation that changes a profile. Only one runs at a time, a
/// second one is refused with a message and `done` is never called.
pub fn change<T, Fut>(
    title: &str,
    work: impl FnOnce(&'static Forge, Progress) -> Fut + Send + 'static,
    done: impl FnOnce(Result<T>) + Send + 'static,
) where
    T: Send + 'static,
    Fut: Future<Output = Result<T>> + Send + 'static,
{
    if CHANGING.swap(true, Ordering::SeqCst) {
        toast::info("wait for the running operation to finish");
        return;
    }
    start(title, work, move |result| {
        CHANGING.store(false, Ordering::SeqCst);
        // Friends see the mods and the changed settings, so every change of
        // the profile is a reason to send them again.
        if result.is_ok() {
            social::share_profile();
        }
        done(result);
    });
}

fn start<T, Fut>(
    title: &str,
    work: impl FnOnce(&'static Forge, Progress) -> Fut + Send + 'static,
    done: impl FnOnce(Result<T>) + Send + 'static,
) where
    T: Send + 'static,
    Fut: Future<Output = Result<T>> + Send + 'static,
{
    let id = status::begin(title);
    let (progress, mut events) = Progress::channel();
    let title = title.to_owned();
    let started = Instant::now();
    log::info!("{title}: start");

    spawn(async move {
        while let Some(event) = events.recv().await {
            on_main(move || status::event(id, event));
        }
    });

    spawn(async move {
        let result = match forge() {
            Ok(forge) => work(forge, progress).await,
            Err(error) => Err(error),
        };
        let took = started.elapsed().as_millis();
        match &result {
            Ok(_) => log::info!("{title}: done in {took} ms"),
            Err(error) => log::error!("{title}: failed in {took} ms: {error:#}"),
        }
        on_main(move || {
            status::end(id);
            done(result);
        });
    });
}
