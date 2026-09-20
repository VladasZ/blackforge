//! The icon of one mod. A neutral square stands in until the picture is here.

use blackforge_core::ident::VersionedId;
use hilen::{
    refs::{Weak, manage::DataManager},
    ui::{Image, ImageView, Setup, ViewData, view},
};

use crate::{icons, ui::colors};

const CHANNELS: u8 = 4;

#[view]
pub struct ModIcon {
    /// `Owner-Name-1.2.3` of the icon to show now. A table row is reused
    /// while the user scrolls, so a picture that arrives late is checked
    /// against this before it is shown.
    wanted: String,

    #[init]
    picture: ImageView,
}

impl Setup for ModIcon {
    fn setup(self: Weak<Self>) {
        self.set_color(colors::PILL_BG);
        self.set_corner_radius(8);
        self.picture.set_corner_radius(8);
        self.picture.place().back();
    }
}

impl ModIcon {
    /// Back to the neutral square, for a reused row that has no mod.
    pub fn clear(mut self: Weak<Self>) {
        self.wanted.clear();
        self.picture.set_hidden(true);
    }

    pub fn show(mut self: Weak<Self>, id: &str, version: &str) {
        let key = format!("{id}-{version}");
        if self.wanted == key {
            return;
        }
        self.wanted.clone_from(&key);

        // Every icon shown once stays in memory under its key.
        if let Some(image) = Image::get_existing(&key) {
            self.picture.set_image(image);
            self.picture.set_hidden(false);
            return;
        }
        self.picture.set_hidden(true);

        let package: VersionedId = match key.parse() {
            Ok(package) => package,
            Err(error) => {
                log::warn!("no icon for {key}: {error}");
                return;
            }
        };
        icons::load(package, move |result| {
            if !self.is_ok() || self.wanted != key {
                return;
            }
            match result {
                Ok(pixels) => {
                    let image = Image::from_raw_data(
                        pixels.rgba,
                        &key,
                        (pixels.side, pixels.side).into(),
                        CHANNELS,
                    );
                    self.picture.set_image(image);
                    self.picture.set_hidden(false);
                }
                Err(error) => log::warn!("no icon for {key}: {error:#}"),
            }
        });
    }
}
