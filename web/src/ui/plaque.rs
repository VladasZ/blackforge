//! A board of dark planks held by forged iron corners. The sign with the
//! title and the download boards are made of it.

use hilen::{
    refs::Weak,
    ui::{Container, ImageView, Setup, Shadow, UIEvents, View, ViewData, ViewSubviews, view},
};

use crate::{
    art,
    ui::{colors, textured::Textured},
};

const RADIUS: f32 = 4.0;
const BRACKET: f32 = 46.0;
/// The iron wraps the corner, so it sticks out of the board a little.
const BRACKET_OUT: f32 = -3.0;

#[view]
pub struct Plaque {
    #[init]
    wood: Textured,
    edge: Container,
    light: Container,
    wash: Container,
    top_left: ImageView,
    top_right: ImageView,
    bottom_left: ImageView,
    bottom_right: ImageView,
}

impl Setup for Plaque {
    fn setup(mut self: Weak<Self>) {
        self.set_corner_radius(RADIUS);

        self.wood.set_corner_radius(RADIUS);
        self.wood.set_shift(190.0);
        self.wood.place().back();

        self.edge
            .set_color(colors::CLEAR)
            .set_border_color(colors::WOOD_EDGE)
            .set_border_width(2)
            .set_corner_radius(RADIUS);
        self.edge.place().back();

        self.light.set_color(colors::WOOD_EDGE_LIGHT);
        self.light.place().lr(3).t(2).h(1);

        self.wash
            .set_color(colors::WOOD_HOVER)
            .set_corner_radius(RADIUS);
        self.wash.set_hidden(true);
        self.wash.place().back();

        self.top_right.flip_x = true;
        self.bottom_left.flip_y = true;
        self.bottom_right.flip_x = true;
        self.bottom_right.flip_y = true;

        for bracket in [
            self.top_left,
            self.top_right,
            self.bottom_left,
            self.bottom_right,
        ] {
            bracket.set_image(art::bracket());
        }

        self.top_left.place().tl(BRACKET_OUT).size(BRACKET, BRACKET);
        self.top_right
            .place()
            .tr(BRACKET_OUT)
            .size(BRACKET, BRACKET);
        self.bottom_left
            .place()
            .bl(BRACKET_OUT)
            .size(BRACKET, BRACKET);
        self.bottom_right
            .place()
            .br(BRACKET_OUT)
            .size(BRACKET, BRACKET);

        self.restyle();
        UIEvents::theme_changed().sub(self, move || self.restyle());
    }
}

impl Plaque {
    /// What is written on the board. It is added last, so it sits in front
    /// of the planks and the iron at any nesting depth.
    pub fn add_face<V: 'static + View + Default>(self: Weak<Self>) -> Weak<V> {
        let face = self.add_view::<V>();
        face.place().back();
        face
    }

    pub fn set_hovered(self: Weak<Self>, hovered: bool) {
        self.wash.set_hidden(!hovered);
        self.set_shadow(shadow(hovered));
    }

    // A texture and a shadow hold plain colors, the engine re-resolves only
    // view and text colors on a theme switch.
    fn restyle(self: Weak<Self>) {
        self.set_color(colors::WOOD_BASE);
        self.wood
            .set_texture(art::wood(), art::WOOD_TILE, (true, false));
        self.set_shadow(shadow(false));
    }
}

fn shadow(lifted: bool) -> Shadow {
    Shadow {
        offset: (0, if lifted { 10 } else { 6 }).into(),
        radius: if lifted { 22.0 } else { 14.0 },
        color: colors::SHADOW.resolve(),
    }
}
