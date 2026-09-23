//! The left column: the brand, one entry per page and the button that starts
//! the game.

use std::cell::Cell;

use hilen::{
    Event,
    refs::Weak,
    ui::{Button, Container, Label, Setup, ViewData, ViewSubviews, view},
};

use crate::{
    launcher::{self, Run},
    ui::{
        colors,
        nav_item::{Badge, NavItem},
        page::Page,
        style,
    },
};

pub const WIDTH: f32 = 208.0;

const PAD: f32 = 14.0;
const NAV_TOP: f32 = 72.0;
const NAV_HEIGHT: f32 = 36.0;
const NAV_GAP: f32 = 4.0;

thread_local! {
    static SIDEBAR: Cell<Weak<Sidebar>> = const { Cell::new(Weak::const_default()) };
}

fn sidebar() -> Option<Weak<Sidebar>> {
    let sidebar = SIDEBAR.with(Cell::get);
    sidebar.is_ok().then_some(sidebar)
}

/// Shows what waits on `page` next to its entry.
pub fn set_badge(page: Page, badge: Badge) {
    if let Some(sidebar) = sidebar() {
        for (item_page, item) in &sidebar.items {
            if *item_page == page {
                item.set_badge(badge);
            }
        }
    }
}

/// The run button follows the launcher.
pub fn show_run(run: Run) {
    if let Some(sidebar) = sidebar() {
        sidebar.show_run(run);
    }
}

#[view]
pub struct Sidebar {
    pub selected: Event<Page>,

    items: Vec<(Page, Weak<NavItem>)>,

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
            item.set_content(page.icon(), page.title());
            item.place().t(y).l(PAD).r(PAD).h(NAV_HEIGHT);
            item.tapped.sub(move || self.selected.trigger(page));
            self.items.push((page, item));
            y += NAV_HEIGHT + NAV_GAP;
        }

        self.run.set_text_size(14);
        self.run.place().b(PAD + 4.0).l(PAD).r(PAD).h(40);
        self.run.on_tap(launcher::run_game);

        SIDEBAR.with(|slot| slot.set(self));
        self.show_run(launcher::run());
    }
}

impl Sidebar {
    pub fn select(self: Weak<Self>, selected: Page) {
        for (page, item) in &self.items {
            item.set_selected(*page == selected);
        }
    }

    fn show_run(self: Weak<Self>, run: Run) {
        let mut button = self.run;
        let waiting = run == Run::Idle && launcher::steam_missing();
        let enabled = run == Run::Idle && !waiting;
        // Before the style. Turning the button back on puts back the colors it
        // had when it was turned off, which would undo the style.
        button.set_enabled(enabled);
        let text = match run {
            Run::Idle if waiting => "Open Steam first",
            Run::Idle => "Run game",
            Run::Syncing => "Syncing...",
            Run::Starting => "Starting...",
            Run::Running => "Running",
        };
        if enabled {
            style::primary(button, text);
        } else {
            style::ghost(button, text);
            button.set_text_color(colors::DIM);
        }
        button.set_text_size(14);
    }
}
