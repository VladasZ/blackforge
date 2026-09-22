//! A small square button that shows one icon, for an action a row has no
//! room to spell out.

use hilen::{
    Event,
    refs::Weak,
    ui::{ImageView, Setup, ViewData, ViewTouch, view},
};

use crate::{
    assets,
    ui::{colors, hover},
};

pub const SIZE: f32 = 28.0;

const ICON: f32 = 16.0;

#[view]
pub struct IconButton {
    pub tapped: Event,

    icon_name: String,
    disabled: bool,

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
        self.touch().up_inside.sub(self, move || {
            if !self.disabled {
                self.tapped.trigger(());
            }
        });

        hover::clickable(self);

        self.enable_hover();
        self.touch().hovered.val(self, move |hovered| {
            if hovered && !self.disabled {
                self.set_color(colors::NAV_HOVER_BG);
            } else {
                self.set_color(colors::CLEAR);
            }
        });
    }
}

impl IconButton {
    pub fn set_icon(mut self: Weak<Self>, name: &str) {
        name.clone_into(&mut self.icon_name);
        self.icon.set_image(name);
    }

    /// A disabled button draws its icon gray and ignores taps.
    pub fn set_enabled(mut self: Weak<Self>, enabled: bool) {
        self.disabled = !enabled;
        if enabled {
            self.icon.set_image(self.icon_name.as_str());
        } else {
            self.icon
                .set_image(assets::tinted(&self.icon_name, colors::ICON.resolve()));
            self.set_color(colors::CLEAR);
        }
    }
}
