use std::collections::BTreeMap;

use blackforge_api::setup::{Mod, Setup};
use tempfile::tempdir;
use tokio::fs;

use super::{History, capture, portable, recover, restore::write_settings, review, validate};
use crate::{
    config::ConfigFile,
    error::{IoContext, Result},
    game::{Target, VALHEIM},
    paths::DataDir,
    profile::ProfileStore,
};

fn setup(value: &str) -> Setup {
    Setup {
        configs: BTreeMap::from([(
            "example.cfg".to_owned(),
            BTreeMap::from([(
                "general".to_owned(),
                BTreeMap::from([("count".to_owned(), value.to_owned())]),
            )]),
        )]),
        ..Setup::default()
    }
}

fn add_mod(setup: &mut Setup, id: &str, version: &str) {
    setup.mods.insert(
        id.to_owned(),
        Mod {
            version: version.to_owned(),
            requested: Some("*".to_owned()),
            enabled: true,
            dependencies: Vec::new(),
        },
    );
}

#[test]
fn independent_mod_and_setting_changes_merge() {
    let base = setup("1");
    let mut local = base.clone();
    add_mod(&mut local, "Owner-Mod", "1.2.3");
    let remote = setup("2");
    let merged = review(&base, &base, &local, &remote).merged().unwrap();
    assert_eq!(merged.configs, remote.configs);
    assert_eq!(merged.mods, local.mods);
}

#[test]
fn both_changed_same_setting_requires_a_choice() {
    let mut result = review(&setup("1"), &setup("1"), &setup("2"), &setup("3"));
    assert_eq!(result.conflicts(), 1);
    assert!(result.merged().is_none());
    result.changes[0].take_remote = Some(false);
    assert_eq!(result.merged(), Some(setup("2")));
    result.changes[0].take_remote = Some(true);
    assert_eq!(result.merged(), Some(setup("3")));
}

#[test]
fn saving_local_changes_does_not_revert_pending_remote_changes() {
    let local = setup("1");
    let mut remote = setup("2");
    add_mod(&mut remote, "Owner-Mod", "1.2.3");
    let again = review(&local, &remote, &local, &remote);
    assert_eq!(again.merged(), Some(remote.clone()));
    assert_eq!(again.conflicts(), 0);
    let mut edited = local.clone();
    add_mod(&mut edited, "Owner-Another", "2.0.0");
    let merged = review(&local, &remote, &edited, &remote).merged().unwrap();
    assert_eq!(merged.configs, remote.configs);
    assert_eq!(merged.mods.len(), 2);
}

#[test]
fn removals_resets_and_identical_edits() {
    let mut base = setup("2");
    add_mod(&mut base, "Owner-Mod", "1.2.3");
    let remote = setup("1");
    assert_eq!(
        review(&base, &base, &base, &remote).merged(),
        Some(remote.clone())
    );
    assert!(review(&base, &base, &remote, &remote).changes.is_empty());
    assert_eq!(
        review(&base, &base, &Setup::default(), &base).merged(),
        Some(Setup::default())
    );
    let mut local = base.clone();
    add_mod(&mut local, "Owner-Mod", "2.0.0");
    assert_eq!(review(&base, &base, &local, &remote).conflicts(), 1);
}

#[test]
fn first_machine_restore_does_not_upload_empty_setup() {
    let empty = Setup::default();
    assert_eq!(
        review(&empty, &empty, &empty, &setup("2")).merged(),
        Some(setup("2"))
    );
}

#[test]
fn a_machine_local_path_never_deletes_or_replaces_the_cloud_value() {
    use super::Key;
    let local = Setup::default();
    let remote = setup("default");
    let mut result = review(&setup("old"), &setup("old"), &local, &remote);
    result.keep_local_only(
        &remote,
        &[Key::Setting {
            file: "example.cfg".to_owned(),
            section: "general".to_owned(),
            key: "count".to_owned(),
        }],
    );
    assert!(result.changes.is_empty());
    assert_eq!(result.merged(), Some(remote));
}

#[test]
fn unsafe_paths_and_secrets_are_rejected() {
    for value in [
        r"C:\Users\me\mods",
        "/Users/me/mods",
        r"\\server\mods",
        "~/mods",
    ] {
        assert!(!portable("folder", value));
    }
    assert!(!portable("API Key", "private"));
    assert!(portable("hotkey", "LeftControl"));
    for file in [
        "../escape.cfg",
        "C:/escape.cfg",
        "a\\escape.cfg",
        "/escape.cfg",
        "a/../escape.cfg",
    ] {
        let mut bad = Setup::default();
        bad.configs.insert(file.to_owned(), BTreeMap::new());
        assert!(validate(&bad).is_err(), "{file}");
    }
}

