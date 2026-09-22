//! What the pointer shows over things that take a click: a hand cursor on all
//! of them, a hover fill on a button, a light fill behind a row.

use std::sync::Mutex;

use hilen::{
    gm::color::Color,
    refs::Weak,
    ui::{Button, CursorIcon, UIColor, View, ViewData, ViewTouch},
};

use crate::ui::colors;

pub fn clickable(view: Weak<impl View + ?Sized>) {
    view.set_hover_cursor(CursorIcon::Pointer);
}

/// Swaps the fill for `lit` while the pointer rests on the button. A child
/// view laid over the button hid its text, so the fill itself changes.
/// Pages recolor some buttons while they show, so the old fill comes back
/// on exit only when nothing else recolored the button meanwhile.
pub fn button(button: Weak<Button>, lit: impl Into<UIColor> + Copy + Send + Sync + 'static) {
    clickable(button);
    let saved: Mutex<Option<(Color, Color)>> = Mutex::new(None);
    button.touch().hovered.val(button, move |hovered| {
        let mut saved = saved.lock().unwrap();
        if hovered {
            if button.is_enabled() {
                let idle = *button.color();
                button.set_color(lit);
                *saved = Some((idle, *button.color()));
            }
        } else if let Some((idle, shown)) = saved.take()
            && *button.color() == shown
        {
            button.set_color(idle);
        }
    });
}

/// A table row whose click opens the details of its item.
pub fn row(row: Weak<impl View + 'static>) {
    clickable(row);
    row.touch().hovered.val(row, move |hovered| {
        row.set_color(if hovered {
            colors::NAV_HOVER_BG.into()
        } else {
            UIColor::from(colors::CLEAR)
        });
    });
}
