//! A small square button that shows one icon, for an action a row has no
//! room to spell out.

use hilen::{
    Event,
    refs::Weak,
    ui::{ImageView, Setup, ViewData, ViewTouch, view},
};

use crate::ui::colors;

pub const SIZE: f32 = 28.0;

const ICON: f32 = 16.0;

#[view]
pub struct IconButton {
    pub tapped: Event,

    #[init]
    icon: ImageView,
}

impl Setup for IconButton {
    fn setup(self: Weak<Self>) {
        self.set_corner_radius(7);
        self.set_border_color(colors::BORDER);
        self.set_border_width(1);

        self.icon.place().center().size(ICON, ICON);

        self.enable_touch();
        self.touch()
            .up_inside
            .sub(self, move || self.tapped.trigger(()));

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

impl IconButton {
    pub fn set_icon(self: Weak<Self>, name: &str) {
        self.icon.set_image(name);
    }
}
