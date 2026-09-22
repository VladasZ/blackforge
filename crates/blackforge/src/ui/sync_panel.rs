//! The cloud sync card above the Mods list. Sync runs by itself, so the card
//! only says how it goes and opens the history.

use std::cell::Cell;

use hilen::{
    Event,
    login::GoogleLoginButton,
    refs::Weak,
    ui::{Container, ImageView, Label, ModalView, Setup, ViewData, ViewFrame, ViewTooltip, view},
};

use crate::{
    cloud::{self, Status},
    social,
    ui::{
        colors, history_modal::HistoryModal, icon_label_button::IconLabelButton, style, time, toast,
    },
};

pub const HEIGHT: f32 = 60.0;

const PAD: f32 = 12.0;
const BADGE: f32 = 36.0;
const ICON: f32 = 22.0;
const GAP: f32 = 14.0;
const LOGIN_W: f32 = 220.0;
const TEXT_L: f32 = PAD + BADGE + GAP;
const TITLE_T: f32 = 11.0;
/// A failure text longer than this is cut, the toast shows it in full.
const TEXT_MAX: f32 = 440.0;

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
    /// The width of the usual status texts, measured once. The button only
    /// moves for a text longer than these.
    usual: f32,

    #[init]
    badge: Container,
    icon: ImageView,
    title: Label,
    status: Label,
    login: GoogleLoginButton,
    history: IconLabelButton,
}

impl Setup for SyncPanel {
    fn setup(mut self: Weak<Self>) {
        style::card(self);

        self.badge.set_color(colors::ACCENT_BG);
        self.badge.set_corner_radius(10);
        self.badge.place().l(PAD).center_y().size(BADGE, BADGE);
        // A sibling of the badge, not its child. It is declared after the
        // badge, so it draws over it.
        self.icon.set_image("sync_cloud.svg");
        self.icon
            .place()
            .l(PAD + (BADGE - ICON) / 2.0)
            .center_y()
            .size(ICON, ICON);

        // The title over the status, right of the badge.
        style::body(self.title);
        self.title.set_text("Cloud sync");
        self.title.place().l(TEXT_L).t(TITLE_T).size(TEXT_MAX, 18);

        style::dim(self.status);
        self.status.set_ellipsize(true);
        self.status.set_text("Synced 59 minutes ago");
        self.usual = self.status.content_size().width;

        self.login.logged_in.val(move |_| {
            self.show();
            cloud::schedule();
        });
        self.login.failed.val(toast::error);

        self.history.set("sync_history.svg", "History");
        self.history
            .tapped
            .sub(|| HistoryModal::show_modally_with_input((), |_| {}));

        // The room for the status text follows the width of the card.
        self.size_changed().sub(move || self.show());

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
            let status = cloud::status();
            self.status.set_text(status.text());
            if let Status::Synced(at) = status {
                self.status
                    .set_tooltip(format!("Last synced {}", time::full(at)));
            }
        } else {
            self.status
                .set_text("Sign in to keep your mods and settings the same on every machine.");
        }
        // The text gives way to the button, it is cut before the button
        // leaves the card.
        let button = if signed_in {
            self.history.set("sync_history.svg", "History")
        } else {
            LOGIN_W
        };
        let room = (self.width() - TEXT_L - GAP - button - PAD).max(0.0);
        let text = self
            .status
            .content_size()
            .width
            .clamp(self.usual, TEXT_MAX)
            .min(room);
        self.status
            .place()
            .clear()
            .l(TEXT_L)
            .t(TITLE_T + 20.0)
            .size(text, 18);

        // The one visible button stands right of the text block, centered on
        // the height of the whole card.
        let left = TEXT_L + text + GAP;
        self.history
            .place()
            .clear()
            .l(left)
            .center_y()
            .size(button, style::BUTTON_H);
        self.login
            .place()
            .clear()
            .l(left)
            .center_y()
            .size(LOGIN_W, 36);
    }
}
