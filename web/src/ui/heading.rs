//! A section title with the ornament rule under it.

use hilen::{
    refs::Weak,
    ui::{Container, ImageView, Setup, UIEvents, ViewData, ViewFrame, view},
};

use crate::{
    art, fonts,
    ui::{colors, embossed::Embossed},
};

pub const HEIGHT: f32 = 76.0;

const ORNAMENT: (f32, f32) = (120.0, 24.0);
const TITLE_HEIGHT: f32 = 40.0;
/// The ornament ends in a line on both sides, these carry it on.
const LINE_MAX: f32 = 260.0;

#[view]
pub struct Heading {
    #[init]
    title: Embossed,
    left: Container,
    left_lift: Container,
    right: Container,
    right_lift: Container,
    ornament: ImageView,
}

impl Setup for Heading {
    fn setup(self: Weak<Self>) {
        self.title
            .each(|label| {
                label
                    .set_font(fonts::heading(600.0))
                    .set_letter_spacing(3.0);
            })
            .set_shade(colors::INK_LIFT);
        self.title.face().set_text_color(colors::INK_TITLE);

        for line in [self.left, self.right] {
            line.set_color(colors::RULE);
        }
        for line in [self.left_lift, self.right_lift] {
            line.set_color(colors::RULE_LIFT);
        }

        self.restyle();
        UIEvents::theme_changed().sub(self, move || self.restyle());
        self.size_changed().sub(move || self.relayout());
    }
}

impl Heading {
    pub fn set_text(self: Weak<Self>, text: &str, size: f32) {
        self.title.set_text(text).each(|label| {
            label.set_text_size(size);
        });
    }

    fn restyle(self: Weak<Self>) {
        self.ornament.set_image(art::ornament());
    }

    fn relayout(self: Weak<Self>) {
        let width = self.width();
        let middle = width / 2.0;
        let y = TITLE_HEIGHT + 10.0;
        let line_y = y + ORNAMENT.1 / 2.0 - 0.5;
        let line = (middle - ORNAMENT.0 / 2.0).clamp(0.0, LINE_MAX);

        self.title.set_frame((0.0, 0.0, width, TITLE_HEIGHT));
        self.ornament
            .set_frame((middle - ORNAMENT.0 / 2.0, y, ORNAMENT.0, ORNAMENT.1));

        let left_x = middle - ORNAMENT.0 / 2.0 - line;
        let right_x = middle + ORNAMENT.0 / 2.0;

        self.left.set_frame((left_x, line_y, line, 1.0));
        self.left_lift.set_frame((left_x, line_y + 1.0, line, 1.0));
        self.right.set_frame((right_x, line_y, line, 1.0));
        self.right_lift
            .set_frame((right_x, line_y + 1.0, line, 1.0));
    }
}
