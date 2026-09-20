use hilen::{
    App,
    refs::Own,
    ui::{Setup, Size, View},
};

use crate::ui::LandingPage;

#[derive(Default)]
pub struct BlackforgeWeb;

impl App for BlackforgeWeb {
    fn make_root_view(&self) -> Own<dyn View> {
        LandingPage::new()
    }

    /// Desktop is the dev loop for a page that ships as wasm. This opens it at
    /// the width the layout was checked against, so a `cargo run` window and
    /// the browser reference show the same breakpoints.
    fn initial_size(&self) -> Size {
        (1920, 1700).into()
    }

    fn log_targets(&self) -> &'static [&'static str] {
        &["blackforge_web"]
    }
}
