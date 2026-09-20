//! The landing page: downloads and features in a scrollable column.

use hilen::{
    dispatch::{on_main, spawn},
    refs::Weak,
    ui::{ScrollView, Setup, UIManager, ViewData, ViewFrame, ViewSubviews, view},
};

use crate::{
    api,
    ui::{colors, content::Content},
};

#[view]
pub struct LandingPage {
    content: Weak<Content>,

    #[init]
    scroll: ScrollView,
}

impl Setup for LandingPage {
    fn setup(mut self: Weak<Self>) {
        UIManager::set_clear_color(colors::BG);
        self.scroll.place().back();

        self.content = self.scroll.add_view::<Content>();

        self.size_changed().sub(move || self.relayout());

        spawn(async move {
            let manifest = api::manifest().await;

            on_main(move || {
                if !self.is_ok() {
                    return;
                }

                match manifest {
                    Ok(manifest) => {
                        self.content.set_manifest(manifest);
                        self.relayout();
                    }
                    Err(error) => {
                        log::error!("Failed to read the release manifest: {error}");
                        self.content.manifest_failed();
                        self.relayout();
                    }
                }
            });
        });
    }
}

impl LandingPage {
    fn relayout(mut self: Weak<Self>) {
        let width = self.width();

        if width < 1.0 {
            return;
        }

        let height = self.content.relayout(width, self.height());

        self.content.set_frame((0.0, 0.0, width, height));
        self.scroll.set_content_height(height);
    }
}