#[tokio::test]
async fn fresh_config_restore_preserves_local_secrets_and_round_trips() -> Result<()> {
    let temp = tempdir().unwrap();
    let store = ProfileStore::new(DataDir::at(temp.path().to_path_buf()));
    let profile = store.create("default", VALHEIM, Target::Client).await?;
    let dir = profile.dir().join("BepInEx/config");
    fs::create_dir_all(&dir).await.at(&dir)?;
    let file = dir.join("example.cfg");
    fs::write(&file, "[general]\r\nPassword = private\r\ncount = 1\r\n")
        .await
        .at(&file)?;
    let before = capture(&profile).await?;
    let mut wanted = setup("3");
    wanted
        .configs
        .insert("new.cfg".to_owned(), wanted.configs["example.cfg"].clone());
    write_settings(&profile, &before, &wanted).await?;
    let text = fs::read_to_string(&file).await.at(&file)?;
    assert!(text.contains("Password = private\r\n"));
    assert_eq!(ConfigFile::parse(&text).get("General", "Count")?.value, "3");
    assert_eq!(capture(&profile).await?, wanted);
    write_settings(&profile, &wanted, &Setup::default()).await?;
    let text = fs::read_to_string(&file).await.at(&file)?;
    assert!(text.contains("Password = private"));
    assert!(ConfigFile::parse(&text).get("general", "count").is_err());
    Ok(())
}

#[tokio::test]
async fn history_is_isolated_by_account_and_replaceable() -> Result<()> {
    let temp = tempdir().unwrap();
    let history = History {
        local: setup("1"),
        cloud: setup("2"),
    };
    history.save(temp.path(), "account-a").await?;
    history.save(temp.path(), "account-a").await?;
    assert_eq!(
        History::read(temp.path(), "account-a").await?.cloud,
        setup("2")
    );
    assert_eq!(
        History::read(temp.path(), "account-b").await?.cloud,
        Setup::default()
    );
    assert!(history.save(temp.path(), "../escape").await.is_err());
    Ok(())
}

#[tokio::test]
async fn interrupted_directory_swap_restores_working_profile() -> Result<()> {
    let temp = tempdir().unwrap();
    let dir = temp.path().join("default");
    let backup = dir.with_extension("cloud-backup");
    fs::create_dir_all(&backup).await.at(&backup)?;
    let file = backup.join("working");
    fs::write(&file, "original").await.at(&file)?;
    recover(&dir).await?;
    assert_eq!(
        fs::read_to_string(dir.join("working")).await.at(&dir)?,
        "original"
    );
    Ok(())
}

#[tokio::test]
async fn failed_install_keeps_live_files_then_retry_restores_exact_versions() -> Result<()> {
    use super::restore::stage;
    use crate::{forge::Forge, install::ZipCache, progress::Progress, testing::write_zip};

    let temp = tempdir().unwrap();
    let data = DataDir::at(temp.path().to_path_buf());
    let forge = Forge::at(data.clone())?;
    fs::create_dir_all(data.index_dir()).await.at(data.root())?;
    fs::write(
        data.index_dir().join("schema.json"),
        include_bytes!("../testdata/schema.json"),
    )
    .await
    .at(data.root())?;
    let profile = forge
        .store()
        .create("default", VALHEIM, Target::Client)
        .await?;
    let original = capture(&profile).await?;
    let marker = profile.dir().join("keep.txt");
    fs::write(&marker, "working installation")
        .await
        .at(&marker)?;
    let mut wanted = setup("9");
    add_mod(&mut wanted, "Owner-Mod", "1.2.3");
    let zip = ZipCache::new(&data).path(&"Owner-Mod-1.2.3".parse()?);
    fs::create_dir_all(zip.parent().unwrap()).await.at(&zip)?;
    fs::write(&zip, "broken download").await.at(&zip)?;
    assert!(
        stage(&forge, &profile, &wanted, &Progress::silent())
            .await
            .is_err()
    );
    assert_eq!(capture(&profile).await?, original);
    assert_eq!(
        fs::read_to_string(&marker).await.at(&marker)?,
        "working installation"
    );
    write_zip(&zip, &["plugins/mod.dll"])?;
    stage(&forge, &profile, &wanted, &Progress::silent()).await?;
    assert_eq!(capture(&profile).await?, wanted);
    assert_eq!(
        fs::read_to_string(&marker).await.at(&marker)?,
        "working installation"
    );
    recover(profile.dir()).await?;
    Ok(())
}
