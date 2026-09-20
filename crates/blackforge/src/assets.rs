use hilen::{refs::manage::DataManager, ui::Image};

// Embed the interface images so installers and updates carry one executable.
pub fn load() {
    let images: [(&str, &[u8]); 8] = [
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
            "open_page.svg",
            include_bytes!("../../../assets/images/open_page.svg"),
        ),
        (
            "pill_downloads.svg",
            include_bytes!("../../../assets/images/pill_downloads.svg"),
        ),
        (
            "pill_version.svg",
            include_bytes!("../../../assets/images/pill_version.svg"),
        ),
    ];
    for (name, bytes) in images {
        Image::load(bytes, name);
    }
}
