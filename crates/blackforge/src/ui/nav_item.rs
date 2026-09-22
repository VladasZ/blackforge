//! One entry of a list of places to go, a page in the sidebar or a tab inside
//! a page: the icon, the title, a hover wash and an accent state for the open
//! one. A sidebar entry can carry a badge at its right edge.

use hilen::{
    Event,
    refs::Weak,
    ui::{Container, ImageView, Label, Setup, TextAlignment, UIEvents, ViewData, ViewTouch, view},
};

use crate::{
    assets,
    ui::{colors, hover, style},
};

const ICON: f32 = 18.0;
const BADGE_H: f32 = 18.0;
const BADGE_MIN_W: f32 = 20.0;
const BADGE_PAD: f32 = 12.0;
const DOT: f32 = 8.0;

/// What waits on the page behind an entry.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Badge {
    #[default]
    None,
    /// How many things wait, like friend requests or mod updates.
    Count(usize),
    /// Something is wrong there, like a failed Doctor check.
    Alert,
}

#[view]
pub struct NavItem {
    pub tapped: Event,

    selected: bool,
    icon_name: String,

    #[init]
    icon: ImageView,
    label: Label,
    count: Label,
    dot: Container,
}

impl Setup for NavItem {
    fn setup(self: Weak<Self>) {
        self.set_corner_radius(8);

        self.icon.place().l(12).center_y().size(ICON, ICON);

        style::body(self.label);
        self.label.place().l(12.0 + ICON + 12.0).r(8).t(0).b(0);

        self.count
            .set_text_size(11)
            .set_text_color(colors::ON_ACCENT);
        self.count.set_alignment(TextAlignment::Center);
        self.count.set_color(colors::ACCENT);
        self.count.set_corner_radius(BADGE_H / 2.0);
        self.count.set_hidden(true);

        self.dot.set_color(colors::BAD);
        self.dot.set_corner_radius(DOT / 2.0);
        self.dot.place().r(BADGE_PAD).center_y().size(DOT, DOT);
        self.dot.set_hidden(true);

        self.enable_touch();
        self.touch()
            .up_inside
            .sub(self, move || self.tapped.trigger(()));

        hover::clickable(self);

        self.enable_hover();
        self.touch()
            .hovered
            .val(self, move |hovered| self.refresh(hovered));

        // The icon tint is a plain color, it does not follow the theme by
        // itself.
        UIEvents::theme_changed().sub(self, move || self.refresh(self.is_hovered()));

        self.refresh(false);
    }
}

impl NavItem {
    pub fn set_content(mut self: Weak<Self>, icon: &str, title: &str) {
        icon.clone_into(&mut self.icon_name);
        self.label.set_text(title);
        self.refresh(self.is_hovered());
    }

    pub fn set_selected(mut self: Weak<Self>, selected: bool) {
        self.selected = selected;
        self.refresh(self.is_hovered());
    }

    pub fn set_badge(self: Weak<Self>, badge: Badge) {
        self.dot.set_hidden(badge != Badge::Alert);
        let count = match badge {
            Badge::Count(count) if count > 0 => Some(count),
            _ => None,
        };
        self.count.set_hidden(count.is_none());
        if let Some(count) = count {
            self.count.set_text(if count > 99 {
                "99+".to_owned()
            } else {
                count.to_string()
            });
            let width = (self.count.content_size().width + 10.0).max(BADGE_MIN_W);
            self.count
                .place()
                .clear()
                .r(BADGE_PAD)
                .center_y()
                .size(width, BADGE_H);
        }
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
        if !self.icon_name.is_empty() {
            let tint = if self.selected {
                colors::ACCENT
            } else {
                colors::ICON.resolve()
            };
            self.icon.set_image(assets::tinted(&self.icon_name, tint));
        }
    }
}
