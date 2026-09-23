use std::collections::BTreeMap;

use blackforge_api::setup::{Launch, Mod, Setup};
use tempfile::tempdir;
use tokio::fs;

use super::{
    Baseline, Key, LaunchKey, Snapshot, Step, capture,
    merge::review,
    plan, portable, recover,
    restore::{write_launch, write_settings},
    snapshot, validate,
};
use crate::{
    config::ConfigFile,
    error::{IoContext, Result},
    forge::Forge,
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

fn snap(setup: Setup) -> Snapshot {
    Snapshot {
        setup,
        local_only: Vec::new(),
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
            server: None,
        },
    );
}

#[test]
fn independent_mod_and_setting_changes_merge() {
    let base = setup("1");
    let mut local = base.clone();
    add_mod(&mut local, "Owner-Mod", "1.2.3");
    let remote = setup("2");
    let result = review(&base, &local, &remote);
    assert!(result.incoming());
    let merged = result.merged().unwrap();
    assert_eq!(merged.configs, remote.configs);
    assert_eq!(merged.mods, local.mods);
}

#[test]
fn both_changed_same_setting_is_a_conflict_with_two_whole_sides() {
    let result = review(&setup("1"), &setup("2"), &setup("3"));
    assert!(result.merged().is_none());
    assert_eq!(result.local_side(), setup("2"));
    assert_eq!(
        plan(Some(&setup("1")), &snap(setup("2")), Some(&setup("3")), &[]).unwrap(),
        Step::Conflict { local: setup("2") }
    );
}

#[test]
fn removals_resets_and_identical_edits() {
    let mut base = setup("2");
    add_mod(&mut base, "Owner-Mod", "1.2.3");
    let remote = setup("1");
    assert_eq!(review(&base, &base, &remote).merged(), Some(remote.clone()));
    assert!(review(&base, &remote, &remote).is_empty());
    assert_eq!(
        review(&base, &Setup::default(), &base).merged(),
        Some(Setup::default())
    );
    let mut local = base.clone();
    add_mod(&mut local, "Owner-Mod", "2.0.0");
    assert!(review(&base, &local, &remote).merged().is_none());
}

#[test]
fn only_local_changes_upload_without_an_install() {
    let base = setup("1");
    let mut local = base.clone();
    add_mod(&mut local, "Owner-Mod", "1.2.3");
    assert_eq!(
        plan(Some(&base), &snap(local.clone()), Some(&base), &[]).unwrap(),
        Step::Merge {
            merged: local,
            install: false,
            upload: true,
        }
    );
}

#[test]
fn only_cloud_changes_install_without_an_upload() {
    let base = setup("1");
    let remote = setup("2");
    assert_eq!(
        plan(Some(&base), &snap(base.clone()), Some(&remote), &[]).unwrap(),
        Step::Merge {
            merged: remote.clone(),
            install: true,
            upload: false,
        }
    );
    assert_eq!(
        plan(Some(&remote), &snap(remote.clone()), Some(&remote), &[]).unwrap(),
        Step::Settled
    );
}

// The upload is held behind the install. A machine that cannot install the
// cloud side must not save its own changes on top of it.
#[test]
fn changes_on_both_sides_install_before_they_upload() {
    let base = setup("1");
    let mut local = base.clone();
    add_mod(&mut local, "Owner-Mod", "1.2.3");
    let step = plan(Some(&base), &snap(local), Some(&setup("2")), &[]).unwrap();
    assert!(matches!(
        step,
        Step::Merge {
            install: true,
            upload: true,
            ..
        }
    ));
}

#[test]
fn an_account_with_nothing_saved_takes_the_local_setup() {
    let mut local = setup("1");
    add_mod(&mut local, "Owner-Mod", "1.2.3");
    // A stale baseline must not read the missing head as "everything removed".
    assert_eq!(
        plan(Some(&local), &snap(local.clone()), None, &[]).unwrap(),
        Step::Merge {
            merged: local,
            install: false,
            upload: true,
        }
    );
}

#[test]
fn first_sign_in_on_a_fresh_machine_takes_the_cloud_setup() {
    let loaders = ["denikson-BepInExPack_Valheim".to_owned()];
    let mut local = setup("1");
    add_mod(&mut local, &loaders[0], "5.4.2202");
    let mut remote = setup("2");
    add_mod(&mut remote, "Owner-Mod", "1.2.3");
    assert_eq!(
        plan(None, &snap(local), Some(&remote), &loaders).unwrap(),
        Step::Merge {
            merged: remote.clone(),
            install: true,
            upload: false,
        }
    );
}

