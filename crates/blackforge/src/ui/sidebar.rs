//! The left column: the brand, one entry per page and the button that starts
//! the game.

use hilen::{
    Event,
    refs::Weak,
    ui::{Button, Container, ImageView, Label, Setup, ViewData, ViewSubviews, ViewTouch, view},
};

use crate::{
    launcher,
    ui::{colors, page::Page, style},
};

pub const WIDTH: f32 = 208.0;

const PAD: f32 = 14.0;
const NAV_TOP: f32 = 72.0;
const NAV_HEIGHT: f32 = 36.0;
const NAV_GAP: f32 = 4.0;
const ICON: f32 = 18.0;

#[view]
pub struct Sidebar {
    pub selected: Event<Page>,

    items: Vec<Weak<NavItem>>,

    #[init]
    brand: Label,
    line: Container,
    run: Button,
}

impl Setup for Sidebar {
    fn setup(mut self: Weak<Self>) {
        self.set_color(colors::SIDEBAR_BG);

        style::title(self.brand, "Blackforge");
        self.brand.set_text_size(19);
        self.brand.place().t(22).l(PAD + 8.0).r(PAD).h(28);

        self.line.set_color(colors::BORDER);
        self.line.place().t(0).b(0).r(0).w(1);

        let mut y = NAV_TOP;
        for page in Page::ALL {
            let item = self.add_view::<NavItem>();
            item.set_page(page);
            item.place().t(y).l(PAD).r(PAD).h(NAV_HEIGHT);
            item.tapped.sub(move || self.selected.trigger(page));
            self.items.push(item);
            y += NAV_HEIGHT + NAV_GAP;
        }

        style::primary(self.run, "Run game");
        self.run.set_text_size(14);
        self.run.place().b(PAD + 4.0).l(PAD).r(PAD).h(40);
        self.run.on_tap(launcher::run_game);
    }
}

impl Sidebar {
    pub fn select(self: Weak<Self>, selected: Page) {
        for item in &self.items {
            item.set_selected(item.page == selected);
        }
    }
}

/// One entry of the page list: the icon, the title, a hover wash and an
/// accent state for the open page.
#[view]
struct NavItem {
    tapped: Event,

    page: Page,
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
    fn set_page(mut self: Weak<Self>, page: Page) {
        self.page = page;
        self.icon.set_image(page.icon());
        self.label.set_text(page.title());
    }

    fn set_selected(mut self: Weak<Self>, selected: bool) {
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
