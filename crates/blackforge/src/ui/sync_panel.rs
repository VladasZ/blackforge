//! The cloud sync line above the Mods list. Sync runs by itself, so this only
//! says how it goes and opens the history.

use std::cell::Cell;

use hilen::{
    Event,
    login::GoogleLoginButton,
    refs::Weak,
    ui::{Button, Label, ModalView, Setup, ViewData, view},
};

use crate::{
    cloud, social,
    ui::{history_modal::HistoryModal, style, toast},
};

thread_local! {
    static PANEL: Cell<Weak<SyncPanel>> = const { Cell::new(Weak::const_default()) };
}

fn panel() -> Option<Weak<SyncPanel>> {
    let panel = PANEL.with(Cell::get);
    panel.is_ok().then_some(panel)
}

/// The sync status changed. Nothing happens while another page is open.
pub fn refresh() {
    if let Some(panel) = panel() {
        panel.show();
    }
}

/// A setup from the cloud was installed, the page has to read the mods again.
pub fn applied() {
    if let Some(panel) = panel() {
        panel.applied.trigger(());
    }
}

#[view]
pub struct SyncPanel {
    pub applied: Event,

    #[init]
    title: Label,
    status: Label,
    login: GoogleLoginButton,
    history: Button,
}

impl Setup for SyncPanel {
    fn setup(self: Weak<Self>) {
        style::body(self.title);
        self.title.set_text("Cloud sync");
        self.title.place().t(0).l(0).size(100, 32);
        style::dim(self.status);
        self.status.set_multiline(true);
        self.status.place().t(0).l(108).r(0).h(32);
        self.login.place().t(38).l(0).size(220, 36);
        self.login.logged_in.val(move |_| {
            self.show();
            cloud::schedule();
        });
        self.login.failed.val(toast::error);
        style::ghost(self.history, "History");
        self.history.place().t(38).l(0).size(100, style::BUTTON_H);
        self.history
            .on_tap(|| HistoryModal::show_modally_with_input((), |_| {}));

        PANEL.with(|slot| slot.set(self));
        self.show();
        cloud::schedule();
    }
}

impl SyncPanel {
    fn show(self: Weak<Self>) {
        let signed_in = social::signed_in();
        self.login.set_hidden(signed_in);
        self.history.set_hidden(!signed_in);
        if signed_in {
            self.status.set_text(cloud::status().text());
        } else {
            self.status
                .set_text("Sign in to keep your mods and settings the same on every machine.");
        }
    }
}
