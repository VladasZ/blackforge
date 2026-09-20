//! Short messages that stack at the top of the window and leave on their
//! own, so a result never blocks the page it reports on.

use std::cell::Cell;

use hilen::{
    dispatch::{after, on_main},
    gm::color::Color,
    refs::{Own, Weak},
    ui::{Container, Label, Setup, ViewData, ViewFrame, ViewSubviews, view},
};

use crate::ui::{colors, hint::describe, style};

/// A lower z draws in front and the engine keeps modals at 0.4, so a toast
/// stays readable while a modal is still on screen.
const Z: f32 = 0.35;

const LIFETIME: f32 = 4.0;
const HEIGHT: f32 = 36.0;
const GAP: f32 = 8.0;
const TOP: f32 = 14.0;
const PAD: f32 = 12.0;
const ACCENT_WIDTH: f32 = 3.0;
const TEXT_LEFT: f32 = PAD + ACCENT_WIDTH + 10.0;
const MIN_WIDTH: f32 = 200.0;
const SIDE_MARGIN: f32 = 16.0;

thread_local! {
    static HOST: Cell<Weak<ToastHost>> = const { Cell::new(Weak::const_default()) };
}

pub fn success(text: impl Into<String>) {
    let text = text.into();
    log::info!("toast: {text}");
    show(text, colors::OK);
}

pub fn info(text: impl Into<String>) {
    let text = text.into();
    log::info!("toast: {text}");
    show(text, colors::ACCENT);
}

pub fn error(text: impl Into<String>) {
    let text = text.into();
    log::error!("toast: {text}");
    show(text, colors::BAD);
}

/// A failed core operation, with the way out when the core knows one.
pub fn failure(error: &anyhow::Error) {
    self::error(describe(error));
}

/// The engine only accepts a custom z before a view joins a superview, so
/// the host is built here and handed to the shell already lifted.
pub fn host() -> Own<ToastHost> {
    let mut host = ToastHost::new();
    host.set_z_position(Z);
    host
}

pub fn register(host: Weak<ToastHost>) {
    HOST.with(|slot| slot.set(host));
}

fn show(text: String, accent: Color) {
    on_main(move || {
        let host = HOST.with(Cell::get);
        if host.is_ok() {
            host.push(&text, accent);
        }
    });
}

#[view]
pub struct ToastHost {
    toasts: Vec<Weak<Container>>,
}

impl Setup for ToastHost {
    fn setup(self: Weak<Self>) {
        self.set_color(colors::CLEAR);
        self.size_changed().sub(move || self.relayout());
    }
}

impl ToastHost {
    fn push(mut self: Weak<Self>, text: &str, accent: Color) {
        let card = self.add_view::<Container>();
        style::card(card);

        let accent_bar = card.add_view::<Container>();
        accent_bar.set_color(accent);
        accent_bar.set_corner_radius(ACCENT_WIDTH / 2.0);
        accent_bar
            .place()
            .l(PAD)
            .center_y()
            .size(ACCENT_WIDTH, HEIGHT - 14.0);

        let label = card.add_view::<Label>();
        style::body(label);
        label.set_text(text).set_text_size(13);
        label.place().l(TEXT_LEFT).r(PAD).t(0).b(0);

        card.set_frame((
            0.0,
            0.0,
            (TEXT_LEFT + label.content_size().width + PAD).max(MIN_WIDTH),
            HEIGHT,
        ));

        self.toasts.push(card);
        self.relayout();

        after(LIFETIME, move || {
            if self.is_ok() && card.is_ok() {
                self.dismiss(card);
            }
        });
    }

    fn dismiss(mut self: Weak<Self>, mut card: Weak<Container>) {
        self.toasts.retain(|toast| toast.raw() != card.raw());
        card.remove_from_superview();
        self.relayout();
    }

    fn relayout(self: Weak<Self>) {
        let host_width = self.frame().size.width;

        // Layout has not run yet, so every width would come out negative.
        if host_width < SIDE_MARGIN * 2.0 {
            return;
        }

        let mut y = TOP;
        for card in &self.toasts {
            let width = card.frame().size.width.min(host_width - SIDE_MARGIN * 2.0);
            let x = ((host_width - width) / 2.0).max(SIDE_MARGIN);
            card.set_frame((x, y, width, HEIGHT));
            y += HEIGHT + GAP;
        }
    }
}
