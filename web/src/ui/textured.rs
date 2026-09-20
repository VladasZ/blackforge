//! A surface filled with one repeating picture, wood or parchment.

use hilen::{
    gm::LossyConvert,
    refs::{Weak, weak_from_ref},
    ui::{
        Image, ImageView, Setup, UIManager, ViewCallbacks, ViewData, ViewFrame, ViewSubviews, view,
    },
};

#[view]
pub struct Textured {
    image: Weak<Image>,
    tile: (f32, f32),
    /// Every other row starts this far to the left, so the grain of two
    /// planks does not line up.
    shift: f32,
    /// Every other tile is drawn mirrored along these axes. A mirrored
    /// neighbor always meets its tile without a seam, whatever the picture.
    mirror: (bool, bool),
    tiles: Vec<Weak<ImageView>>,
    /// What the tiles were last laid out for: the part of a pixel the view
    /// starts at, on both axes, and its size.
    laid: (f32, f32, f32, f32),
}

impl Setup for Textured {
    // The tiles run past the right and bottom edge and are cut here. A clip
    // is also a draw barrier: everything before it is on screen before the
    // tiles, and the tiles before anything after it. Views that blend over a
    // texture, a shade or a rivet, count on that order.
    fn clips_to_bounds(&self) -> bool {
        true
    }
}

impl ViewCallbacks for Textured {
    // The place on screen is known only here, after the layout pass.
    fn update(&mut self) {
        let origin = self.absolute_frame().origin;
        let scale = UIManager::scale();
        let state = (
            (origin.x * scale).rem_euclid(1.0),
            (origin.y * scale).rem_euclid(1.0),
            self.width(),
            self.height(),
        );

        if state != self.laid {
            self.laid = state;
            weak_from_ref(self).relayout();
        }
    }
}

impl Textured {
    pub fn set_texture(
        mut self: Weak<Self>,
        image: Weak<Image>,
        tile: (f32, f32),
        mirror: (bool, bool),
    ) {
        self.image = image;
        self.tile = tile;
        self.mirror = mirror;

        for tile in &self.tiles {
            tile.set_image(image);
        }

        self.relayout();
    }

    pub fn set_shift(mut self: Weak<Self>, shift: f32) {
        self.shift = shift;
        self.relayout();
    }

    // The edge of a picture is smoothed, so a tile edge that falls inside a
    // pixel leaves a faint line there. Every tile covers a whole number of
    // pixels and starts on a pixel, at any display scale and any position.
    fn relayout(mut self: Weak<Self>) {
        if self.tile.0 < 1.0 || self.tile.1 < 1.0 || self.width() < 1.0 {
            return;
        }

        let scale = UIManager::scale();
        let (start_x, start_y, ..) = self.laid;
        let tile_width = (self.tile.0 * scale).round() / scale;
        let tile_height = (self.tile.1 * scale).round() / scale;
        let shift = (self.shift * scale).round() / scale;
        let left = -start_x / scale;
        let top = -start_y / scale;

        let rows: usize = ((self.height() - top) / tile_height).ceil().lossy_convert();
        let columns: usize = ((self.width() - left + shift) / tile_width)
            .ceil()
            .lossy_convert();

        while self.tiles.len() < rows * columns {
            // A tile sits at the depth of the surface itself and not one
            // level in front of it, so the views laid over the surface, its
            // later siblings, are in front of the tiles too.
            let mut tile = ImageView::new();
            tile.set_z_position(self.z_position());

            let tile = self.add_subview(tile);
            tile.set_image(self.image);
            self.tiles.push(tile);
        }

        let (mirror_x, mirror_y) = self.mirror;

        for (index, tile) in self.tiles.iter_mut().enumerate() {
            let row = index / columns;
            let column = index % columns;

            if row >= rows {
                tile.set_hidden(true);
                continue;
            }

            let row_shift = if row % 2 == 1 { shift } else { 0.0 };
            let x: f32 = column.lossy_convert();
            let y: f32 = row.lossy_convert();

            tile.flip_x = mirror_x && column % 2 == 1;
            tile.flip_y = mirror_y && row % 2 == 1;
            tile.set_hidden(false);
            tile.set_frame((
                left + x * tile_width - row_shift,
                top + y * tile_height,
                tile_width,
                tile_height,
            ));
        }
    }
}
