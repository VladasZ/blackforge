//! Checks everything that a run depends on.

use std::path::{Path, PathBuf};

use blackforge_core::doctor::{Check, Status};
use hilen::{
    refs::Weak,
    system::open_url,
    ui::{
        Button, CellRegistry, Container, Label, Setup, TableData, TableView, View, ViewData,
        ViewTooltip, view,
    },
};

use crate::{
    backend,
    ui::{
        colors,
        hint::with_hint,
        icon_button::{self, IconButton},
        style, toast,
    },
};

const ROW_HEIGHT: f32 = 58.0;
const DOT: f32 = 10.0;

#[view]
pub struct DoctorPage {
    checks: Vec<Check>,

    #[init]
    title: Label,
    subtitle: Label,
    again: Button,
    table: TableView,
}

impl Setup for DoctorPage {
    fn setup(self: Weak<Self>) {
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
        self.table
            .place()
            .t(style::HEADER)
            .l(style::PAGE_PAD)
            .r(style::PAGE_PAD)
            .b(0);

        self.check();
    }
}

impl DoctorPage {
    fn check(mut self: Weak<Self>) {
        self.subtitle.set_text("checking");
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
                        let problems = checks
                            .iter()
                            .filter(|check| check.status == Status::Problem)
                            .count();
                        self.subtitle.set_text(match problems {
                            0 => "no problems found".to_owned(),
                            1 => "1 check found a problem".to_owned(),
                            _ => format!("{problems} checks found a problem"),
                        });
                        self.checks = checks;
                        self.table.reload_data();
                    }
                    Err(error) => {
                        self.subtitle.set_text("the checks did not run");
                        toast::failure(&error);
                    }
                }
            },
        );
    }
}

impl TableData for DoctorPage {
    fn cell_height(&self, _: usize) -> f32 {
        ROW_HEIGHT
    }

    fn number_of_cells(&self) -> usize {
        self.checks.len()
    }

    fn setup_cell(&mut self, index: usize, registry: &mut CellRegistry) -> Weak<dyn View> {
        let cell = registry.cell::<CheckCell>();
        cell.set_check(&self.checks[index]);
        cell
    }

    fn cell_selected(&mut self, _: usize) {}
}

#[view]
struct CheckCell {
    /// The folder the detail names, when it names one on this disk.
    folder: Option<PathBuf>,

    #[init]
    dot: Container,
    name: Label,
    detail: Label,
    open: IconButton,
    line: Container,
}

impl Setup for CheckCell {
    fn setup(self: Weak<Self>) {
        self.dot.set_corner_radius(DOT / 2.0);
        self.dot.place().l(6).t(15).size(DOT, DOT);

        style::body(self.name);
        self.name.place().t(10).l(28).r(4).h(20);

        style::dim(self.detail);
        self.detail.set_ellipsize(true);
        self.detail
            .place()
            .t(32)
            .l(28)
            .r(16.0 + icon_button::SIZE + 12.0)
            .h(16);

        self.open.set_icon("open_folder.svg");
        self.open.set_tooltip("Open folder");
        self.open
            .place()
            .r(16)
            .center_y()
            .size(icon_button::SIZE, icon_button::SIZE);
        self.open.tapped.sub(move || {
            if let Some(folder) = &self.folder
                && let Err(error) = open_url(folder.display())
            {
                toast::error(format!("cannot open the folder: {error}"));
            }
        });

        self.line.set_color(colors::BORDER);
        self.line.place().l(0).r(0).b(0).h(1);
    }
}

impl CheckCell {
    fn set_check(mut self: Weak<Self>, check: &Check) {
        self.dot.set_color(match check.status {
            Status::Ok => colors::OK,
            Status::Warning => colors::WARN,
            Status::Problem => colors::BAD,
        });
        self.name.set_text(check.name);
        self.detail.set_text(with_hint(&check.detail, check.fix));
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
