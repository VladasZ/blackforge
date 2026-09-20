#![allow(incomplete_features)]
#![feature(specialization)]
#![feature(arbitrary_self_types)]
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod assets;
mod backend;
mod icons;
mod launcher;
mod ui;
mod updater;

use hilen::App;

use crate::app::BlackforgeApp;

fn main() {
    BlackforgeApp::start();
}
