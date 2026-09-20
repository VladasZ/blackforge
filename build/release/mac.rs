// The mac release: a universal binary, the .app bundle, Developer ID signing,
// notarization and the dmg. A local run without the Apple secrets still
// produces an unsigned dmg to test the bundle with, but in CI a missing
// APPLE_SIGNING_IDENTITY fails the build so an unsigned release can never
// ship silently. Outputs in dist/:
//   <name>-<v>-mac-universal.dmg      first install
//   <name>-<v>-macos-universal        the bare binary the updater swaps in

use anyhow::{Context, Result, bail};
use shared::release::{self, Release};
use shared::run::{capture, run, run_secret};
use std::env::set_var;
use std::env::var;
use std::fs::copy;
use std::fs::create_dir_all;
use std::fs::remove_dir_all;
use std::fs::remove_file;
use std::fs::write;
use std::path::Path;

const TARGETS: [&str; 2] = ["aarch64-apple-darwin", "x86_64-apple-darwin"];

fn main() -> Result<()> {
    let r = release::read()?;
    create_dir_all("dist")?;
    create_dir_all("target/release")?;
    let signing = var("APPLE_SIGNING_IDENTITY").ok().filter(|s| !s.is_empty());
    if signing.is_none() && var("CI").is_ok() {
        bail!("APPLE_SIGNING_IDENTITY is not set, refusing to build an unsigned release in CI");
    }
    if signing.is_some() {
        unlock_keychain()?;
    }

    // The runner shell starts without the interactive env, so cc has no
    // sysroot without this and native C deps fail to compile.
    let sdk = capture("xcrun --sdk macosx --show-sdk-path")?;
    unsafe {
        set_var("SDKROOT", sdk);
    }
    for target in TARGETS {
        run(&format!("rustup target add {target}"))?;
        run(&format!(
            "cargo build --locked --release -p blackforge --bin blackforge-gui --target {target}"
        ))?;
    }
    let universal = format!("target/release/{}-universal", r.name);
    run(&format!(
        "lipo -create -output {universal} target/{}/release/{} target/{}/release/{}",
        TARGETS[0], "blackforge-gui", TARGETS[1], "blackforge-gui"
    ))?;

    let app = bundle(&r, &universal)?;
    if let Some(identity) = &signing {
        run(&format!(
            r#"codesign --force --deep --options runtime --timestamp --sign "{identity}" "{app}""#
        ))?;
        run(&format!(
            r#"codesign --force --options runtime --timestamp --sign "{identity}" "{universal}""#
        ))?;
        notarize_app(&app)?;
    }

    let dmg = format!("dist/{}", r.artifact("mac-universal.dmg"));
    make_dmg(&r, &app, &dmg)?;
    if signing.is_some() {
        notarize(&dmg)?;
    }
    println!("built {dmg}");

    let bare = format!("dist/{}", r.artifact("macos-universal"));
    copy(&universal, &bare)?;
    run(&format!(
        "cargo run --locked --manifest-path build/Cargo.toml --bin release-sign -- {bare}"
    ))?;
    println!("built {bare}");
    Ok(())
}

fn unlock_keychain() -> Result<()> {
    var("APPLE_CI_KEYCHAIN_PASSWORD").context("APPLE_CI_KEYCHAIN_PASSWORD")?;
    let keychain = "~/Library/Keychains/ci-signing.keychain-db";
    run_secret(
        r#"security unlock-keychain -p "$APPLE_CI_KEYCHAIN_PASSWORD" ~/Library/Keychains/ci-signing.keychain-db"#,
    )?;
    run_secret(
        r#"security set-key-partition-list -S apple-tool:,apple: -k "$APPLE_CI_KEYCHAIN_PASSWORD" ~/Library/Keychains/ci-signing.keychain-db > /dev/null"#,
    )?;
    run(&format!(
        "security list-keychains -d user -s {keychain} ~/Library/Keychains/login.keychain-db"
    ))?;
    Ok(())
}

fn bundle(r: &Release, universal: &str) -> Result<String> {
    let app = format!("target/release/bundle/{}.app", r.name);
    let contents = format!("{app}/Contents");
    if Path::new(&app).exists() {
        remove_dir_all(&app)?;
    }
    create_dir_all(format!("{contents}/MacOS"))?;
    create_dir_all(format!("{contents}/Resources"))?;
    copy(universal, format!("{contents}/MacOS/{}", r.name))?;
    write(format!("{contents}/Info.plist"), info_plist(r))?;
    Ok(app)
}

fn info_plist(r: &Release) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>{name}</string>
  <key>CFBundleDisplayName</key><string>{name}</string>
  <key>CFBundleIdentifier</key><string>{bundle_id}</string>
  <key>CFBundleExecutable</key><string>{name}</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>{version}</string>
  <key>CFBundleVersion</key><string>{version}</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.games</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
"#,
        name = r.name,
        bundle_id = r.bundle_id,
        version = r.version
    )
}

// The app is notarized on its own before it goes into the dmg, so the
// bundle carries a stapled ticket and launches offline after a drag install.
fn notarize_app(app: &str) -> Result<()> {
    let zip = format!("{app}.zip");
    run(&format!(r#"ditto -c -k --keepParent "{app}" "{zip}""#))?;
    notarize(&zip)?;
    run(&format!(r#"xcrun stapler staple "{app}""#))?;
    remove_file(zip)?;
    Ok(())
}

fn notarize(file: &str) -> Result<()> {
    for name in [
        "APPLE_ID_EMAIL",
        "APPLE_APP_SPECIFIC_PASSWORD",
        "APPLE_TEAM_ID",
    ] {
        var(name).with_context(|| format!("{name} is missing"))?;
    }
    run_secret(&format!(
        r#"xcrun notarytool submit "{file}" --apple-id "$APPLE_ID_EMAIL" --password "$APPLE_APP_SPECIFIC_PASSWORD" --team-id "$APPLE_TEAM_ID" --wait"#
    ))?;
    if file.ends_with(".dmg") {
        run(&format!(r#"xcrun stapler staple "{file}""#))?;
    }
    Ok(())
}

fn make_dmg(r: &Release, app: &str, dmg: &str) -> Result<()> {
    let staging = "target/release/bundle/dmg";
    if Path::new(staging).exists() {
        remove_dir_all(staging)?;
    }
    create_dir_all(staging)?;
    run(&format!(r#"cp -R "{app}" {staging}/"#))?;
    run(&format!("ln -s /Applications {staging}/Applications"))?;
    if Path::new(dmg).exists() {
        remove_file(dmg)?;
    }
    run(&format!(
        r#"hdiutil create -volname "{}" -srcfolder {staging} -ov -format UDZO "{dmg}""#,
        r.name
    ))?;
    Ok(())
}
