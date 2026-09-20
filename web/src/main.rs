#![allow(incomplete_features)]
#![feature(specialization)]
#![feature(arbitrary_self_types)]

mod api;
mod app;
mod fonts;
mod model;
mod net;
mod ui;

use hilen::App;

use crate::app::BlackforgeWeb;

fn main() {
    BlackforgeWeb::start();
}
