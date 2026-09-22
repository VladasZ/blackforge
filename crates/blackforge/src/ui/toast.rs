//! Short messages that stack at the top of the window and leave on their
//! own, so a result never blocks the page it reports on. An error stays
//! longer and wraps, so its whole text can be read.

use std::cell::Cell;

use hilen::{
    dispatch::{after, on_main},
    gm::color::Color,
    refs::{Own, Weak},
    ui::{Container, Label, Setup, VerticalAlignment, ViewData, ViewFrame, ViewSubviews, view},
};

use crate::ui::{colors, hint::describe, names, style};

/// A lower z draws in front and the engine keeps modals at 0.4, so a toast
/// stays readable while a modal is still on screen.
const Z: f32 = 0.35;

const LIFETIME: f32 = 4.0;
const ERROR_LIFETIME: f32 = 10.0;
const MIN_HEIGHT: f32 = 36.0;
/// Above and below the text of a toast that wraps.
const TEXT_PAD: f32 = 9.0;
const GAP: f32 = 8.0;
const TOP: f32 = 14.0;
const PAD: f32 = 12.0;
const ACCENT_WIDTH: f32 = 3.0;
const TEXT_LEFT: f32 = PAD + ACCENT_WIDTH + 10.0;
const MIN_WIDTH: f32 = 200.0;
/// A longer text wraps into more lines.
const MAX_WIDTH: f32 = 560.0;
const SIDE_MARGIN: f32 = 16.0;

thread_local! {
    static HOST: Cell<Weak<ToastHost>> = const { Cell::new(Weak::const_default()) };
}

pub fn success(text: impl Into<String>) {
    let text = text.into();
    log::info!("toast: {text}");
    show(text, colors::OK, LIFETIME);
}

pub fn info(text: impl Into<String>) {
    let text = text.into();
    log::info!("toast: {text}");
    show(text, colors::ACCENT, LIFETIME);
}

pub fn error(text: impl Into<String>) {
    let text = text.into();
    log::error!("toast: {text}");
    show(text, colors::BAD, ERROR_LIFETIME);
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

/// Texts from the core start in lower case, a toast starts a sentence.
fn show(text: String, accent: Color, lifetime: f32) {
    on_main(move || {
        let host = HOST.with(Cell::get);
        if host.is_ok() {
            host.push(&names::sentence(&text), accent, lifetime);
        }
    });
}

struct Toast {
    card: Weak<Container>,
    label: Weak<Label>,
}

#[view]
pub struct ToastHost {
    toasts: Vec<Toast>,
}

impl Setup for ToastHost {
    fn setup(self: Weak<Self>) {
        self.set_color(colors::CLEAR);
        self.size_changed().sub(move || self.relayout());
    }
}

impl ToastHost {
    fn push(mut self: Weak<Self>, text: &str, accent: Color, lifetime: f32) {
        let card = self.add_view::<Container>();
        style::card(card);

        let accent_bar = card.add_view::<Container>();
        accent_bar.set_color(accent);
        accent_bar.set_corner_radius(ACCENT_WIDTH / 2.0);
        accent_bar
            .place()
            .l(PAD)
            .t(TEXT_PAD)
            .b(TEXT_PAD)
            .w(ACCENT_WIDTH);

        let label = card.add_view::<Label>();
        style::body(label);
        label.set_text(text).set_text_size(13);
        label.set_multiline(true);
        label.set_vertical_alignment(VerticalAlignment::Center);
        label.place().l(TEXT_LEFT).r(PAD).t(0).b(0);

        self.toasts.push(Toast { card, label });
        self.relayout();

        after(lifetime, move || {
            if self.is_ok() && card.is_ok() {
                self.dismiss(card);
            }
        });
    }

    fn dismiss(mut self: Weak<Self>, mut card: Weak<Container>) {
        self.toasts.retain(|toast| toast.card.raw() != card.raw());
        card.remove_from_superview();
        self.relayout();
    }

    /// Every toast is as wide as its text up to the most the window allows,
    /// and as tall as the lines that width gives.
    fn relayout(self: Weak<Self>) {
        let host_width = self.frame().size.width;

        // Layout has not run yet, so every width would come out negative.
        if host_width < SIDE_MARGIN * 2.0 {
            return;
        }

        let widest = MAX_WIDTH.min(host_width - SIDE_MARGIN * 2.0);
        let mut y = TOP;
        for toast in &self.toasts {
            let text = toast.label.content_size().width;
            let width = (TEXT_LEFT + text + PAD).clamp(MIN_WIDTH.min(widest), widest);
            let lines = toast.label.size_for_width(width - TEXT_LEFT - PAD).height;
            let height = (lines + TEXT_PAD * 2.0).max(MIN_HEIGHT);
            let x = ((host_width - width) / 2.0).max(SIDE_MARGIN);
            toast.card.set_frame((x, y, width, height));
            y += height + GAP;
        }
    }
}
