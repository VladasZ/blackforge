use std::{env::var, fs::create_dir_all};

use blackforge_core::paths::DataDir;
use dotenvy::dotenv;
use hilen::{
    App, AppRunner, BugReport, PinnedFuture, Window,
    dispatch::after,
    refs::Own,
    store::OnDisk,
    system::UpdateSource,
    ui::{Setup, Size, View},
};

use crate::ui::Shell;

#[derive(Default)]
pub struct BlackforgeApp;

impl App for BlackforgeApp {
    fn make_root_view(&self) -> Own<dyn View> {
        crate::assets::load();
        Shell::new()
    }

    // Physical pixels. On a retina screen this opens as 1200 x 800 points.
    fn initial_size(&self) -> Size {
        (2400, 1600).into()
    }

    // The settings of the window live next to the profiles, so one folder
    // still holds everything.
    fn before_launch(&self) {
        let data = DataDir::locate().expect("this system has no home folder");
        create_dir_all(data.root()).expect("cannot create the data folder");
        OnDisk::<()>::set_root_path(data.root());
    }

    fn after_launch(&self) {
        AppRunner::set_window_title("Blackforge");
        Window::set_icon(include_bytes!("../../../assets/icon.png"));
        // The rooster loop of kukareker, shown in the engine's bug report
        // dialog.
        BugReport::set_animation(include_bytes!("../../../assets/bug-rooster.gif"));
        crate::social::init();
        after(3.0, || {
            crate::updater::check(|_| {});
            // Mods and configs can change while the app is closed, from the
            // command line or a text editor.
            crate::social::share_profile();
            // The sidebar badges say what waits before a page is opened.
            crate::updates::check(false);
            crate::ui::doctor_page::check_in_background();
            crate::social::watch_requests();
            crate::launcher::watch_steam();
            crate::launcher::move_old_game_args();
        });
    }

    fn update_source(&self) -> PinnedFuture<Option<UpdateSource>> {
        Box::pin(async { Ok(Some(crate::updater::source())) })
    }

    // Without these the engine logger drops everything of the app below a
    // warning, and the log a bug report attaches says nothing.
    fn log_targets(&self) -> &'static [&'static str] {
        &["blackforge_gui", "blackforge_core"]
    }

    // Crashes and bug reports go to Sentry through the engine. A release
    // build embeds the DSN from BLACKFORGE_SENTRY_URL at compile time, the
    // release workflow puts it in the env from Infisical. A dev build
    // without it reads the same variable at runtime, from the environment
    // or a .env next to the binary, and without one reporting stays off.
    fn sentry_url(&self) -> PinnedFuture<Option<String>> {
        Box::pin(async {
            if let Some(url) = option_env!("BLACKFORGE_SENTRY_URL").filter(|url| !url.is_empty()) {
                return Ok(Some(url.to_owned()));
            }
            if let Err(error) = dotenv() {
                log::debug!("no .env loaded: {error}");
            }
            Ok(var("BLACKFORGE_SENTRY_URL")
                .ok()
                .filter(|url| !url.is_empty()))
        })
    }
}
