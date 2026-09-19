use std::collections::HashMap;

use anyhow::Result;
use blackforge_core::{
    forge::LockChange,
    ident::VersionedId,
    install::SyncReport,
    progress::{Event, Progress},
};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use tokio::{spawn, sync::mpsc::UnboundedReceiver, task::JoinHandle};

// Plain ASCII on purpose, a Windows console with a legacy code page cannot
// print the block characters indicatif uses by default.
const BAR_CHARS: &str = "=> ";

/// Draws the progress events of one core operation.
pub struct Ui {
    pub progress: Progress,
    drawer: JoinHandle<Result<()>>,
}

impl Ui {
    pub fn start() -> Self {
        let (progress, events) = Progress::channel();
        Self {
            progress,
            drawer: spawn(draw(events)),
        }
    }

    /// Call after the operation so the last events are drawn before any
    /// result is printed.
    pub async fn finish(self) -> Result<()> {
        drop(self.progress);
        self.drawer.await?
    }
}

async fn draw(mut events: UnboundedReceiver<Event>) -> Result<()> {
    let multi = MultiProgress::new();
    let bytes_style = ProgressStyle::with_template("{msg:40} [{bar:30}] {bytes}/{total_bytes}")?
        .progress_chars(BAR_CHARS);
    let count_style =
        ProgressStyle::with_template("{msg:40} [{bar:30}] {pos}/{len}")?.progress_chars(BAR_CHARS);
    let mut index_bar: Option<ProgressBar> = None;
    let mut downloads: HashMap<VersionedId, ProgressBar> = HashMap::new();

    while let Some(event) = events.recv().await {
        match event {
            Event::IndexChunk { done, total } => {
                let bar = index_bar.get_or_insert_with(|| {
                    let bar =
                        multi.add(ProgressBar::new(total as u64).with_style(count_style.clone()));
                    bar.set_message("package list");
                    bar
                });
                bar.set_position(done as u64);
                if done == total {
                    bar.finish_and_clear();
                }
            }
            Event::DownloadStarted {
                package,
                total_bytes,
            } => {
                let bar = multi.add(
                    ProgressBar::new(total_bytes.unwrap_or(0)).with_style(bytes_style.clone()),
                );
                bar.set_message(package.to_string());
                downloads.insert(package, bar);
            }
            Event::DownloadProgress { package, bytes } => {
                if let Some(bar) = downloads.get(&package) {
                    bar.set_position(bytes);
                }
            }
            Event::DownloadFinished { package } => {
                if let Some(bar) = downloads.remove(&package) {
                    bar.finish_and_clear();
                }
            }
            Event::Installed { package } => multi.println(format!("  installed {package}"))?,
            Event::Removed { package } => multi.println(format!("  removed   {package}"))?,
        }
    }
    Ok(())
}

pub fn print_lock_change(change: &LockChange) {
    for package in &change.added {
        println!("  + {package}");
    }
    for (id, from, to) in &change.changed {
        println!("  ~ {id} {from} -> {to}");
    }
    for package in &change.removed {
        println!("  - {package}");
    }
    for missing in &change.missing {
        println!(
            "  warning: {} needs {}, which is no longer on Thunderstore",
            missing.required_by, missing.wanted
        );
    }
    if change.is_empty() {
        println!("  the lock did not change");
    }
}

pub fn print_sync_report(report: &SyncReport) {
    println!(
        "{} installed, {} removed, {} unchanged",
        report.installed.len(),
        report.removed.len(),
        report.unchanged
    );
}

/// Prints rows as left aligned columns.
pub fn print_table(rows: &[Vec<String>]) {
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    let widths: Vec<usize> = (0..columns)
        .map(|column| {
            rows.iter()
                .filter_map(|row| row.get(column))
                .map(|cell| cell.chars().count())
                .max()
                .unwrap_or(0)
        })
        .collect();
    for row in rows {
        let cells: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(column, cell)| {
                if column + 1 == row.len() {
                    cell.clone()
                } else {
                    format!("{cell:<width$}", width = widths[column])
                }
            })
            .collect();
        println!("{}", cells.join("  ").trim_end());
    }
}
