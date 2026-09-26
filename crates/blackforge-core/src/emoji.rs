//! Animated emojis in the game chat and above the heads of players.
//!
//! A small loader plugin, the source is in `assets/emoji`. It goes into every
//! Valheim client started from the app, with the join plugin, there is no
//! setting. A player types `:petuh` and the chat shows the animation.
//!
//! Unity cannot read a GIF, so the app turns every GIF into one PNG sprite
//! sheet, the frames in rows from the top left, and a JSON file that tells the
//! plugin the frame size and speed.

use std::{
    io::Cursor,
    path::{Path, PathBuf},
    time::Duration,
};

use image::{AnimationDecoder, ImageFormat, RgbaImage, codecs::gif::GifDecoder, imageops::replace};
use serde::Serialize;
use tokio::{fs, task::spawn_blocking};

use crate::error::{IoContext, Result};

const PLUGIN: &[u8] = include_bytes!("../../../assets/emoji/BlackforgeEmoji.dll");
const PLUGIN_DIR: [&str; 3] = ["BepInEx", "plugins", "blackforge-emoji"];
const PLUGIN_FILE: &str = "BlackforgeEmoji.dll";
const LIST_FILE: &str = "emojis.json";

/// The code a player types is a colon and this name.
struct Emoji {
    name: &'static str,
    gif: &'static [u8],
}

const EMOJIS: &[Emoji] = &[Emoji {
    name: "petuh",
    gif: include_bytes!("../../../assets/bug-rooster.gif"),
}];

/// About square, so the sheet stays far below the texture size limit of
/// Unity, 16384 pixels on a side.
const COLUMNS: u32 = 8;

#[derive(Debug, Serialize)]
struct List {
    emojis: Vec<Sheet>,
}

#[derive(Debug, Serialize)]
struct Sheet {
    name: String,
    file: String,
    width: u32,
    height: u32,
    columns: u32,
    frames: u32,
    fps: u32,
}

pub fn plugin_dir(profile_dir: &Path) -> PathBuf {
    PLUGIN_DIR
        .iter()
        .fold(profile_dir.to_path_buf(), |path, part| path.join(part))
}

pub async fn apply(profile_dir: &Path) -> Result<()> {
    let dir = plugin_dir(profile_dir);
    fs::create_dir_all(&dir).await.at(&dir)?;
    let path = dir.join(PLUGIN_FILE);
    fs::write(&path, PLUGIN).await.at(&path)?;

    let mut list = List { emojis: Vec::new() };
    for emoji in EMOJIS {
        let (sheet, png) = spawn_blocking(|| to_sheet(emoji)).await??;
        let path = dir.join(&sheet.file);
        fs::write(&path, png).await.at(&path)?;
        list.emojis.push(sheet);
    }
    let path = dir.join(LIST_FILE);
    fs::write(&path, serde_json::to_vec_pretty(&list)?)
        .await
        .at(&path)
}

fn to_sheet(emoji: &Emoji) -> Result<(Sheet, Vec<u8>)> {
    let frames = GifDecoder::new(Cursor::new(emoji.gif))?
        .into_frames()
        .collect_frames()?;
    let (width, height) = frames
        .first()
        .map_or((0, 0), |frame| frame.buffer().dimensions());
    let count = u32::try_from(frames.len()).unwrap_or(u32::MAX);
    let columns = COLUMNS.min(count).max(1);
    let rows = count.div_ceil(columns);

    let mut sheet = RgbaImage::new(width * columns, height * rows);
    let mut total = Duration::ZERO;
    for (index, frame) in (0..count).zip(&frames) {
        let x = (index % columns) * width;
        let y = (index / columns) * height;
        replace(&mut sheet, frame.buffer(), i64::from(x), i64::from(y));
        total += Duration::from(frame.delay());
    }
    // The plugin plays at one speed, so an uneven GIF gets its average.
    let millis = u32::try_from(total.as_millis()).unwrap_or(u32::MAX);
    let fps = (count * 1000).checked_div(millis).unwrap_or(10).max(1);

    let mut png = Vec::new();
    sheet.write_to(&mut Cursor::new(&mut png), ImageFormat::Png)?;
    let sheet = Sheet {
        name: emoji.name.to_string(),
        file: format!("{}.png", emoji.name),
        width,
        height,
        columns,
        frames: count,
        fps,
    };
    Ok((sheet, png))
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use tempfile::tempdir;

    use super::*;

    #[derive(Deserialize)]
    struct ReadList {
        emojis: Vec<ReadSheet>,
    }

    #[derive(Deserialize)]
    struct ReadSheet {
        name: String,
        file: String,
        width: u32,
        height: u32,
        columns: u32,
        frames: u32,
        fps: u32,
    }

    #[tokio::test]
    async fn writes_the_plugin_and_the_sheets() -> Result<()> {
        let temp = tempdir().at(Path::new("tempdir"))?;
        let dir = plugin_dir(temp.path());

        apply(temp.path()).await?;
        let path = dir.join(PLUGIN_FILE);
        assert_eq!(fs::read(&path).await.at(&path)?, PLUGIN);

        let path = dir.join(LIST_FILE);
        let list: ReadList = serde_json::from_slice(&fs::read(&path).await.at(&path)?)?;
        let [rooster] = list.emojis.as_slice() else {
            panic!("one emoji is shipped");
        };
        assert_eq!(rooster.name, "petuh");
        assert_eq!((rooster.width, rooster.height), (320, 200));
        assert_eq!((rooster.frames, rooster.columns, rooster.fps), (67, 8, 10));

        let path = dir.join(&rooster.file);
        let sheet = image::load_from_memory(&fs::read(&path).await.at(&path)?)?;
        assert_eq!((sheet.width(), sheet.height()), (320 * 8, 200 * 9));

        // A second start writes it all again over the old copy.
        apply(temp.path()).await?;
        Ok(())
    }
}
