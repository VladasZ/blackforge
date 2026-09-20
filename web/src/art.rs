//! Textures and ornaments, drawn as svg and embedded in the binary, so the
//! page downloads no assets. A themed picture is built from a template by
//! replacing its color tokens, one image per theme.
//!
//! The texture templates declare a tiny `width` and `height` over a large
//! `viewBox`. The engine also rasterizes every svg once at eight times its
//! declared size, and at the real tile size that pass alone costs seconds of
//! noise filters. An `ImageView` still draws the tile at its exact size.

use hilen::{
    gm::{LossyConvert, color::Color},
    refs::{Weak, manage::DataManager},
    ui::{Image, Theme},
};
use noise::{Fbm, MultiFractal, NoiseFn, Perlin};

static SPECKS: &str = include_str!("../art/specks.svg");
static WOOD: &str = include_str!("../art/wood.svg");
static ORNAMENT: &str = include_str!("../art/ornament.svg");
static BRACKET: &[u8] = include_bytes!("../art/bracket.svg");

/// The ground picture covers this many points. A smaller one shows up as a
/// pattern on a wide screen, this one is wider than most screens.
pub const GROUND_TILE: (f32, f32) = (2048.0, 2048.0);
/// The ground picture is this many pixels on a side. Its shapes are soft, so
/// a few points per pixel are enough, the crisp part is the `specks` tile.
const GROUND_SIDE: u32 = 640;
pub const SPECKS_TILE: (f32, f32) = (256.0, 256.0);
/// Two planks of 50 points each.
pub const WOOD_TILE: (f32, f32) = (512.0, 100.0);

/// One layer of stains. The numbers are the ones the page was first drawn
/// with as an svg turbulence filter: the waves per point, the octaves, and
/// how the noise turns into cover.
struct Stains {
    color: Color,
    opacity: f32,
    frequency: f64,
    octaves: usize,
    /// An svg turbulence sums raw octaves, this crate scales the sum to one.
    /// The factor brings the value back to the svg range.
    swing: f32,
    gain: f32,
    offset: f32,
}

impl Stains {
    fn dark(color: &str, opacity: f32) -> Self {
        Self {
            color: Color::hex(color),
            opacity,
            frequency: 0.0078,
            octaves: 4,
            swing: 1.326,
            gain: 1.9,
            offset: -0.7,
        }
    }

    fn light(color: &str, opacity: f32) -> Self {
        Self {
            color: Color::hex(color),
            opacity,
            frequency: 0.0195,
            octaves: 2,
            swing: 1.06,
            gain: 1.5,
            offset: -0.55,
        }
    }

    fn noise(&self, seed: u32) -> Fbm<Perlin> {
        Fbm::<Perlin>::new(seed)
            .set_frequency(self.frequency)
            .set_octaves(self.octaves)
    }

    /// How much of the layer covers the point, 0 to `opacity`.
    fn cover(&self, noise: &Fbm<Perlin>, x: f64, y: f64) -> f32 {
        let value: f32 = noise.get([x, y]).lossy_convert();
        let level = (self.swing * value + 1.0) / 2.0;

        (self.gain * level + self.offset).clamp(0.0, 1.0) * self.opacity
    }
}

fn themed(name: &str, svg: impl FnOnce(Theme) -> String) -> Weak<Image> {
    let theme = Theme::current();
    let key = format!("{name}-{theme:?}");

    Image::get_existing(&key).unwrap_or_else(|| Image::load(svg(theme).as_bytes(), key))
}

/// The page itself, parchment by day and sooty stone by night: dark stains
/// and light patches over a base color. It is painted here and not drawn as
/// an svg tile. A tile big enough to hide its repeats takes most of a second
/// of noise filters at page load, this takes a fraction of that.
pub fn ground() -> Weak<Image> {
    let theme = Theme::current();
    let key = format!("ground-{theme:?}");

    if let Some(image) = Image::get_existing(&key) {
        return image;
    }

    let (base, dark, light) = match theme {
        Theme::Light => (
            Color::hex("#e6d8b8"),
            Stains::dark("#b89a62", 0.55),
            Stains::light("#f7efd9", 0.35),
        ),
        Theme::Dark => (
            Color::hex("#17130f"),
            Stains::dark("#000000", 0.5),
            Stains::light("#3a2c1c", 0.4),
        ),
    };

    let (dark_noise, light_noise) = (dark.noise(11), light.noise(29));
    let points_per_pixel = f64::from(GROUND_TILE.0) / f64::from(GROUND_SIDE);
    let mut pixels = Vec::new();

    for y in 0..GROUND_SIDE {
        for x in 0..GROUND_SIDE {
            let (x, y) = (
                f64::from(x) * points_per_pixel,
                f64::from(y) * points_per_pixel,
            );
            let dark_cover = dark.cover(&dark_noise, x, y);
            let light_cover = light.cover(&light_noise, x, y);

            for (base, dark, light) in [
                (base.r, dark.color.r, light.color.r),
                (base.g, dark.color.g, light.color.g),
                (base.b, dark.color.b, light.color.b),
            ] {
                let stained = mix(base, dark, dark_cover);
                let lit = mix(stained, light, light_cover);

                pixels.push((lit * 255.0).round().lossy_convert());
            }
            pixels.push(255);
        }
    }

    Image::from_raw_data(pixels, key, (GROUND_SIDE, GROUND_SIDE).into(), 4)
}

fn mix(from: f32, to: f32, cover: f32) -> f32 {
    from + (to - from) * cover
}

/// Fine specks over the ground. They repeat, which nobody can see at their
/// size, and they stay sharp where the ground picture is soft.
pub fn specks() -> Weak<Image> {
    themed("specks", |theme| {
        let (speck, opacity) = match theme {
            Theme::Light => ("#7a5c2e", "0.3"),
            Theme::Dark => ("#000000", "0.4"),
        };

        SPECKS
            .replace("SPECK_OPACITY", opacity)
            .replace("SPECK", speck)
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
