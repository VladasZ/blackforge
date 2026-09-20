use hilen::{
    refs::Weak,
    ui::{Container, Label, Setup, TextAlignment, UIEvent, ViewData, ViewFrame, ViewTouch, view},
};

use crate::{
    fonts,
    ui::{colors, text_margin},
};

pub const HEIGHT: f32 = 140.0;

#[view]
pub struct DownloadButton {
    pub on_tap: UIEvent,
    enabled: bool,

    #[init]
    inset: Container,
    os: Label,
    sub: Label,
    action: Label,
}

impl Setup for DownloadButton {
    fn setup(self: Weak<Self>) {
        self.set_color(colors::CARD_BG);
        self.set_border_width(1);
        self.set_border_color(colors::BORDER);
        self.set_corner_radius(0);
        self.inset.set_color(colors::CLEAR);
        self.inset.set_border_color(colors::BORDER_SOFT);
        self.inset.set_border_width(1);
        for label in [self.os, self.sub, self.action] {
            label.set_color(colors::CLEAR);
            label.set_alignment(TextAlignment::Left);
            label.set_font(fonts::inter(400.0));
        }
        self.os
            .set_font(fonts::heading(600.0))
            .set_text_size(21)
            .set_text_color(colors::ACCENT);
        self.sub.set_text_size(12).set_text_color(colors::FG_DIM);
        self.action
            .set_font(fonts::mono(400.0))
            .set_text_size(12)
            .set_text_color(colors::FG);
        self.enable_touch();
        self.enable_hover();
        self.touch().hovered.val(self, move |hovered| {
            self.set_color(if hovered {
                colors::HOVER
            } else {
                colors::CARD_BG
            });
            self.set_border_color(if hovered {
                colors::ACCENT
            } else {
                colors::BORDER.resolve()
            });
        });
        self.touch().up_inside.sub(self, move || {
            if self.enabled {
                self.on_tap.trigger(());
            }
        });
        self.size_changed().sub(move || self.relayout());
    }
}

impl DownloadButton {
    pub fn set_data(mut self: Weak<Self>, os: &str, sub: &str, version: &str) {
        self.os.set_text(os);
        self.sub.set_text(sub);
        self.enabled = !version.is_empty();
        self.action.set_text(if self.enabled {
            format!("↓ Download v{version}")
        } else {
            "No release available".to_string()
        });
    }

    fn relayout(self: Weak<Self>) {
        self.inset
            .set_frame((5.0, 5.0, (self.width() - 10.0).max(0.0), HEIGHT - 10.0));
        let x = 20.0 - text_margin();
        let width = (self.width() - 40.0 + text_margin()).max(0.0);
        self.os.set_frame((x, 18.0, width, 30.0));
        self.sub.set_frame((x, 52.0, width, 22.0));
        self.action.set_frame((x, 99.0, width, 23.0));
    }
}
