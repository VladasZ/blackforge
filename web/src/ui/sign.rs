//! The signboard at the top of the page, the name in raised brass letters.

use hilen::{
    refs::Weak,
    ui::{Label, Setup, ViewData, ViewFrame, view},
};

use crate::{
    fonts,
    ui::{colors, embossed::Embossed, plaque::Plaque},
};

const EYEBROW_Y: f32 = 34.0;
const TITLE_Y: f32 = 62.0;
/// Cinzel capitals are wide, ten of them take about this many text sizes.
const TITLE_WIDTH_IN_SIZES: f32 = 7.4;

#[view]
pub struct Sign {
    face: Weak<Face>,

    #[init]
    board: Plaque,
}

impl Setup for Sign {
    fn setup(mut self: Weak<Self>) {
        self.board.place().back();
        self.face = self.board.add_face::<Face>();
    }
}

impl Sign {
    /// Sizes the title for this width and returns the height of the board.
    pub fn height_for_width(self: Weak<Self>, width: f32) -> f32 {
        let size = ((width - 72.0) / TITLE_WIDTH_IN_SIZES).clamp(24.0, 84.0);
        let title_height = size * 1.3;

        self.face.title.each(|label| {
            label.set_text_size(size);
        });
        self.face.eyebrow.set_frame((0.0, EYEBROW_Y, width, 20.0));
        self.face
            .title
            .set_frame((0.0, TITLE_Y, width, title_height));

        TITLE_Y + title_height + 34.0
    }
}

#[view]
struct Face {
    #[init]
    eyebrow: Label,
    title: Embossed,
}

impl Setup for Face {
    fn setup(self: Weak<Self>) {
        self.eyebrow
            .set_color(colors::CLEAR)
            .set_font(fonts::heading(600.0))
            .set_text_size(12)
            .set_letter_spacing(4.0)
            .set_text_color(colors::BRASS_DIM)
            .set_text("VALHEIM / MOD MANAGER");

        self.title
            .set_text("Blackforge")
            .each(|label| {
                label
                    .set_font(fonts::heading(800.0))
                    .set_letter_spacing(2.0);
            })
            .set_depth(2.5)
            .set_shade(colors::WOOD_TEXT_SHADOW)
            .set_light(colors::WOOD_TEXT_LIGHT);
        self.title
            .face()
            .set_text_gradient(colors::BRASS_TOP, colors::BRASS_BOTTOM);
    }
}
