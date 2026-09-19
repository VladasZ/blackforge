use std::fs::create_dir_all;

use blackforge_core::paths::DataDir;
use hilen::{
    App, AppRunner,
    refs::Own,
    store::OnDisk,
    ui::{Setup, Size, View},
};

use crate::ui::Shell;

#[derive(Default)]
pub struct BlackforgeApp;

impl App for BlackforgeApp {
    fn make_root_view(&self) -> Own<dyn View> {
        Shell::new()
    }

    // Physical pixels. On a retina screen this opens as 1200 x 800 points.
    fn initial_size(&self) -> Size {
        (2400, 1600).into()
    }

    // The settings of the window live next to the profiles, so one folder
    // still holds everything.
    fn before_launch(&self) {
        let data = DataDir::locate().expect("this system has no home folder");
        create_dir_all(data.root()).expect("cannot create the data folder");
        OnDisk::<()>::set_root_path(data.root());
    }

    fn after_launch(&self) {
        AppRunner::set_window_title("Blackforge");
    }
}
