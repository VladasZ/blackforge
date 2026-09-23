//! The settings of this machine: where the game is, how it starts, and the
//! achievements switch. They used to sit on top of the Mods page and pushed
//! the list half way down. Under them the account and the app version.

use hilen::{
    refs::Weak,
    ui::{Button, Label, Setup, ViewData, ViewFrame, view},
};

use crate::{
    ui::{
        account_card::{self, AccountCard},
        colors,
        game_panel::GamePanel,
        style, toast,
    },
    updater,
};

const GAP: f32 = 16.0;
const VERSION_H: f32 = 20.0;
const CHECK_W: f32 = 130.0;

#[view]
pub struct SettingsPage {
    #[init]
    title: Label,
    subtitle: Label,
    game: GamePanel,
    account: AccountCard,
    version: Label,
    check: Button,
}

impl Setup for SettingsPage {
    fn setup(self: Weak<Self>) {
        style::title(self.title, "Settings");
        self.title.place().t(24).l(style::PAGE_PAD).size(300, 30);

        style::dim(self.subtitle);
        self.subtitle
            .set_text("How the game starts on this machine");
        self.subtitle.place().t(56).l(style::PAGE_PAD).size(500, 16);

        self.account
            .place()
            .below(self.game, GAP)
            .l(style::PAGE_PAD)
            .r(style::PAGE_PAD)
            .h(account_card::HEIGHT);

        style::dim(self.version);
        self.version
            .set_text(format!("Version {}", env!("CARGO_PKG_VERSION")));
        let version = self.version.content_size().width;
        self.version
            .place()
            .below(self.account, GAP)
            .l(style::PAGE_PAD)
            .size(version, VERSION_H);

        style::ghost(self.check, "Check for updates");
        self.check.set_border_width(0);
        self.check.set_text_size(12);
        self.check.set_text_color(colors::ACCENT);
        self.check
            .place()
            .below(self.account, GAP - 2.0)
            .l(style::PAGE_PAD + version)
            .size(CHECK_W, 24);
        self.check.on_tap(check_for_update);

        // The hint of the game card wraps, so the card grows as the page
        // gets narrower.
        self.place_game();
        self.size_changed().sub(move || self.place_game());
    }
}

impl SettingsPage {
    fn place_game(self: Weak<Self>) {
        let height = self.game.fit(self.width() - 2.0 * style::PAGE_PAD);
        self.game
            .place()
            .clear()
            .t(style::HEADER)
            .l(style::PAGE_PAD)
            .r(style::PAGE_PAD)
            .h(height);
    }
}

/// A new version shows as the update button in the status bar, the toast
/// points there.
fn check_for_update() {
    updater::check(|state| {
        if let Some(error) = &state.error {
            toast::error(format!("The update check failed: {error}"));
        } else if state.has_update() {
            toast::info(format!(
                "Version {} is ready, the button at the bottom installs it",
                state.version.as_deref().unwrap_or_default()
            ));
        } else {
            toast::success("This is the newest version");
        }
    });
}
