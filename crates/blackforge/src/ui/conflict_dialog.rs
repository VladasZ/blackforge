//! Shown only when this machine and the cloud both changed the same thing.
//! It has no close button, sync cannot move until one whole side is picked.

use blackforge_api::setup::Summary;
use hilen::{
    OnceEvent,
    refs::Weak,
    ui::{Button, Label, ModalView, Setup, Size, UIColor, VerticalAlignment, ViewData, view},
};

use crate::{
    cloud,
    ui::{colors, style},
};

const PAD: f32 = 24.0;

#[derive(Clone, Debug, Default)]
pub struct ConflictInput {
    pub machine: String,
    pub created: i64,
    /// What using the cloud setup changes on this machine.
    pub summary: Summary,
}

#[view]
pub struct ConflictDialog {
    /// True keeps this machine, false takes the cloud.
    event: OnceEvent<bool>,

    #[init]
    title: Label,
    text: Label,
    detail: Label,
    local: Button,
    cloud: Button,
}

impl ModalView<ConflictInput, bool> for ConflictDialog {
    fn modal_event(&self) -> &OnceEvent<bool> {
        &self.event
    }

    fn modal_size() -> Size {
        (540, 250).into()
    }

    fn modal_scrim_color() -> UIColor {
        colors::SCRIM.into()
    }

    fn setup_input(self: Weak<Self>, input: ConflictInput) {
        self.detail.set_text(format!(
            "The cloud setup was saved from {} on {}. Using it here means: {}. The setup you do not pick stays in the history.",
            cloud::machine_label(&input.machine),
            cloud::when(input.created),
            input.summary
        ));
    }
}

impl Setup for ConflictDialog {
    fn setup(self: Weak<Self>) {
        style::card(self);

        style::title(self.title, "Sync conflict");
        self.title.set_text_size(18);
        self.title.place().t(PAD).l(PAD).r(PAD).h(24);

        style::body(self.text);
        self.text.set_multiline(true);
        self.text.set_vertical_alignment(VerticalAlignment::Top);
        self.text.set_text(
            "This machine and the cloud both changed the same mods or settings. Pick the setup to keep.",
        );
        self.text.place().t(PAD + 36.0).l(PAD).r(PAD).h(40);

        style::dim(self.detail);
        self.detail.set_multiline(true);
        self.detail.set_vertical_alignment(VerticalAlignment::Top);
        self.detail.place().t(PAD + 84.0).l(PAD).r(PAD).h(64);

        style::primary(self.cloud, "Use the cloud");
        self.cloud.place().r(PAD).b(PAD).size(150, style::BUTTON_H);
        self.cloud.on_tap(move || self.hide_modal(false));

        style::ghost(self.local, "Keep this machine");
        self.local
            .place()
            .r(PAD + 160.0)
            .b(PAD)
            .size(170, style::BUTTON_H);
        self.local.on_tap(move || self.hide_modal(true));
    }
}

pub fn show(input: ConflictInput, picked: impl FnOnce(bool) + Send + 'static) {
    ConflictDialog::show_modally_with_input(input, picked);
}
