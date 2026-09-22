use std::sync::LazyLock;

use hilen::{gm::color::Color, refs::manage::DataManager, ui::Image};
use regex::Regex;

// Embed the interface images so installers and updates carry one executable.
const IMAGES: [(&str, &[u8]); 24] = [
    (
        "nav_settings.svg",
        include_bytes!("../../../assets/images/nav_settings.svg"),
    ),
    (
        "trash.svg",
        include_bytes!("../../../assets/images/trash.svg"),
    ),
    (
        "open_folder.svg",
        include_bytes!("../../../assets/images/open_folder.svg"),
    ),
    (
        "check.svg",
        include_bytes!("../../../assets/images/check.svg"),
    ),
    ("bug.svg", include_bytes!("../../../assets/images/bug.svg")),
    (
        "nav_servers.svg",
        include_bytes!("../../../assets/images/nav_servers.svg"),
    ),
    (
        "nav_mods.svg",
        include_bytes!("../../../assets/images/nav_mods.svg"),
    ),
    (
        "nav_browse.svg",
        include_bytes!("../../../assets/images/nav_browse.svg"),
    ),
    (
        "nav_configs.svg",
        include_bytes!("../../../assets/images/nav_configs.svg"),
    ),
    (
        "nav_share.svg",
        include_bytes!("../../../assets/images/nav_share.svg"),
    ),
    (
        "nav_doctor.svg",
        include_bytes!("../../../assets/images/nav_doctor.svg"),
    ),
    (
        "nav_friends.svg",
        include_bytes!("../../../assets/images/nav_friends.svg"),
    ),
    (
        "open_page.svg",
        include_bytes!("../../../assets/images/open_page.svg"),
    ),
    (
        "pill_downloads.svg",
        include_bytes!("../../../assets/images/pill_downloads.svg"),
    ),
    (
        "pill_broken.svg",
        include_bytes!("../../../assets/images/pill_broken.svg"),
    ),
    (
        "pill_dependency.svg",
        include_bytes!("../../../assets/images/pill_dependency.svg"),
    ),
    (
        "pill_deprecated.svg",
        include_bytes!("../../../assets/images/pill_deprecated.svg"),
    ),
    (
        "pill_disabled.svg",
        include_bytes!("../../../assets/images/pill_disabled.svg"),
    ),
    (
        "pill_pinned.svg",
        include_bytes!("../../../assets/images/pill_pinned.svg"),
    ),
    (
        "pill_version.svg",
        include_bytes!("../../../assets/images/pill_version.svg"),
    ),
    (
        "sync_cloud.svg",
        include_bytes!("../../../assets/images/sync_cloud.svg"),
    ),
    (
        "sync_history.svg",
        include_bytes!("../../../assets/images/sync_history.svg"),
    ),
    (
        "tab_friends.svg",
        include_bytes!("../../../assets/images/tab_friends.svg"),
    ),
    (
        "tab_search.svg",
        include_bytes!("../../../assets/images/tab_search.svg"),
    ),
];

pub fn load() {
    for (name, bytes) in IMAGES {
        Image::load(bytes, name);
    }
}

/// The name of `name` drawn in `tint`, made on first use. The icons are
/// single color line art, so every hex color of the SVG becomes the tint.
/// The engine `Tinted` reads the file from disk, and a release carries its
/// images inside the executable.
pub fn tinted(name: &str, tint: Color) -> String {
    let hex = tint.as_hex();
    let tinted = format!("{name}:{hex}");
    if Image::get_existing(&tinted).is_none()
        && let Some((_, bytes)) = IMAGES.iter().find(|(image, _)| *image == name)
    {
        let source = String::from_utf8_lossy(bytes);
        let svg = HEX_COLOR.replace_all(&source, format!("\"{hex}\""));
        Image::load(svg.as_bytes(), &tinted);
    }
    tinted
}

static HEX_COLOR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r##""#[0-9a-fA-F]{6}""##).expect("the pattern is valid"));
