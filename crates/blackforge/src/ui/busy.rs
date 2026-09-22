//! While a change of the profile runs, every button that starts one is off,
//! and the pressed one says what it does next to a turning ring. Only one
//! change runs at a time, see `backend::change`.

use std::cell::RefCell;

use hilen::{
    refs::Weak,
    ui::{Button, RingSpinner, ViewData, ViewSubviews},
};

use crate::{
    backend,
    ui::{colors, icon_button::IconButton},
};

const SPINNER: f32 = 14.0;
const SPINNER_LEFT: f32 = 10.0;

#[derive(Clone, Copy)]
enum Tracked {
    Button(Weak<Button>),
    Icon(Weak<IconButton>),
}

impl Tracked {
    fn is_ok(self) -> bool {
        match self {
            Self::Button(button) => button.is_ok(),
            Self::Icon(icon) => icon.is_ok(),
        }
    }

    fn set_enabled(self, enabled: bool) {
        match self {
            Self::Button(mut button) => {
                button.set_enabled(enabled);
            }
            Self::Icon(icon) => icon.set_enabled(enabled),
        }
    }
}

struct Pressed {
    button: Weak<Button>,
    text: String,
    spinner: Weak<RingSpinner>,
}

thread_local! {
    static TRACKED: RefCell<Vec<Tracked>> = const { RefCell::new(Vec::new()) };
    static PRESSED: RefCell<Option<Pressed>> = const { RefCell::new(None) };
}

/// `button` starts a change of the profile, so it is off while one runs.
/// Call it once, where the button is set up.
pub fn track(button: Weak<Button>) {
    add(Tracked::Button(button));
}

pub fn track_icon(button: Weak<IconButton>) {
    add(Tracked::Icon(button));
}

fn add(tracked: Tracked) {
    TRACKED.with(|all| {
        let mut all = all.borrow_mut();
        all.retain(|tracked| tracked.is_ok());
        all.push(tracked);
    });
    if backend::busy() {
        tracked.set_enabled(false);
    }
}

/// `button` was pressed and its change is about to start. It shows
/// `working`, like "Adding...", with a ring until the change ends.
pub fn press(button: Weak<Button>, working: &str) {
    if backend::busy() {
        return;
    }
    let text = button.text().to_owned();
    button.set_text(working);
    let mut spinner = button.add_view::<RingSpinner>();
    spinner.set_ring_color(colors::DIM);
    spinner
        .place()
        .l(SPINNER_LEFT)
        .center_y()
        .size(SPINNER, SPINNER);
    PRESSED.with(|pressed| {
        *pressed.borrow_mut() = Some(Pressed {
            button,
            text,
            spinner,
        });
    });
}

/// A change started or ended, `backend::change` calls this.
pub fn changed(busy: bool) {
    let tracked: Vec<Tracked> = TRACKED.with(|all| {
        let mut all = all.borrow_mut();
        all.retain(|tracked| tracked.is_ok());
        all.clone()
    });
    for tracked in tracked {
        tracked.set_enabled(!busy);
    }
    if busy {
        return;
    }
    // A table can reuse the cell of the pressed button for another row
    // meanwhile. The page reads its rows again after every change, so the
    // text put back here is replaced right after in that case.
    if let Some(pressed) = PRESSED.with(|pressed| pressed.borrow_mut().take()) {
        let mut spinner = pressed.spinner;
        if spinner.is_ok() {
            spinner.remove_from_superview();
        }
        let button = pressed.button;
        if button.is_ok() {
            button.set_text(pressed.text);
        }
    }
}
