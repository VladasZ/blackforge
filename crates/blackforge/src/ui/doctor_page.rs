//! Checks everything that a run depends on. The checks also run once at
//! start, so a problem shows as a dot on the sidebar entry before the user
//! opens this page.

use std::path::{Path, PathBuf};

use blackforge_core::{
    doctor::{Check, Status},
    fix::Fix,
    steam,
};
use hilen::{
    dispatch::{after, on_main, spawn},
    filesystem::Paths,
    refs::{Weak, weak_from_ref},
    system::open_url,
    ui::{
        Button, CellRegistry, Container, Label, Setup, TableData, TableView, VerticalAlignment,
        View, ViewData, ViewFrame, ViewTooltip, view,
    },
};

use crate::{
    backend,
    ui::{
        busy, colors,
        hint::with_hint,
        icon_button::{self, IconButton},
        names,
        nav_item::Badge,
        page::Page,
        sidebar, style, toast,
    },
};

const DOT: f32 = 10.0;
const NAME_T: f32 = 10.0;
const DETAIL_T: f32 = 32.0;
const DETAIL_L: f32 = 28.0;
const ROW_PAD_B: f32 = 12.0;
const MIN_ROW: f32 = 58.0;
/// Seconds Steam takes to start before the checks run again.
const STEAM_START: f32 = 8.0;
const FIX_W: f32 = 150.0;
/// The detail ends left of the fix button and the folder button.
const DETAIL_R: f32 = 16.0 + icon_button::SIZE + 12.0 + FIX_W + 12.0;

/// Runs the checks without the page and marks the sidebar entry.
pub fn check_in_background() {
    backend::load(
        "checking the setup",
        |forge, progress| async move {
            let profile = backend::profile(forge, &progress).await?;
            Ok(forge.doctor_of(&profile).await?)
        },
        |result| match result {
            Ok(checks) => show_badge(&checks),
            Err(error) => log::warn!("the checks at start did not run: {error:#}"),
        },
    );
}

fn show_badge(checks: &[Check]) {
    let problem = checks.iter().any(|check| check.status == Status::Problem);
    sidebar::set_badge(
        Page::Doctor,
        if problem { Badge::Alert } else { Badge::None },
    );
}

/// The button a failed check gets, when the window can do the fix itself.
fn fix_label(fix: Fix) -> Option<&'static str> {
    match fix {
        Fix::GiveGameFolder => Some("Pick game folder"),
        Fix::Sync => Some("Install missing files"),
        Fix::StartSteam => Some("Open Steam"),
        Fix::CreateProfile | Fix::PickProfile => None,
    }
}

/// The detail with the way out, unless a button on the row does it.
fn detail_text(check: &Check) -> String {
    let button = check.fix.and_then(fix_label).is_some();
    let fix = if button { None } else { check.fix };
    names::sentence(&with_hint(&check.detail, fix))
}

#[view]
pub struct DoctorPage {
    checks: Vec<Check>,

    #[init]
    title: Label,
    subtitle: Label,
    again: Button,
    table: TableView,
    /// Never shown. It has the look of a detail, so it can say how tall a
    /// detail is at the width the table has now.
    probe: Label,
}

impl Setup for DoctorPage {
    fn setup(mut self: Weak<Self>) {
        style::title(self.title, "Doctor");
        self.title.place().t(24).l(style::PAGE_PAD).size(300, 30);

        style::dim(self.subtitle);
        self.subtitle.place().t(56).l(style::PAGE_PAD).size(500, 16);

        style::ghost(self.again, "Check again");
        self.again
            .place()
            .t(28)
            .r(style::PAGE_PAD)
            .size(110, style::BUTTON_H);
        self.again.on_tap(move || self.check());

        self.table
            .set_data_source(self)
            .register_cell::<CheckCell>();
        style::table(self.table);
        self.table.set_variable_heights(true);
        self.table
            .place()
            .t(style::HEADER)
            .l(style::PAGE_PAD)
            .r(style::PAGE_PAD)
            .b(0);
        // A detail wraps at the width of the page, so a new width means new
        // row heights.
        self.size_changed().sub(move || self.table.reload_data());

        style::dim(self.probe);
        self.probe.set_multiline(true);
        self.probe.set_hidden(true);

        self.check();
    }
}

