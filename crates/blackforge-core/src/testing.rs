//! Shared helpers for unit tests.

use std::{fs::File, io::Write, path::Path};

use zip::{ZipWriter, write::SimpleFileOptions};

use crate::{
    error::{IoContext, Result},
    game::{GameDef, Schema, Target, VALHEIM},
};

/// The Valheim part of the real Thunderstore schema.
pub fn valheim(target: Target) -> Result<GameDef> {
    Schema::from_json(include_bytes!("testdata/schema.json"))?.game(VALHEIM, target)
}

/// Writes a zip where every file holds its own entry name as the content.
pub fn write_zip(path: &Path, entries: &[&str]) -> Result<()> {
    let mut writer = ZipWriter::new(File::create(path).at(path)?);
    for entry in entries {
        writer.start_file(*entry, SimpleFileOptions::default())?;
        writer.write_all(entry.as_bytes()).at(path)?;
    }
    writer.finish()?;
    Ok(())
}
