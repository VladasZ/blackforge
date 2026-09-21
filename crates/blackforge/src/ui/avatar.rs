//! The Google picture of a person. A circle with the first letter of the
//! username stands in until the picture is here, and for somebody who has
//! none.

use hilen::{
    dispatch::{on_main, spawn},
    gm::color::Color,
    refs::{Weak, manage::DataManager},
    ui::{Container, Image, ImageView, Label, Setup, ViewData, view},
};

use crate::ui::colors;

/// The owner places the avatar as a square of this side.
pub const SIZE: f32 = 36.0;
const DOT: f32 = 12.0;

#[view]
pub struct Avatar {
    /// The link of the picture to show now. A table row is reused while the
    /// user scrolls, so a picture that arrives late is checked against this
    /// before it is shown.
    wanted: String,

    #[init]
    letter: Label,
    picture: ImageView,
    dot: Container,
}

impl Setup for Avatar {
    fn setup(self: Weak<Self>) {
        self.set_color(colors::NAV_ACTIVE_BG);
        self.set_corner_radius(SIZE / 2.0);

        self.letter.set_text_size(15).set_text_color(colors::ACCENT);
        self.letter.set_color(colors::CLEAR);
        self.letter.place().back();

        self.picture.set_corner_radius(SIZE / 2.0);
        self.picture.place().back();

        // The ring in the color of the page keeps the dot apart from the
        // picture under it.
        self.dot.set_corner_radius(DOT / 2.0);
        self.dot.set_border_color(colors::BG);
        self.dot.set_border_width(2);
        self.dot.place().r(0).b(0).size(DOT, DOT);
        self.dot.set_hidden(true);
    }
}

impl Avatar {
    pub fn show(mut self: Weak<Self>, username: &str, picture: Option<&str>) {
        let letter: String = username
            .chars()
            .take(1)
            .map(|letter| letter.to_ascii_uppercase())
            .collect();
        self.letter.set_text(letter);

        let link = picture.unwrap_or_default();
        if self.wanted == link {
            return;
        }
        link.clone_into(&mut self.wanted);
        self.picture.set_hidden(true);

        if link.is_empty() {
            return;
        }
        // Every picture shown once stays in memory under its link.
        if let Some(image) = Image::get_existing(link) {
            self.picture.set_image(image);
            self.picture.set_hidden(false);
            return;
        }

        let link = link.to_owned();
        spawn(async move {
            let image = Image::download(&link, &link).await;

            on_main(move || {
                if !self.is_ok() || self.wanted != link {
                    return;
                }
                match image {
                    Ok(image) => {
                        self.picture.set_image(image);
                        self.picture.set_hidden(false);
                    }
                    Err(error) => log::warn!("a profile picture was not loaded: {error:#}"),
                }
            });
        });
    }

    /// A small dot on the corner of the picture, `None` takes it away.
    pub fn set_status(self: Weak<Self>, color: Option<Color>) {
        if let Some(color) = color {
            self.dot.set_color(color);
        }
        self.dot.set_hidden(color.is_none());
    }
}
