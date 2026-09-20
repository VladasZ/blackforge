//! Embedded headings, body text and download metadata fonts.

use std::{cell::RefCell, collections::BTreeMap};

use hilen::{refs::Weak, ui::Font};

static INTER: &[u8] = include_bytes!("../fonts/Inter.ttf");
static MONO: &[u8] = include_bytes!("../fonts/JetBrainsMono.ttf");
static CINZEL: &[u8] = include_bytes!("../fonts/Cinzel.ttf");

thread_local! {
    static CACHE: RefCell<BTreeMap<String, Weak<Font>>> = RefCell::default();
}

fn font(family: &str, data: &'static [u8], weight: f32) -> Weak<Font> {
    let name = format!("{family}-{weight}");

    CACHE.with(|cache| {
        if let Some(font) = cache.borrow().get(&name) {
            return *font;
        }

        let font = Font::with_variations(&name, data, &[(*b"wght", weight)])
            .expect("Failed to load a bundled font");

        cache.borrow_mut().insert(name, font);

        font
    })
}

pub fn inter(weight: f32) -> Weak<Font> {
    font("inter", INTER, weight)
}

pub fn mono(weight: f32) -> Weak<Font> {
    font("mono", MONO, weight)
}

pub fn heading(weight: f32) -> Weak<Font> {
    font("cinzel", CINZEL, weight)
}
