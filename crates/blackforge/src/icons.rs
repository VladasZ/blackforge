//! The icons of mods as pixels for a texture. The files come from the icon
//! cache of the core, which downloads an icon once and keeps it for good.

use anyhow::Result;
use blackforge_core::{ident::VersionedId, thunderstore::IconCache};
use hilen::dispatch::{on_main, spawn};
use image::{ImageFormat, imageops::FilterType, load_from_memory_with_format};
use tokio::{fs, task::spawn_blocking};

use crate::backend::forge;

/// Thunderstore icons are 256 pixels, which is 262 KB as a texture, and a row
/// shows one at 40 points. Half the side is a quarter of the memory and still
/// sharp on a retina screen, also at the larger size of the details.
const SIDE: u32 = 128;

/// Square RGBA pixels, `side` by `side`.
pub struct Pixels {
    pub rgba: Vec<u8>,
    pub side: u32,
}

/// Loads the icon of `package` and hands it over on the main thread. It
/// makes no entry in the status bar, a page of results asks for dozens.
pub fn load(package: VersionedId, done: impl FnOnce(Result<Pixels>) + Send + 'static) {
    spawn(async move {
        let result = pixels(&package).await;
        on_main(move || done(result));
    });
}

async fn pixels(package: &VersionedId) -> Result<Pixels> {
    let forge = forge()?;
    let path = IconCache::new(forge.data())
        .fetch(forge.client(), package)
        .await?;
    let png = fs::read(&path).await?;
    // Decoding and scaling are plain computing, they must not hold up the
    // async runtime that also drives the downloads.
    spawn_blocking(move || shrink(&png)).await?
}

fn shrink(png: &[u8]) -> Result<Pixels> {
    let icon = load_from_memory_with_format(png, ImageFormat::Png)?;
    let small = icon
        .resize_exact(SIDE, SIDE, FilterType::Triangle)
        .into_rgba8();
    Ok(Pixels {
        rgba: small.into_raw(),
        side: SIDE,
    })
}
