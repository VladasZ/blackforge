//! The palette, light and dark pairs. Warm grays with an ember accent.

use hilen::{gm::color::Color, ui::DynamicColor};

pub const BG: DynamicColor = DynamicColor::new(Color::hex("#f6f4f1"), Color::hex("#131211"));
pub const SIDEBAR_BG: DynamicColor =
    DynamicColor::new(Color::hex("#ffffff"), Color::hex("#1a1917"));
pub const CARD_BG: DynamicColor = DynamicColor::new(Color::hex("#ffffff"), Color::hex("#201e1c"));
pub const FIELD_BG: DynamicColor = DynamicColor::new(Color::hex("#ffffff"), Color::hex("#171615"));
pub const BORDER: DynamicColor = DynamicColor::new(
    Color::hex("#14110d").with_alpha(0.12),
    Color::hex("#ffffff").with_alpha(0.09),
);
pub const FG: DynamicColor = DynamicColor::new(Color::hex("#1a1712"), Color::hex("#f1ede6"));
pub const DIM: DynamicColor = DynamicColor::new(
    Color::hex("#1a1712").with_alpha(0.55),
    Color::hex("#f1ede6").with_alpha(0.55),
);
pub const NAV_ACTIVE_BG: DynamicColor = DynamicColor::new(
    Color::hex("#e8590c").with_alpha(0.12),
    Color::hex("#e8590c").with_alpha(0.18),
);
pub const PILL_BG: DynamicColor = DynamicColor::new(
    Color::hex("#14110d").with_alpha(0.07),
    Color::hex("#ffffff").with_alpha(0.09),
);
pub const NAV_HOVER_BG: DynamicColor = DynamicColor::new(
    Color::hex("#14110d").with_alpha(0.05),
    Color::hex("#ffffff").with_alpha(0.06),
);

pub const ACCENT: Color = Color::hex("#e8590c");
pub const ON_ACCENT: Color = Color::hex("#ffffff");
pub const OK: Color = Color::hex("#22a35a");
pub const OK_BG: Color = Color::hex("#22a35a").with_alpha(0.14);
pub const WARN: Color = Color::hex("#d9a20b");
pub const BAD: Color = Color::hex("#dc4a3d");
pub const BAD_BG: Color = Color::hex("#dc4a3d").with_alpha(0.14);
pub const SCRIM: Color = Color::hex("#000000").with_alpha(0.4);
pub const CLEAR: Color = Color::rgba(0.0, 0.0, 0.0, 0.0);
