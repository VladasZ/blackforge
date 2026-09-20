//! The bar at the bottom of the window. It draws the progress events of the
//! running core operations, the job `indicatif` does for the command line.

use std::{cell::Cell, collections::HashMap};

use blackforge_core::{ident::VersionedId, progress::Event};
use hilen::{
    BugReport,
    gm::LossyConvert,
    refs::Weak,
    ui::{Button, Container, Label, ProgressView, Setup, ViewData, view},
};

use crate::ui::{colors, icon_button::IconButton, style};
use crate::{
    ui::toast,
    updater::{self, Phase},
};

pub const HEIGHT: f32 = 34.0;

const BUTTON_HEIGHT: f32 = 26.0;
const GAP: f32 = 8.0;
/// Fits the longest label, "Install 0.1.10 & restart", at text size 12.
const UPDATE_WIDTH: f32 = 160.0;
const PROGRESS_WIDTH: f32 = 200.0;
const PROGRESS_GAP: f32 = 12.0;
const TEXT_GAP: f32 = 40.0;

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
    update: Button,
    bug: IconButton,
}

impl Setup for StatusBar {
    fn setup(self: Weak<Self>) {
        self.set_color(colors::SIDEBAR_BG);

        self.line.set_color(colors::BORDER);
        self.line.place().l(0).r(0).t(0).h(1);

        // From the right edge: the bug button, the update button, the
        // progress line, then the text takes what is left.
        let update_right = style::PAGE_PAD + BUTTON_HEIGHT + GAP;
        let progress_right = update_right + UPDATE_WIDTH + PROGRESS_GAP;
        let text_right = progress_right + PROGRESS_WIDTH + TEXT_GAP;

        style::dim(self.text);
        self.text.set_text("ready");
        self.text.place().l(style::PAGE_PAD).r(text_right).t(0).b(0);

        self.progress.set_hidden(true);
        self.progress
            .place()
            .r(progress_right)
            .center_y()
            .size(PROGRESS_WIDTH, 6);

        // The button always shows, like in kukareker. The engine opens the
        // dialog only with a Sentry DSN, without one a tap only logs a warning.
        self.bug.set_icon("bug.svg");
        self.bug
            .place()
            .r(style::PAGE_PAD)
            .center_y()
            .size(BUTTON_HEIGHT, BUTTON_HEIGHT);
        self.bug.tapped.sub(BugReport::open);

        self.update
            .place()
            .r(update_right)
            .center_y()
            .size(UPDATE_WIDTH, BUTTON_HEIGHT);
        style::ghost(self.update, "Check for updates");
        self.update.set_text_size(12);
        self.refresh_update();
        updater::state().changed.sub(move || self.refresh_update());
        self.update.on_tap(|| {
            let state = updater::state();
            if state.busy() {
                return;
            }
            if state.has_update() {
                updater::install(toast::error);
            } else {
                updater::check(|state| {
                    if let Some(error) = &state.error {
                        toast::error(error);
                    } else if !state.has_update() {
                        toast::info("Blackforge is up to date");
                    }
                });
            }
        });

        BAR.with(|slot| slot.set(self));
    }
}

impl StatusBar {
    fn refresh_update(self: Weak<Self>) {
        let state = updater::state();
        let label = match state.phase {
            Phase::Idle => "Check for updates".to_owned(),
            Phase::Checking => "Checking for updates…".to_owned(),
            Phase::Available => format!(
                "Install {} & restart",
                state.version.as_deref().unwrap_or_default()
            ),
            Phase::Installing => format!("Installing {}%", state.progress),
        };
        self.update.set_text(label);
    }

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
