pub mod colors;
mod content;
mod download_button;
mod drop_menu;
mod page;

use hilen::ui::UIManager;
pub use page::LandingPage;

/// The engine indents left aligned label text by 16 physical pixels. A label
/// that must start on a css padding edge is placed this much further left.
pub fn text_margin() -> f32 {
    16.0 / UIManager::scale()
}