#[test]
fn first_sign_in_with_own_mods_is_a_conflict_even_without_a_shared_key() {
    let mut local = Setup::default();
    add_mod(&mut local, "Owner-Mine", "1.0.0");
    let mut remote = Setup::default();
    add_mod(&mut remote, "Owner-Theirs", "1.0.0");
    assert_eq!(
        plan(None, &snap(local.clone()), Some(&remote), &[]).unwrap(),
        Step::Conflict { local }
    );
    assert_eq!(
        plan(None, &snap(remote.clone()), Some(&remote), &[]).unwrap(),
        Step::Settled
    );
}

#[test]
fn a_machine_local_path_never_deletes_or_replaces_the_cloud_value() {
    use super::Key;
    let local = Setup::default();
    let remote = setup("default");
    let mut result = review(&setup("old"), &local, &remote);
    result.keep_local_only(
        &remote,
        &[Key::Setting {
            file: "example.cfg".to_owned(),
            section: "general".to_owned(),
            key: "count".to_owned(),
        }],
    );
    assert!(result.is_empty());
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
async fn baseline_is_isolated_by_account_and_replaceable() -> Result<()> {
    let temp = tempdir().unwrap();
    let old = temp.path().join("cloud-account-a.json");
    fs::write(&old, "two snapshots of an old release")
        .await
        .at(&old)?;
    assert!(Baseline::read(temp.path(), "account-a").await?.is_none());
    Baseline { setup: setup("1") }
        .save(temp.path(), "account-a")
        .await?;
    Baseline { setup: setup("2") }
        .save(temp.path(), "account-a")
        .await?;
    assert_eq!(
        Baseline::read(temp.path(), "account-a")
            .await?
            .unwrap()
            .setup,
        setup("2")
    );
    assert!(!fs::try_exists(&old).await.at(&old)?);
    assert!(Baseline::read(temp.path(), "account-b").await?.is_none());
    assert!(
        Baseline::default()
            .save(temp.path(), "../escape")
            .await
            .is_err()
    );
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

#[test]
fn a_launch_change_merges_like_a_setting() {
    let base = setup("1");
    let mut local = setup("2");
    let mut remote = base.clone();
    remote.launch.keep_achievements = Some(true);
    let merged = review(&base, &local, &remote).merged().unwrap();
    assert_eq!(merged.launch.keep_achievements, Some(true));
    assert_eq!(merged.configs, local.configs);

    local.launch.keep_achievements = Some(false);
    assert!(review(&base, &local, &remote).merged().is_none());
}

#[test]
fn game_args_with_a_path_or_a_line_break_are_not_synced() {
    for args in ["-savedir /Users/me/saves", "-console\n-windowed"] {
        let mut bad = Setup::default();
        bad.launch.game_args = Some(args.to_owned());
        assert!(validate(&bad).is_err(), "{args}");
    }
}

#[tokio::test]
async fn launch_settings_round_trip_and_local_paths_stay() -> Result<()> {
    let temp = tempdir().unwrap();
    let data = DataDir::at(temp.path().to_path_buf());
    let forge = Forge::at(data.clone())?;
    let profile = ProfileStore::new(data)
        .create("default", VALHEIM, Target::Client)
        .await?;
    // A profile without a launch file syncs nothing of it.
    assert!(capture(&profile).await?.launch.is_empty());

    forge
        .edit_launch_settings(&profile, |settings| {
            settings.game_args = "-console".to_owned();
            settings.keep_achievements = true;
        })
        .await?;
    let captured = capture(&profile).await?;
    assert_eq!(
        captured.launch,
        Launch {
            game_args: Some("-console".to_owned()),
            keep_achievements: Some(true),
        }
    );

    let mut remote = captured;
    remote.launch.game_args = Some("-console -windowed".to_owned());
    remote.launch.keep_achievements = Some(false);
    write_launch(&forge, &profile, &remote).await?;
    assert_eq!(capture(&profile).await?, remote);

    forge
        .edit_launch_settings(&profile, |settings| {
            settings.game_args = "-savedir /Users/me/saves".to_owned();
        })
        .await?;
    let local = snapshot(&profile).await?;
    assert_eq!(local.setup.launch.game_args, None);
    assert_eq!(local.local_only, [Key::Launch(LaunchKey::GameArgs)]);
    write_launch(&forge, &profile, &remote).await?;
    assert_eq!(
        forge.launch_settings(&profile).await?.game_args,
        "-savedir /Users/me/saves"
    );
    Ok(())
}
