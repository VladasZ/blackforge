use hilen::{
    Event,
    dispatch::{on_main, spawn},
    refs::main_lock::MainLock,
    system::{UpdateInfo, UpdateSource, Updater},
};

// Shared state for the startup check and the status bar update button.

pub const MANIFEST_URL: &str = "https://gebling-studio.vladas.xyz/blackforge/download/updater.json";

// The public half of the ed25519 key that signs release artifacts in
// CI, see build/release. A binary that fails this check never installs.
const VERIFY_KEY: &str = include_str!("../../../assets/update-key.pub");

pub fn source() -> UpdateSource {
    UpdateSource {
        manifest_url: std::env::var("BLACKFORGE_UPDATE_URL")
            .unwrap_or_else(|_| MANIFEST_URL.into()),
        current_version: env!("CARGO_PKG_VERSION").into(),
        verify_key: VERIFY_KEY.trim().into(),
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Idle,
    Checking,
    Available,
    Installing,
}

#[derive(Default)]
pub struct UpdateState {
    pub phase: Phase,
    pub version: Option<String>,
    // Whole percent of the download, 0 until the first chunk lands.
    pub progress: u8,
    pub error: Option<String>,
    pub changed: Event,
    info: Option<UpdateInfo>,
}

static STATE: MainLock<UpdateState> = MainLock::new();

pub fn state() -> &'static mut UpdateState {
    STATE.get_or_init(UpdateState::default)
}

impl UpdateState {
    pub fn has_update(&self) -> bool {
        matches!(self.phase, Phase::Available | Phase::Installing)
    }

    pub fn busy(&self) -> bool {
        matches!(self.phase, Phase::Checking | Phase::Installing)
    }
}

// Asks the manifest, `on_done` runs on main with the settled state.
pub fn check(on_done: impl FnOnce(&UpdateState) + Send + 'static) {
    let state = state();
    if state.busy() {
        return;
    }
    state.phase = Phase::Checking;
    state.error = None;
    state.changed.trigger(());
    log::info!("updater: check start");
    spawn(async move {
        let result = Updater::check().await;
        on_main(move || {
            let state = self::state();
            match result {
                Ok(Some(info)) => {
                    log::info!(
                        "updater: update available current={} latest={}",
                        env!("CARGO_PKG_VERSION"),
                        info.version
                    );
                    state.version = Some(info.version.clone());
                    state.info = Some(info);
                    state.phase = Phase::Available;
                }
                Ok(None) => {
                    log::info!("updater: no update available");
                    state.version = None;
                    state.info = None;
                    state.phase = Phase::Idle;
                }
                Err(e) => {
                    log::error!("updater: check failed: {e:#}");
                    state.error = Some(format!("{e:#}"));
                    state.phase = Phase::Idle;
                }
            }
            state.changed.trigger(());
            on_done(state);
        });
    });
}

// Downloads, verifies, swaps the binary and relaunches. Errors land in
// `state().error` and `on_error`.
pub fn install(on_error: impl FnOnce(String) + Send + 'static) {
    let state = state();
    let Some(info) = state.info.take() else {
        return;
    };
    let version = info.version.clone();
    state.phase = Phase::Installing;
    state.progress = 0;
    state.error = None;
    state.changed.trigger(());
    log::info!(
        "updater: install start current={} latest={version}",
        env!("CARGO_PKG_VERSION")
    );
    spawn(async move {
        let mut last = 0u8;
        let result = Updater::install_with_progress(info, move |done, total| {
            let Some(total) = total.filter(|t| *t > 0) else {
                if done == 0 {
                    log::warn!(
                        "updater: Content-Length missing, the server may be returning an HTML fallback instead of the artifact"
                    );
                }
                return;
            };
            let percent = (done.saturating_mul(100) / total).min(100) as u8;
            if percent == last {
                return;
            }
            last = percent;
            on_main(move || {
                let state = self::state();
                state.progress = percent;
                state.changed.trigger(());
            });
        })
        .await;
        on_main(move || {
            let state = self::state();
            match result {
                Ok(()) => {
                    log::info!("updater: installed {version}, relaunching");
                    if let Err(e) = Updater::relaunch() {
                        log::error!("updater: relaunch failed: {e:#}");
                        state.error = Some(format!("{e:#}"));
                        state.phase = Phase::Idle;
                        state.changed.trigger(());
                        on_error(format!("{e:#}"));
                    }
                }
                Err(e) => {
                    log::error!("updater: install failed: {e:#} latest={version}");
                    state.error = Some(format!("{e:#}"));
                    state.phase = Phase::Idle;
                    state.version = None;
                    state.changed.trigger(());
                    on_error(format!("{e:#}"));
                }
            }
        });
    });
}