impl DoctorPage {
    fn check(mut self: Weak<Self>) {
        self.subtitle.set_text("Checking");
        backend::load(
            "checking the setup",
            |forge, progress| async move {
                let profile = backend::profile(forge, &progress).await?;
                Ok(forge.doctor_of(&profile).await?)
            },
            move |result| {
                if !self.is_ok() {
                    return;
                }
                match result {
                    Ok(checks) => {
                        show_badge(&checks);
                        let problems = checks
                            .iter()
                            .filter(|check| check.status == Status::Problem)
                            .count();
                        self.subtitle.set_text(match problems {
                            0 => "No problems found".to_owned(),
                            1 => "1 check found a problem".to_owned(),
                            _ => format!("{problems} checks found a problem"),
                        });
                        self.checks = checks;
                        self.table.reload_data();
                    }
                    Err(error) => {
                        self.subtitle.set_text("The checks did not run");
                        toast::failure(&error);
                    }
                }
            },
        );
    }

    fn detail_width(&self) -> f32 {
        self.table.width() - DETAIL_L - DETAIL_R
    }

    fn fix(self: Weak<Self>, index: usize, button: Weak<Button>) {
        let Some(fix) = self.checks.get(index).and_then(|check| check.fix) else {
            return;
        };
        match fix {
            Fix::GiveGameFolder => self.pick_game_folder(button),
            Fix::Sync => self.install_missing(button),
            Fix::StartSteam => self.open_steam(),
            Fix::CreateProfile | Fix::PickProfile => {}
        }
    }

    /// Steam needs a few seconds to start, the checks run again after them.
    fn open_steam(self: Weak<Self>) {
        backend::load(
            "opening Steam",
            |_, _| async move { Ok(steam::open().await?) },
            move |result| match result {
                Ok(()) => {
                    toast::info("Steam is starting");
                    after(STEAM_START, move || {
                        if self.is_ok() {
                            self.check();
                        }
                    });
                }
                Err(error) => toast::failure(&error),
            },
        );
    }

    fn install_missing(self: Weak<Self>, button: Weak<Button>) {
        busy::press(button, "Installing...");
        backend::change(
            "installing the missing files",
            |forge, progress| async move {
                let profile = backend::profile(forge, &progress).await?;
                Ok(forge.sync(&profile, &progress).await?)
            },
            move |result| {
                match result {
                    Ok(report) => {
                        toast::success(format!("{} files installed", report.installed.len()));
                    }
                    Err(error) => toast::failure(&error),
                }
                if self.is_ok() {
                    self.check();
                }
            },
        );
    }

    /// The folder is remembered by the core, the game starts from it later.
    fn pick_game_folder(self: Weak<Self>, button: Weak<Button>) {
        spawn(async move {
            let picked = Paths::pick_folder().await;
            on_main(move || {
                let Some(folder) = picked else {
                    return;
                };
                busy::press(button, "Saving...");
                backend::change(
                    "saving the game folder",
                    |forge, progress| async move {
                        let profile = backend::profile(forge, &progress).await?;
                        let game = forge.game(&profile.manifest().await?).await?;
                        let install = forge.locate_game(&game, Some(folder)).await?;
                        Ok(install.dir)
                    },
                    move |result| {
                        match result {
                            Ok(dir) => {
                                toast::success(format!("The game is in {}", dir.display()));
                            }
                            Err(error) => toast::failure(&error),
                        }
                        if self.is_ok() {
                            self.check();
                        }
                    },
                );
            });
        });
    }
}

