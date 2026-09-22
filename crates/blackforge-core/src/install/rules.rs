//! Decides where each file of a mod zip goes inside a profile.
//!
//! This follows `InstallRulePluginInstaller` of r2modman step by step. Mods
//! are built and tested against that behavior, flattening included, so any
//! difference here would break mods that load assets by a fixed path.

use std::collections::BTreeMap;

use crate::{
    error::{Error, Result},
    game::{GameDef, InstallRule, LoaderPackage, TrackingMethod},
    ident::PackageId,
};

/// One file to write. Paths use `/` and are relative, `entry` inside the zip
/// and `dest` inside the profile.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Placement {
    pub entry: String,
    pub dest: String,
    /// An untracked file belongs to the user once written, a config for
    /// example. It is never overwritten and never removed.
    pub tracked: bool,
}

struct FlatRule {
    route: String,
    tracking: TrackingMethod,
    extensions: Vec<String>,
    is_default: bool,
}

fn flatten(rules: &[InstallRule], parent: Option<&str>, out: &mut Vec<FlatRule>) {
    for rule in rules {
        let route = parent.map_or_else(
            || rule.route.clone(),
            |parent| format!("{parent}/{}", rule.route),
        );
        out.push(FlatRule {
            route: route.clone(),
            tracking: rule.tracking_method,
            extensions: rule.default_file_extensions.clone(),
            is_default: rule.is_default_location,
        });
        flatten(&rule.sub_routes, Some(&route), out);
    }
}

#[derive(Default)]
struct Dir {
    /// File name and the zip entry it came from.
    files: Vec<(String, String)>,
    dirs: BTreeMap<String, Dir>,
}

impl Dir {
    fn insert(&mut self, parts: &[String], entry: &str) {
        match parts {
            [] => {}
            [name] => self.files.push((name.clone(), entry.to_owned())),
            [dir, rest @ ..] => self
                .dirs
                .entry(dir.clone())
                .or_default()
                .insert(rest, entry),
        }
    }

    /// Every file below this folder with its path relative to the folder.
    fn all_files(&self, prefix: &str, out: &mut Vec<(String, String)>) {
        for (name, entry) in &self.files {
            out.push((format!("{prefix}{name}"), entry.clone()));
        }
        for (name, dir) in &self.dirs {
            dir.all_files(&format!("{prefix}{name}/"), out);
        }
    }
}

/// Splits a zip entry name into safe parts. Zips made on Windows can use `\`.
/// `None` for a folder entry and for a name that tries to leave the root.
pub fn entry_parts(name: &str) -> Option<Vec<String>> {
    let normalized = name.replace('\\', "/");
    if normalized.ends_with('/') {
        return None;
    }
    let mut parts = Vec::new();
    for part in normalized.split('/') {
        match part {
            "" | "." => {}
            ".." => return None,
            part if part.contains(':') => return None,
            part => parts.push(part.to_owned()),
        }
    }
    (!parts.is_empty()).then_some(parts)
}

fn tree_of(entries: &[String]) -> Dir {
    let mut root = Dir::default();
    for entry in entries {
        if let Some(parts) = entry_parts(entry) {
            root.insert(&parts, entry);
        }
    }
    root
}

/// The install routes that are not tracked, the config folders. A file
/// there belongs to the user once written.
pub fn untracked_routes(game: &GameDef) -> Vec<String> {
    let mut rules = Vec::new();
    flatten(&game.install_rules, None, &mut rules);
    rules
        .into_iter()
        .filter(|rule| rule.tracking == TrackingMethod::None)
        .map(|rule| rule.route)
        .collect()
}

pub fn plan_mod(game: &GameDef, id: &PackageId, entries: &[String]) -> Result<Vec<Placement>> {
    let mut rules = Vec::new();
    flatten(&game.install_rules, None, &mut rules);
    let planner = Planner {
        rules,
        exclusions: &game.file_exclusions,
        mod_name: id.to_string(),
    };
    let mut out = Vec::new();
    planner.visit(&tree_of(entries), "", &mut out)?;
    out.sort();
    Ok(out)
}

/// The loader zip keeps the loader inside one root folder. That folder goes
/// into the profile root as it is.
pub fn plan_loader(game: &GameDef, loader: &LoaderPackage, entries: &[String]) -> Vec<Placement> {
    const PACKAGE_FILES: [&str; 3] = ["manifest.json", "readme.md", "icon.png"];
    let mut rules = Vec::new();
    flatten(&game.install_rules, None, &mut rules);

    let mut out = Vec::new();
    for entry in entries {
        let Some(parts) = entry_parts(entry) else {
            continue;
        };
        let inside = if loader.root_folder.is_empty() {
            parts.as_slice()
        } else if parts
            .first()
            .is_some_and(|first| first == &loader.root_folder)
        {
            &parts[1..]
        } else {
            continue;
        };
        let [first, ..] = inside else {
            continue;
        };
        if inside.len() == 1 && PACKAGE_FILES.contains(&first.to_lowercase().as_str()) {
            continue;
        }
        let dest = inside.join("/");
        let tracked = !rules.iter().any(|rule| {
            rule.tracking == TrackingMethod::None && dest.starts_with(&format!("{}/", rule.route))
        });
        out.push(Placement {
            entry: entry.clone(),
            dest,
            tracked,
        });
    }
    out.sort();
    out
}

