//! One entry of a list of places to go, a page in the sidebar or a tab inside
//! a page: the icon, the title, a hover wash and an accent state for the open
//! one.

use hilen::{
    Event,
    refs::Weak,
    ui::{ImageView, Label, Setup, ViewData, ViewTouch, view},
};

use crate::ui::{colors, style};

const ICON: f32 = 18.0;

#[view]
pub struct NavItem {
    pub tapped: Event,

    selected: bool,

    #[init]
    icon: ImageView,
    label: Label,
}

impl Setup for NavItem {
    fn setup(self: Weak<Self>) {
        self.set_corner_radius(8);

        self.icon.place().l(12).center_y().size(ICON, ICON);

        style::body(self.label);
        self.label.place().l(12.0 + ICON + 12.0).r(8).t(0).b(0);

        self.enable_touch();
        self.touch()
            .up_inside
            .sub(self, move || self.tapped.trigger(()));

        self.enable_hover();
        self.touch()
            .hovered
            .val(self, move |hovered| self.refresh(hovered));

        self.refresh(false);
    }
}

impl NavItem {
    pub fn set_content(self: Weak<Self>, icon: &str, title: &str) {
        self.icon.set_image(icon);
        self.label.set_text(title);
    }

    pub fn set_selected(mut self: Weak<Self>, selected: bool) {
        self.selected = selected;
        self.refresh(self.is_hovered());
    }

    fn refresh(self: Weak<Self>, hovered: bool) {
        if self.selected {
            self.set_color(colors::NAV_ACTIVE_BG);
            self.label.set_text_color(colors::ACCENT);
        } else if hovered {
            self.set_color(colors::NAV_HOVER_BG);
            self.label.set_text_color(colors::FG);
        } else {
            self.set_color(colors::CLEAR);
            self.label.set_text_color(colors::FG);
        }
    }
}