impl TableData for DoctorPage {
    fn cell_height(&self, index: usize) -> f32 {
        self.probe.set_text(detail_text(&self.checks[index]));
        let detail = self.probe.size_for_width(self.detail_width()).height;
        (DETAIL_T + detail + ROW_PAD_B).max(MIN_ROW)
    }

    fn number_of_cells(&self) -> usize {
        self.checks.len()
    }

    fn setup_cell(&mut self, index: usize, registry: &mut CellRegistry) -> Weak<dyn View> {
        let cell = registry.cell::<CheckCell>();
        let width = self.detail_width();
        cell.set_check(index, weak_from_ref(self), &self.checks[index], width);
        cell
    }

    fn cell_selected(&mut self, _: usize) {}
}

#[view]
struct CheckCell {
    index: usize,
    page: Weak<DoctorPage>,
    /// The folder the detail names, when it names one on this disk.
    folder: Option<PathBuf>,

    #[init]
    dot: Container,
    name: Label,
    detail: Label,
    fix: Button,
    open: IconButton,
    line: Container,
}

impl Setup for CheckCell {
    fn setup(self: Weak<Self>) {
        self.dot.set_corner_radius(DOT / 2.0);
        self.dot.place().l(6).t(NAME_T + 5.0).size(DOT, DOT);

        style::body(self.name);
        self.name.place().t(NAME_T).l(DETAIL_L).r(DETAIL_R).h(20);

        // The whole text, wrapped. The row is as tall as the text needs.
        style::dim(self.detail);
        self.detail.set_multiline(true);
        self.detail.set_vertical_alignment(VerticalAlignment::Top);

        style::primary(self.fix, "");
        self.fix
            .place()
            .t(NAME_T + 2.0)
            .r(16.0 + icon_button::SIZE + 12.0)
            .size(FIX_W, style::BUTTON_H);
        self.fix.on_tap(move || {
            if self.page.is_ok() {
                self.page.fix(self.index, self.fix);
            }
        });
        busy::track(self.fix);

        self.open.set_icon("open_folder.svg");
        self.open.set_tooltip("Open folder");
        self.open
            .place()
            .r(16)
            .t(NAME_T + 4.0)
            .size(icon_button::SIZE, icon_button::SIZE);
        self.open.tapped.sub(move || {
            if let Some(folder) = &self.folder
                && let Err(error) = open_url(folder.display())
            {
                toast::error(format!("Cannot open the folder: {error}"));
            }
        });

        self.line.set_color(colors::BORDER);
        self.line.place().l(0).r(0).b(0).h(1);
    }
}

impl CheckCell {
    fn set_check(
        mut self: Weak<Self>,
        index: usize,
        page: Weak<DoctorPage>,
        check: &Check,
        width: f32,
    ) {
        self.index = index;
        self.page = page;

        self.dot.set_color(match check.status {
            Status::Ok => colors::OK,
            Status::Warning => colors::WARN,
            Status::Problem => colors::BAD,
        });
        self.name.set_text(names::sentence(check.name));

        self.detail.set_text(detail_text(check));
        let height = self.detail.size_for_width(width).height;
        self.detail
            .place()
            .clear()
            .t(DETAIL_T)
            .l(DETAIL_L)
            .r(DETAIL_R)
            .h(height);

        let fix = check
            .fix
            .filter(|_| check.status != Status::Ok)
            .and_then(fix_label);
        self.fix.set_hidden(fix.is_none());
        if let Some(label) = fix {
            self.fix.set_text(label);
        }

        self.folder = folder_of(&check.detail);
        self.open.set_hidden(self.folder.is_none());
    }
}

/// A check whose detail is a path on this disk: a folder itself, or the
/// folder of a file such as the game exe.
fn folder_of(detail: &str) -> Option<PathBuf> {
    let path = Path::new(detail);
    if !path.is_absolute() {
        return None;
    }
    if path.is_dir() {
        Some(path.to_path_buf())
    } else if path.is_file() {
        path.parent().map(Path::to_path_buf)
    } else {
        None
    }
}
