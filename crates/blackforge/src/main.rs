#![allow(incomplete_features)]
#![feature(specialization)]
#![feature(arbitrary_self_types)]
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod assets;
mod backend;
mod bridge;
mod cloud;
mod game_process;
mod icons;
mod launcher;
mod social;
mod ui;
mod updater;
mod updates;

use hilen::App;

use crate::app::BlackforgeApp;

fn main() {
    BlackforgeApp::start();
}
