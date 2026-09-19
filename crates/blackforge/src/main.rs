#![allow(incomplete_features)]
#![feature(specialization)]
#![feature(arbitrary_self_types)]

mod app;
mod backend;
mod icons;
mod launcher;
mod ui;

use hilen::App;

use crate::app::BlackforgeApp;

fn main() {
    BlackforgeApp::start();
}
