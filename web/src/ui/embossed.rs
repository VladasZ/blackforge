//! A line of text with copies of itself under and over it. A dark copy one
//! point lower and a light one above make raised metal letters, a light copy
//! under dark ink makes letters pressed into a sheet.

use hilen::{
    refs::Weak,
    ui::{Label, Setup, UIColor, ViewData, ViewFrame, view},
};

use crate::ui::colors;

#[view]
pub struct Embossed {
    /// How far under the face the dark copy sits, in points.
    #[educe(Default = 1.0)]
    depth: f32,

    #[init]
    shade: Label,
    light: Label,
    face: Label,
}

impl Setup for Embossed {
    fn setup(self: Weak<Self>) {
        self.each(|label| {
            label.set_color(colors::CLEAR);
        });
        self.light.set_hidden(true);
        self.size_changed().sub(move || self.relayout());
    }
}

impl Embossed {
    /// Font, size, alignment and the text itself go to every copy.
    pub fn each(self: Weak<Self>, apply: impl Fn(Weak<Label>)) -> Weak<Self> {
        for label in [self.shade, self.light, self.face] {
            apply(label);
        }
        self
    }

    pub fn set_text(self: Weak<Self>, text: &str) -> Weak<Self> {
        self.each(|label| {
            label.set_text(text);
        })
    }

    pub fn face(self: Weak<Self>) -> Weak<Label> {
        self.face
    }

    pub fn set_shade(self: Weak<Self>, color: impl Into<UIColor>) -> Weak<Self> {
        self.shade.set_text_color(color);
        self
    }

    pub fn set_light(self: Weak<Self>, color: impl Into<UIColor>) -> Weak<Self> {
        self.light.set_text_color(color);
        self.light.set_hidden(false);
        self
    }

    pub fn set_depth(mut self: Weak<Self>, depth: f32) -> Weak<Self> {
        self.depth = depth;
        self.relayout();
        self
    }

    pub fn height_for_width(self: Weak<Self>, width: f32) -> f32 {
        self.face.size_for_width(width).height
    }

    fn relayout(self: Weak<Self>) {
        let (width, height) = (self.width(), self.height());

        self.shade.set_frame((0.0, self.depth, width, height));
        self.light
            .set_frame((0.0, -self.depth * 0.6, width, height));
        self.face.set_frame((0.0, 0.0, width, height));
    }
}