struct Planner<'a> {
    rules: Vec<FlatRule>,
    exclusions: &'a [String],
    mod_name: String,
}

impl Planner<'_> {
    fn visit(&self, dir: &Dir, path: &str, out: &mut Vec<Placement>) -> Result<()> {
        for (name, entry) in &dir.files {
            if let Some(rule) = self.rule_for_file(name) {
                let relative = format!("{path}{name}");
                self.place(rule, name, &relative, entry, out)?;
            }
        }
        for (name, sub) in &dir.dirs {
            let sub_path = format!("{path}{name}/");
            match self.rule_for_dir(name, &sub_path) {
                // A folder no rule knows is walked, which flattens its files.
                None => self.visit(sub, &sub_path, out)?,
                Some(rule) => {
                    let mut files = Vec::new();
                    sub.all_files("", &mut files);
                    for (inner, entry) in files {
                        self.place(rule, &inner, &inner, &entry, out)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// The longest matching extension wins, so `.mm.dll` beats `.dll`. On a
    /// tie the later rule wins. A file that matches nothing goes to the
    /// default rule.
    fn rule_for_file(&self, name: &str) -> Option<&FlatRule> {
        let lower = name.to_lowercase();
        let mut best: Option<(&FlatRule, usize)> = None;
        for rule in &self.rules {
            let matched = rule
                .extensions
                .iter()
                .find(|extension| lower.ends_with(&extension.to_lowercase()))
                .map(String::len);
            if let Some(length) = matched
                && best.is_none_or(|(_, best_length)| length >= best_length)
            {
                best = Some((rule, length));
            }
        }
        best.map(|(rule, _)| rule)
            .or_else(|| self.rules.iter().find(|rule| rule.is_default))
    }

    /// A folder matches a rule by the last part of the route. When several
    /// rules match, the one that shares the most path parts wins, the first
    /// one on a tie.
    fn rule_for_dir(&self, name: &str, path: &str) -> Option<&FlatRule> {
        let lower = name.to_lowercase();
        let file_parts: Vec<&str> = path.trim_end_matches('/').rsplit('/').collect();
        let mut best: Option<(&FlatRule, usize)> = None;
        for rule in &self.rules {
            if rule
                .route
                .rsplit('/')
                .next()
                .is_none_or(|last| last.to_lowercase() != lower)
            {
                continue;
            }
            let shared = rule
                .route
                .rsplit('/')
                .zip(&file_parts)
                .filter(|(a, b)| a == *b)
                .count();
            if best.is_none_or(|(_, best_shared)| shared > best_shared) {
                best = Some((rule, shared));
            }
        }
        best.map(|(rule, _)| rule)
    }

    /// `inner` is the path the file keeps below the rule folder. `relative`
    /// is its path from the zip root, which only `subdir-no-flatten` uses.
    fn place(
        &self,
        rule: &FlatRule,
        inner: &str,
        relative: &str,
        entry: &str,
        out: &mut Vec<Placement>,
    ) -> Result<()> {
        let route = &rule.route;
        let mod_name = &self.mod_name;
        let (dest, tracked) = match rule.tracking {
            TrackingMethod::Subdir => (format!("{route}/{mod_name}/{inner}"), true),
            TrackingMethod::SubdirNoFlatten => (format!("{route}/{mod_name}/{relative}"), true),
            TrackingMethod::None => (format!("{route}/{inner}"), false),
            TrackingMethod::State => {
                let lower = inner.to_lowercase();
                if self
                    .exclusions
                    .iter()
                    .any(|excluded| excluded.to_lowercase() == lower)
                {
                    return Ok(());
                }
                (format!("{route}/{inner}"), true)
            }
            TrackingMethod::PackageZip => {
                return Err(Error::Unsupported(format!(
                    "install rule 'package-zip' of route {route}"
                )));
            }
        };
        out.push(Placement {
            entry: entry.to_owned(),
            dest,
            tracked,
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{game::Target, testing::valheim};

    fn plan(entries: &[&str]) -> Result<Vec<(String, String)>> {
        let entries: Vec<String> = entries.iter().map(|entry| (*entry).to_owned()).collect();
        let placements = plan_mod(&valheim(Target::Client)?, &"Owner-Mod".parse()?, &entries)?;
        Ok(placements
            .into_iter()
            .map(|placement| (placement.entry, placement.dest))
            .collect())
    }

    fn pairs(expected: &[(&str, &str)]) -> Vec<(String, String)> {
        let mut pairs: Vec<_> = expected
            .iter()
            .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
            .collect();
        pairs.sort();
        pairs
    }

    #[test]
    fn root_files_go_to_the_mod_folder_in_plugins() -> Result<()> {
        assert_eq!(
            plan(&["manifest.json", "icon.png", "README.md", "Mod.dll"])?,
            pairs(&[
                ("manifest.json", "BepInEx/plugins/Owner-Mod/manifest.json"),
                ("icon.png", "BepInEx/plugins/Owner-Mod/icon.png"),
                ("README.md", "BepInEx/plugins/Owner-Mod/README.md"),
                ("Mod.dll", "BepInEx/plugins/Owner-Mod/Mod.dll"),
            ])
        );
        Ok(())
    }

    #[test]
    fn unknown_folders_are_flattened() -> Result<()> {
        assert_eq!(
            plan(&["files/Mod.dll", "files/assets/icon.png"])?,
            pairs(&[
                ("files/Mod.dll", "BepInEx/plugins/Owner-Mod/Mod.dll"),
                (
                    "files/assets/icon.png",
                    "BepInEx/plugins/Owner-Mod/icon.png"
                ),
            ])
        );
        Ok(())
    }

    #[test]
    fn rule_folders_keep_their_inner_layout() -> Result<()> {
        assert_eq!(
            plan(&[
                "BepInEx/plugins/Mod.dll",
                "BepInEx/plugins/assets/bundle",
                "patchers/Patch.dll",
                "BepInEx/config/owner.mod.cfg",
                "Config/other.cfg",
            ])?,
            pairs(&[
                (
                    "BepInEx/plugins/Mod.dll",
                    "BepInEx/plugins/Owner-Mod/Mod.dll"
                ),
                (
                    "BepInEx/plugins/assets/bundle",
                    "BepInEx/plugins/Owner-Mod/assets/bundle"
                ),
                ("patchers/Patch.dll", "BepInEx/patchers/Owner-Mod/Patch.dll"),
                (
                    "BepInEx/config/owner.mod.cfg",
                    "BepInEx/config/owner.mod.cfg"
                ),
                ("Config/other.cfg", "BepInEx/config/other.cfg"),
            ])
        );
        Ok(())
    }

    #[test]
    fn longest_extension_wins() -> Result<()> {
        assert_eq!(
            plan(&["Assembly.mm.dll"])?,
            pairs(&[(
                "Assembly.mm.dll",
                "BepInEx/monomod/Owner-Mod/Assembly.mm.dll"
            )])
        );
        Ok(())
    }

    #[test]
    fn config_files_are_untracked() -> Result<()> {
        let entries = vec!["config/a.cfg".to_owned(), "Mod.dll".to_owned()];
        let placements = plan_mod(&valheim(Target::Client)?, &"Owner-Mod".parse()?, &entries)?;
        let tracked: Vec<bool> = placements
            .iter()
            .map(|placement| placement.tracked)
            .collect();
        assert_eq!(tracked, [true, false]);
        Ok(())
    }

    #[test]
    fn unsafe_entries_are_dropped() {
        assert_eq!(entry_parts("../evil.dll"), None);
        assert_eq!(entry_parts("a/../../evil.dll"), None);
        assert_eq!(entry_parts("C:/evil.dll"), None);
        assert_eq!(entry_parts("folder/"), None);
        assert_eq!(
            entry_parts("a\\b\\c.dll"),
            Some(vec!["a".to_owned(), "b".to_owned(), "c.dll".to_owned()])
        );
    }

    #[test]
    fn loader_root_folder_goes_to_the_profile_root() -> Result<()> {
        let game = valheim(Target::Client)?;
        let id = "denikson-BepInExPack_Valheim".parse()?;
        let loader = game
            .loader_package(&id)
            .ok_or_else(|| Error::Invalid("loader not in fixture".to_owned()))?;
        let entries: Vec<String> = [
            "manifest.json",
            "icon.png",
            "BepInExPack_Valheim/winhttp.dll",
            "BepInExPack_Valheim/doorstop_libs/libdoorstop_x64.dylib",
            "BepInExPack_Valheim/BepInEx/core/BepInEx.Preloader.dll",
            "BepInExPack_Valheim/BepInEx/config/BepInEx.cfg",
        ]
        .iter()
        .map(|entry| (*entry).to_owned())
        .collect();
        let placed: Vec<(String, bool)> = plan_loader(&game, loader, &entries)
            .into_iter()
            .map(|placement| (placement.dest, placement.tracked))
            .collect();
        assert_eq!(
            placed,
            [
                ("BepInEx/config/BepInEx.cfg".to_owned(), false),
                ("BepInEx/core/BepInEx.Preloader.dll".to_owned(), true),
                ("doorstop_libs/libdoorstop_x64.dylib".to_owned(), true),
                ("winhttp.dll".to_owned(), true),
            ]
        );
        Ok(())
    }
}
