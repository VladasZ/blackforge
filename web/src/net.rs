//! Where the release files and the manifest come from.
//!
//! Binaries are served by studio and always linked there directly, so a click
//! goes straight to the release host and no bytes pass through this
//! deployment. The manifest read is a `fetch`, which the browser would block
//! cross origin, so in the browser it goes to this host and nginx forwards it
//! to studio. The native dev build has no origin, so it reads studio directly.

pub const STUDIO: &str = "https://gebling-studio.vladas.xyz/blackforge";

/// The absolute link behind a download button.
pub fn download_url(file: &str) -> String {
    format!("{STUDIO}/download/{file}")
}

#[cfg(wasm)]
fn origin() -> String {
    web_sys::window()
        .expect("Failed to get browser window")
        .location()
        .origin()
        .expect("Failed to get location origin")
}

#[cfg(wasm)]
pub fn manifest_url() -> String {
    format!("{}/download/manifest.json", origin())
}

#[cfg(not_wasm)]
pub fn manifest_url() -> String {
    std::env::var("BLACKFORGE_WEB_MANIFEST_URL")
        .unwrap_or_else(|_| format!("{STUDIO}/download/manifest.json"))
}

#[cfg(wasm)]
pub fn track_url() -> String {
    format!("{}/api/downloads/track", origin())
}

#[cfg(not_wasm)]
pub fn track_url() -> String {
    "https://gebling-studio.vladas.xyz/api/downloads/track".to_string()
}
