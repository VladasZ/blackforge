//! Textures and ornaments, drawn as svg and embedded in the binary, so the
//! page downloads no assets. A themed picture is built from a template by
//! replacing its color tokens, one image per theme.
//!
//! The texture templates declare a tiny `width` and `height` over a large
//! `viewBox`. The engine also rasterizes every svg once at eight times its
//! declared size, and at the real tile size that pass alone costs seconds of
//! noise filters. An `ImageView` still draws the tile at its exact size.

use hilen::{
    refs::{Weak, manage::DataManager},
    ui::{Image, Theme},
};

static SHEET: &str = include_str!("../art/sheet.svg");
static WOOD: &str = include_str!("../art/wood.svg");
static ORNAMENT: &str = include_str!("../art/ornament.svg");
static BRACKET: &[u8] = include_bytes!("../art/bracket.svg");

/// Side of the square `ground` tile, in points.
pub const GROUND_TILE: (f32, f32) = (256.0, 256.0);
/// Two planks of 50 points each.
pub const WOOD_TILE: (f32, f32) = (512.0, 100.0);

struct Sheet {
    base: &'static str,
    stain: &'static str,
    stain_opacity: &'static str,
    glow: &'static str,
    glow_opacity: &'static str,
    speck: &'static str,
    speck_opacity: &'static str,
}

fn themed(name: &str, svg: impl FnOnce(Theme) -> String) -> Weak<Image> {
    let theme = Theme::current();
    let key = format!("{name}-{theme:?}");

    Image::get_existing(&key).unwrap_or_else(|| Image::load(svg(theme).as_bytes(), key))
}

fn sheet(sheet: &Sheet) -> String {
    SHEET
        .replace("BASE", sheet.base)
        .replace("STAIN_OPACITY", sheet.stain_opacity)
        .replace("GLOW_OPACITY", sheet.glow_opacity)
        .replace("SPECK_OPACITY", sheet.speck_opacity)
        .replace("STAIN", sheet.stain)
        .replace("GLOW", sheet.glow)
        .replace("SPECK", sheet.speck)
}

/// The page itself, parchment by day and sooty stone by night.
pub fn ground() -> Weak<Image> {
    themed("ground", |theme| {
        sheet(&match theme {
            Theme::Light => Sheet {
                base: "#e6d8b8",
                stain: "#b89a62",
                stain_opacity: "0.55",
                glow: "#f7efd9",
                glow_opacity: "0.35",
                speck: "#7a5c2e",
                speck_opacity: "0.3",
            },
            Theme::Dark => Sheet {
                base: "#17130f",
                stain: "#000000",
                stain_opacity: "0.5",
                glow: "#3a2c1c",
                glow_opacity: "0.4",
                speck: "#000000",
                speck_opacity: "0.4",
            },
        })
    })
}

pub fn wood() -> Weak<Image> {
    themed("wood", |theme| {
        let (base, dark, light, seam) = match theme {
            Theme::Light => ("#5d3c1f", "#2a1708", "#8d6236", "#170b03"),
            Theme::Dark => ("#3b2717", "#170d06", "#6a4a2c", "#0d0704"),
        };

        WOOD.replace("BASE", base)
            .replace("DARK", dark)
            .replace("LIGHT", light)
            .replace("SEAM", seam)
    })
}

pub fn ornament() -> Weak<Image> {
    themed("ornament", |theme| {
        let (ink, shade) = match theme {
            Theme::Light => ("#6b4a22", "#fff8e6"),
            Theme::Dark => ("#b58d4a", "#000000"),
        };

        ORNAMENT.replace("INK", ink).replace("SHADE", shade)
    })
}

pub fn bracket() -> Weak<Image> {
    Image::load(BRACKET, "bracket")
}
