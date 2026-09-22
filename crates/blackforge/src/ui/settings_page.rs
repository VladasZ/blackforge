//! The settings of this machine: where the game is, how it starts, and the
//! achievements switch. They used to sit on top of the Mods page and pushed
//! the list half way down.

use hilen::{
    refs::Weak,
    ui::{Label, Setup, ViewData, view},
};

use crate::ui::{
    game_panel::{self, GamePanel},
    style,
};

#[view]
pub struct SettingsPage {
    #[init]
    title: Label,
    subtitle: Label,
    game: GamePanel,
}

impl Setup for SettingsPage {
    fn setup(self: Weak<Self>) {
        style::title(self.title, "Settings");
        self.title.place().t(24).l(style::PAGE_PAD).size(300, 30);

        style::dim(self.subtitle);
        self.subtitle
            .set_text("how the game starts on this machine");
        self.subtitle.place().t(56).l(style::PAGE_PAD).size(500, 16);

        self.game
            .place()
            .t(style::HEADER)
            .l(style::PAGE_PAD)
            .r(style::PAGE_PAD)
            .h(game_panel::HEIGHT);
    }
}
