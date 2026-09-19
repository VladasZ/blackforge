use anyhow::{Result, bail};
use blackforge_core::{
    game::{Target, VALHEIM},
    manifest::{Manifest, VersionReq},
    thunderstore::{FRESH_ENOUGH, PackageIndex},
};

use crate::{
    Context,
    ui::{Ui, print_lock_change, print_sync_report, print_table},
};

/// `Owner-Name@1.2.3` pins a version, without `@` the mod follows the newest.
fn split_version(spec: &str) -> Result<(&str, VersionReq)> {
    Ok(match spec.split_once('@') {
        Some((name, version)) => (name, version.parse()?),
        None => (spec, VersionReq::Latest),
    })
}

pub async fn sync(context: &Context) -> Result<()> {
    let profile = context.profile().await?;
    let ui = Ui::start();
    let report = context.forge.sync(&profile, &ui.progress).await;
    ui.finish().await?;
    print_sync_report(&report?);
    Ok(())
}

pub async fn add(context: &Context, mods: &[String]) -> Result<()> {
    let profile = context.profile().await?;
    for spec in mods {
        let (name, version) = split_version(spec)?;
        let ui = Ui::start();
        let result = context
            .forge
            .add(&profile, name, version, &ui.progress)
            .await;
        ui.finish().await?;
        let (id, change) = result?;
        println!("added {id}");
        print_lock_change(&change);
    }
    sync(context).await
}

pub async fn remove(context: &Context, mods: &[String]) -> Result<()> {
    let profile = context.profile().await?;
    for name in mods {
        let (id, change) = context.forge.remove(&profile, name).await?;
        println!("removed {id}");
        print_lock_change(&change);
    }
    sync(context).await
}

pub async fn update(context: &Context, mods: &[String]) -> Result<()> {
    let profile = context.profile().await?;
    let ui = Ui::start();
    let change = context.forge.update(&profile, mods, &ui.progress).await;
    ui.finish().await?;
    print_lock_change(&change?);
    sync(context).await
}

pub async fn outdated(context: &Context) -> Result<()> {
    let profile = context.profile().await?;
    let ui = Ui::start();
    let outdated = context.forge.outdated(&profile, &ui.progress).await;
    ui.finish().await?;
    let outdated = outdated?;
    if outdated.is_empty() {
        println!("every mod is on its newest version");
        return Ok(());
    }
    let mut rows = vec![vec![
        "mod".to_owned(),
        "locked".to_owned(),
        "newest".to_owned(),
        String::new(),
    ]];
    for entry in outdated {
        let note = if entry.pinned {
            "pinned in the manifest"
        } else {
            ""
        };
        rows.push(vec![
            entry.id.to_string(),
            entry.locked.to_string(),
            entry.latest.to_string(),
            note.to_owned(),
        ]);
    }
    print_table(&rows);
    Ok(())
}

pub async fn set_enabled(context: &Context, name: &str, enabled: bool) -> Result<()> {
    let profile = context.profile().await?;
    let id = context.forge.set_enabled(&profile, name, enabled).await?;
    println!("{} {id}", if enabled { "enabled" } else { "disabled" });
    sync(context).await
}

pub async fn list(context: &Context) -> Result<()> {
    let profile = context.profile().await?;
    let manifest = profile.manifest().await?;
    let lock = profile.lock().await?;
    println!(
        "profile {}, {} {}",
        profile.name(),
        manifest.game,
        manifest.target
    );
    if lock.packages.is_empty() {
        println!("no mods yet");
        return Ok(());
    }
    let mut rows = Vec::new();
    for package in &lock.packages {
        let note = match manifest.mods.get(&package.id) {
            Some(spec) if !spec.enabled => "disabled",
            Some(spec) if spec.version != VersionReq::Latest => "pinned",
            Some(_) => "",
            None => "dependency",
        };
        rows.push(vec![
            package.id.to_string(),
            package.version.to_string(),
            note.to_owned(),
        ]);
    }
    print_table(&rows);
    Ok(())
}

/// The package list for a command that only reads it. It works without a
/// profile and does not create one, a search should leave no trace.
async fn index(context: &Context) -> Result<PackageIndex> {
    let manifest = match context.existing_profile().await? {
        Some(profile) => profile.manifest().await?,
        None => Manifest::new(VALHEIM, Target::Client),
    };
    let game = context.forge.game(&manifest).await?;
    let ui = Ui::start();
    let index = context.forge.index(&game, FRESH_ENOUGH, &ui.progress).await;
    ui.finish().await?;
    Ok(index?)
}

pub async fn search(context: &Context, query: &str, limit: usize) -> Result<()> {
    let index = index(context).await?;
    let hits = index.search(query);
    if hits.is_empty() {
        bail!("nothing on Thunderstore matches '{query}'");
    }
    let mut rows = Vec::new();
    for package in hits.iter().take(limit) {
        let version = package
            .latest()
            .map(|latest| latest.version.to_string())
            .unwrap_or_default();
        rows.push(vec![
            package.id.to_string(),
            version,
            package.downloads.to_string(),
            shorten(&package.description, 70),
        ]);
    }
    print_table(&rows);
    if hits.len() > limit {
        println!(
            "{} more, narrow the search or raise --limit",
            hits.len() - limit
        );
    }
    Ok(())
}

fn shorten(text: &str, max: usize) -> String {
    let line = text.lines().next().unwrap_or_default();
    if line.chars().count() <= max {
        return line.to_owned();
    }
    let cut: String = line.chars().take(max.saturating_sub(3)).collect();
    format!("{cut}...")
}

pub async fn info(context: &Context, name: &str) -> Result<()> {
    let index = index(context).await?;
    let package = index.find(name)?;
    let latest = package.latest();
    let mut rows = vec![
        vec!["name".to_owned(), package.id.to_string()],
        vec![
            "newest".to_owned(),
            latest
                .map(|latest| latest.version.to_string())
                .unwrap_or_default(),
        ],
        vec!["updated".to_owned(), package.updated.clone()],
        vec!["downloads".to_owned(), package.downloads.to_string()],
        vec!["rating".to_owned(), package.rating.to_string()],
        vec!["categories".to_owned(), package.categories.join(", ")],
        vec!["page".to_owned(), package.package_url.clone()],
        vec!["website".to_owned(), package.website_url.clone()],
    ];
    if package.deprecated {
        rows.push(vec!["status".to_owned(), "deprecated".to_owned()]);
    }
    print_table(&rows);
    println!();
    println!("{}", package.description);
    if let Some(latest) = latest
        && !latest.dependencies.is_empty()
    {
        println!();
        println!("needs:");
        for dependency in &latest.dependencies {
            println!("  {dependency}");
        }
    }
    let versions: Vec<String> = package
        .versions
        .iter()
        .take(10)
        .map(|release| release.version.to_string())
        .collect();
    println!();
    println!("versions: {}", versions.join(", "));
    Ok(())
}
