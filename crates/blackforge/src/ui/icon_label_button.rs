//! A bordered button with an icon before its text, for an action that reads
//! better with a picture next to the word.

use hilen::{
    Event,
    refs::Weak,
    ui::{ImageView, Label, Setup, TextAlignment, ViewData, ViewTouch, view},
};

use crate::ui::{colors, hover};

const PAD: f32 = 12.0;
const ICON: f32 = 16.0;
const GAP: f32 = 8.0;

#[view]
pub struct IconLabelButton {
    pub tapped: Event,

    #[init]
    icon: ImageView,
    label: Label,
}

impl Setup for IconLabelButton {
    fn setup(self: Weak<Self>) {
        self.set_corner_radius(7);
        self.set_border_color(colors::BORDER);
        self.set_border_width(1);

        self.icon.place().l(PAD).center_y().size(ICON, ICON);

        self.label.set_text_size(13).set_text_color(colors::FG);
        self.label.set_alignment(TextAlignment::Left);
        self.label.set_color(colors::CLEAR);

        self.enable_touch();
        self.touch()
            .up_inside
            .sub(self, move || self.tapped.trigger(()));

        hover::clickable(self);

        self.enable_hover();
        self.touch().hovered.val(self, move |hovered| {
            if hovered {
                self.set_color(colors::NAV_HOVER_BG);
            } else {
                self.set_color(colors::CLEAR);
            }
        });
    }
}

impl IconLabelButton {
    /// Sets the content and returns the width the button needs. The text
    /// decides the width, so the owner places the button after the call.
    pub fn set(self: Weak<Self>, icon: &str, text: &str) -> f32 {
        self.icon.set_image(icon);
        self.label.set_text(text);

        let text_left = PAD + ICON + GAP;
        let text_width = self.label.content_size().width;
        self.label
            .place()
            .clear()
            .l(text_left)
            .t(0)
            .b(0)
            .w(text_width);
        text_left + text_width + PAD
    }
}
