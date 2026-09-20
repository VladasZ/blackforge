use hilen::{gm::color::Color, ui::DynamicColor};

const fn fixed(hex: &str) -> DynamicColor {
    DynamicColor::new(Color::hex(hex), Color::hex(hex))
}

pub const BG: DynamicColor = fixed("#141210");
pub const CARD_BG: DynamicColor = fixed("#221d17");
pub const FG: DynamicColor = fixed("#e6d6b5");
pub const FG_DIM: DynamicColor = fixed("#b6a68b");
pub const BORDER: DynamicColor = fixed("#75603b");
pub const BORDER_SOFT: DynamicColor = fixed("#33291c");
pub const ACCENT: Color = Color::hex("#d2a75e");
pub const HOVER: DynamicColor = fixed("#46331e");
pub const MENU_SHADOW: Color = Color::rgba(0.0, 0.0, 0.0, 0.6);
pub const CLEAR: Color = Color::rgba(0.0, 0.0, 0.0, 0.0);
