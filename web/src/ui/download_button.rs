//! A download board: planks with iron corners, the system in brass letters
//! and a brass plate that names the version. The whole board is the button.

use hilen::{
    refs::Weak,
    ui::{
        Container, Label, Setup, Shadow, TextAlignment, UIEvent, ViewData, ViewFrame, ViewTouch,
        view,
    },
};

use crate::{
    fonts,
    ui::{colors, embossed::Embossed, plaque::Plaque, text_margin},
};

/// Three planks.
pub const HEIGHT: f32 = 150.0;

const PAD: f32 = 28.0;
const PLATE_HEIGHT: f32 = 34.0;

#[view]
pub struct DownloadButton {
    pub on_tap: UIEvent,
    enabled: bool,
    face: Weak<Face>,

    #[init]
    board: Plaque,
}

impl Setup for DownloadButton {
    fn setup(mut self: Weak<Self>) {
        self.board.place().back();
        self.face = self.board.add_face::<Face>();

        self.enable_touch();
        self.enable_hover();
        self.touch().hovered.val(self, move |hovered| {
            self.board.set_hovered(hovered && self.enabled);
            self.face.set_plate(self.enabled, hovered);
        });
        self.touch().up_inside.sub(self, move || {
            if self.enabled {
                self.on_tap.trigger(());
            }
        });
    }
}

impl DownloadButton {
    pub fn set_data(mut self: Weak<Self>, os: &str, sub: &str, version: &str) {
        self.enabled = !version.is_empty();
        self.face.os.set_text(os);
        self.face.sub.set_text(sub);
        self.face.action.set_text(&if self.enabled {
            format!("Download v{version}")
        } else {
            "No release yet".to_string()
        });
        self.face.set_plate(self.enabled, self.is_hovered());
    }
}

#[view]
struct Face {
    #[init]
    os: Embossed,
    sub: Label,
    plate: Container,
    action: Embossed,
}

impl Setup for Face {
    fn setup(self: Weak<Self>) {
        self.os
            .each(|label| {
                label
                    .set_alignment(TextAlignment::Left)
                    .set_font(fonts::heading(700.0))
                    .set_text_size(23)
                    .set_letter_spacing(1.0);
            })
            .set_shade(colors::WOOD_TEXT_SHADOW)
            .set_light(colors::WOOD_TEXT_LIGHT);
        self.os
            .face()
            .set_text_gradient(colors::BRASS_TOP, colors::BRASS_BOTTOM);

        self.sub
            .set_color(colors::CLEAR)
            .set_alignment(TextAlignment::Left)
            .set_font(fonts::inter(500.0))
            .set_text_size(12.5)
            .set_text_color(colors::BRASS_DIM);

        self.plate.set_border_width(1).set_corner_radius(3);

        self.action.each(|label| {
            label
                .set_font(fonts::heading(700.0))
                .set_text_size(13)
                .set_letter_spacing(1.5);
        });

        self.set_plate(false, false);
        self.size_changed().sub(move || self.relayout());
    }
}

impl Face {
    /// A brass plate invites a press. With no release it is a dull iron one.
    fn set_plate(self: Weak<Self>, enabled: bool, hovered: bool) {
        let (top, bottom, edge, text, shade) = match (enabled, hovered) {
            (true, false) => (
                colors::PLATE_TOP,
                colors::PLATE_BOTTOM,
                colors::PLATE_EDGE,
                colors::PLATE_TEXT,
                colors::PLATE_TEXT_LIGHT,
            ),
            (true, true) => (
                colors::PLATE_HOVER_TOP,
                colors::PLATE_HOVER_BOTTOM,
                colors::PLATE_EDGE,
                colors::PLATE_TEXT,
                colors::PLATE_TEXT_LIGHT,
            ),
            (false, _) => (
                colors::PLATE_OFF_TOP,
                colors::PLATE_OFF_BOTTOM,
                colors::PLATE_OFF_EDGE,
                colors::PLATE_OFF_TEXT,
                colors::PLATE_OFF_TEXT_SHADOW,
            ),
        };

        self.plate.set_gradient(top, bottom).set_border_color(edge);
        self.plate.set_shadow(Shadow {
            offset: (0, 2).into(),
            radius: 5.0,
            color: colors::WOOD_TEXT_SHADOW,
        });
        self.action.face().set_text_color(text);
        self.action.set_shade(shade);
    }

    fn relayout(self: Weak<Self>) {
        let x = PAD - text_margin();
        let width = (self.width() - PAD * 2.0).max(0.0);
        let text_width = width + text_margin();
        let plate_y = HEIGHT - PAD + 4.0 - PLATE_HEIGHT;

        self.os.set_frame((x, 22.0, text_width, 32.0));
        self.sub.set_frame((x, 56.0, text_width, 20.0));
        self.plate.set_frame((PAD, plate_y, width, PLATE_HEIGHT));
        self.action.set_frame((PAD, plate_y, width, PLATE_HEIGHT));
    }
}
