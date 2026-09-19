//! The bar at the bottom of the window. It draws the progress events of the
//! running core operations, the job `indicatif` does for the command line.

use std::{cell::Cell, collections::HashMap};

use blackforge_core::{ident::VersionedId, progress::Event};
use hilen::{
    gm::LossyConvert,
    refs::Weak,
    ui::{Container, Label, ProgressView, Setup, ViewData, view},
};

use crate::ui::{colors, style};

pub const HEIGHT: f32 = 34.0;

thread_local! {
    static BAR: Cell<Weak<StatusBar>> = const { Cell::new(Weak::const_default()) };
}

fn bar() -> Option<Weak<StatusBar>> {
    let bar = BAR.with(Cell::get);
    bar.is_ok().then_some(bar)
}

/// Marks the start of one operation. The id goes back into `event` and `end`.
pub fn begin(title: &str) -> u64 {
    bar().map_or(0, |bar| bar.begin(title))
}

pub fn event(id: u64, event: Event) {
    if let Some(bar) = bar() {
        bar.event(id, event);
    }
}

pub fn end(id: u64) {
    if let Some(bar) = bar() {
        bar.end(id);
    }
}

#[view]
pub struct StatusBar {
    next_id: u64,
    running: Vec<u64>,
    /// Received and total bytes of every download of the running operations.
    downloads: HashMap<VersionedId, (u64, u64)>,

    #[init]
    line: Container,
    text: Label,
    progress: ProgressView,
}

impl Setup for StatusBar {
    fn setup(self: Weak<Self>) {
        self.set_color(colors::SIDEBAR_BG);

        self.line.set_color(colors::BORDER);
        self.line.place().l(0).r(0).t(0).h(1);

        style::dim(self.text);
        self.text.set_text("ready");
        self.text.place().l(style::PAGE_PAD).r(260).t(0).b(0);

        self.progress.set_hidden(true);
        self.progress
            .place()
            .r(style::PAGE_PAD)
            .center_y()
            .size(200, 6);

        BAR.with(|slot| slot.set(self));
    }
}

impl StatusBar {
    fn begin(mut self: Weak<Self>, title: &str) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        self.running.push(id);
        self.text.set_text(title);
        id
    }

    fn end(mut self: Weak<Self>, id: u64) {
        self.running.retain(|running| *running != id);
        if self.running.is_empty() {
            self.downloads.clear();
            self.text.set_text("ready");
            self.progress.set_hidden(true);
        }
    }

    fn event(mut self: Weak<Self>, id: u64, event: Event) {
        // The events travel on their own task, so the last ones can land
        // after the operation already ended.
        if !self.running.contains(&id) {
            return;
        }
        match event {
            Event::IndexChunk { done, total } => {
                self.text
                    .set_text(format!("package list {done} of {total}"));
                self.show_progress(done.lossy_convert(), total.lossy_convert());
            }
            Event::DownloadStarted {
                package,
                total_bytes,
            } => {
                self.text.set_text(format!("downloading {package}"));
                self.downloads
                    .insert(package, (0, total_bytes.unwrap_or(0)));
                self.show_downloads();
            }
            Event::DownloadProgress { package, bytes } => {
                if let Some(download) = self.downloads.get_mut(&package) {
                    download.0 = bytes;
                }
                self.show_downloads();
            }
            Event::DownloadFinished { package } => {
                if let Some(download) = self.downloads.get_mut(&package) {
                    download.0 = download.1;
                }
                self.show_downloads();
            }
            Event::Installed { package } => {
                self.text.set_text(format!("installed {package}"));
            }
            Event::Removed { package } => {
                self.text.set_text(format!("removed {package}"));
            }
        }
    }

    fn show_downloads(self: Weak<Self>) {
        let received: u64 = self.downloads.values().map(|download| download.0).sum();
        let total: u64 = self.downloads.values().map(|download| download.1).sum();
        self.show_progress(received.lossy_convert(), total.lossy_convert());
    }

    fn show_progress(self: Weak<Self>, done: f32, total: f32) {
        self.progress.set_hidden(total <= 0.0);
        if total > 0.0 {
            self.progress.set_progress(done / total);
        }
    }
}
