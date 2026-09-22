//! The bridge between the views and the core. A core operation is async and
//! runs on the tokio runtime of the engine. Its progress goes to the status
//! bar and its result comes back on the main thread. Every operation leaves
//! its start, its time and its error in the log file of the engine.

use std::{
    collections::HashMap,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

use anyhow::Result;
use blackforge_core::{
    Error,
    broken::Broken,
    cloud::recover,
    forge::{DEFAULT_PROFILE, Forge},
    game::{Target, VALHEIM},
    ident::PackageId,
    profile::Profile,
    progress::Progress,
    thunderstore::{FRESH_ENOUGH, Package, PackageIndex},
};
use hilen::dispatch::{on_main, spawn};
use tokio::sync::{Mutex, oneshot};

use crate::{
    social,
    ui::{busy::changed as buttons_changed, status, toast},
};

static FORGE: OnceLock<Forge> = OnceLock::new();

struct KeptIndex {
    loaded: Instant,
    index: Arc<PackageIndex>,
}

static INDEX: Mutex<Option<KeptIndex>> = Mutex::const_new(None);

struct KeptBroken {
    loaded: Instant,
    broken: Arc<Broken>,
}

static BROKEN: Mutex<Option<KeptBroken>> = Mutex::const_new(None);

/// Held while the default profile is looked up or made, so two operations
/// that start together on a fresh machine do not both try to create it.
static DEFAULT: Mutex<()> = Mutex::const_new(());
static RECOVERED: AtomicBool = AtomicBool::new(false);
pub static PROFILE_IO: Mutex<()> = Mutex::const_new(());

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
    if !RECOVERED.load(Ordering::SeqCst) {
        recover(&forge.data().profiles_dir().join(DEFAULT_PROFILE)).await?;
        RECOVERED.store(true, Ordering::SeqCst);
    }
    let profile = match forge.store().get(DEFAULT_PROFILE).await {
        Ok(profile) => profile,
        Err(Error::ProfileNotFound(_)) => {
            let profile = forge
                .create_profile(DEFAULT_PROFILE, VALHEIM, Target::Client, progress)
                .await?;
            // The lock of a new profile names the mod loader, but none of its
            // files is on disk yet, and the game cannot start without them.
            forge.sync(&profile, progress).await?;
            profile
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

/// The mods the server lists as broken on the game version of this machine.
/// One copy stays in memory for an hour, like the package list.
pub async fn broken(forge: &Forge, progress: &Progress) -> Result<Arc<Broken>> {
    if let Some(kept) = BROKEN.lock().await.as_ref()
        && kept.loaded.elapsed() < FRESH_ENOUGH
    {
        return Ok(kept.broken.clone());
    }

    let profile = profile(forge, progress).await?;
    let game = forge.game(&profile.manifest().await?).await?;
    let broken = Arc::new(forge.broken(&game).await?);
    *BROKEN.lock().await = Some(KeptBroken {
        loaded: Instant::now(),
        broken: broken.clone(),
    });
    Ok(broken)
}

/// The same for a page that has to list mods with or without the network.
/// When the list cannot be had, no mod is flagged, the log says why, and
/// the Doctor page shows the reason to the user.
pub async fn broken_known(forge: &Forge, progress: &Progress) -> Arc<Broken> {
    match broken(forge, progress).await {
        Ok(broken) => broken,
        Err(error) => {
            log::warn!("no broken flags, the list did not load: {error:#}");
            Arc::new(Broken::default())
        }
    }
}

/// The description of every named mod that the package list knows, by id.
/// A page that must work without the network shows its rows without the
/// descriptions when the list cannot be read, the failure goes to the log.
pub async fn descriptions<'a>(
    forge: &Forge,
    progress: &Progress,
    ids: impl Iterator<Item = &'a PackageId>,
) -> HashMap<String, String> {
    let index = match index(forge, progress).await {
        Ok(index) => index,
        Err(error) => {
            log::warn!("no descriptions, the package list did not load: {error:#}");
            return HashMap::new();
        }
    };
    ids.filter_map(|id| {
        let package = index.get(id)?;
        Some((id.to_string(), summary(package)))
    })
    .collect()
}

/// The description of a package as one flowing text, a label wraps it by
/// itself.
pub fn summary(package: &Package) -> String {
    package
        .description
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
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
        toast::info("Wait for the running operation to finish");
        return;
    }
    buttons_changed(true);
    start(
        title,
        move |forge, progress| async move {
            let held = PROFILE_IO.lock().await;
            let result = work(forge, progress).await;
            drop(held);
            result
        },
        move |result| {
            CHANGING.store(false, Ordering::SeqCst);
            buttons_changed(false);
            // Friends see the mods and the changed settings, so every change of
            // the profile is a reason to send them again.
            if result.is_ok() {
                social::share_profile();
            }
            done(result);
        },
    );
}

/// A background sync never queues behind the user, it skips and comes back.
pub fn busy() -> bool {
    CHANGING.load(Ordering::SeqCst)
}

/// A status bar entry for work that already runs on the runtime, the install
/// step of a background sync. Such work has no view that waits for a result.
pub async fn tracked<T, Fut>(title: &str, work: impl FnOnce(Progress) -> Fut) -> Result<T>
where
    Fut: Future<Output = Result<T>>,
{
    let (progress, mut events) = Progress::channel();
    let (sent, begun) = oneshot::channel();
    let shown = title.to_owned();
    on_main(move || {
        let id = status::begin(&shown);
        // Nobody waits for the id when the sync was dropped meanwhile.
        if sent.send(id).is_err() {
            status::end(id);
        }
    });
    let id = begun.await?;
    spawn(async move {
        while let Some(event) = events.recv().await {
            on_main(move || status::event(id, event));
        }
    });
    let result = work(progress).await;
    on_main(move || status::end(id));
    result
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
