# Sync between machines

Signing in with the same Google account keeps the desktop app's `default` profile
the same on every machine. It works like Steam cloud saves: no buttons, every
change uploads, and another machine installs it by itself. Only the main setup
is synced. The CLI can edit that profile; the desktop app picks up those changes
when it next checks.

## What is saved

The private snapshot includes chosen mods, exact locked versions and dependencies,
the server a pin is for, enabled states, and portable values from every `.cfg` file under `BepInEx/config`.
It includes default values and settings without default comments, so it can restore
a fresh installation. It also carries the launch settings of the profile, the extra
game arguments and the achievements switch from `launch.toml`. They merge like config
settings. A setup saved by an app before 0.1.16 has none, and a machine keeps its
own until it changes one. It does not upload game saves, mod archives, login tokens
or game folders. Secret-looking values and absolute local paths are excluded, game
arguments with a path of this machine stay local too. This uses separate routes and a separate table from friend sharing.

A setup from an older app has pins without a server. Reading it, in the app
and in the backend, gives them `LEGACY_PIN_SERVER`, Durka, and migration `0006`
rewrote the stored revisions the same way. So an old app in the fleet only
causes a harmless rewrite, never a pin without a server.

## When it runs

A sync runs at launch, after every profile change, after a config edit, after the
game exits, before the game starts, and every minute while signed in and the game
is closed. The Run button waits for the sync, the same way Steam does. If the
server cannot be reached or an install fails, the game starts on the local setup.

After the sync the Run button installs every mod file the lock lists and the
install state lacks, so there is no separate install button. A new `default`
profile installs the mod loader right after it is created.

## What one sync does

1. Read the head, the newest applied revision of the account.
2. Compare three setups: the baseline this machine and the cloud last agreed on,
   the local profile, and the head.
3. Changes to different mods or settings merge by themselves.
4. The cloud side is installed first. Only then the merged setup uploads. A
   machine never saves on top of a setup it could not install.
5. The merged setup becomes the new baseline in `sync-<account-id>.json`.

The server compares the base revision of an upload with the head, so a stale
machine cannot overwrite newer changes. It reads the new head and tries again.

A failed install keeps the local profile, shows the reason in the status line,
holds the upload and retries at the next check. The way out of a setup that can
never install, such as a mod version removed from Thunderstore, is a restore
from the history.

## Conflicts

A conflict means both sides changed the same mod or setting, or the merge would
pair a mod with a dependency the other side removed. Then a dialog asks for one
whole side: this machine or the cloud. It has no close button and sync waits for
the answer. Picking the cloud first uploads the local setup as a revision marked
as not applied, so a wrong pick can be restored.

A machine that signs in for the first time has no baseline. A fresh profile, one
that only holds the mod loader, takes the cloud setup with no dialog. A profile
with own mods that differs from the cloud is a conflict.

## History and restore

Every upload is a revision and none is ever removed. Each revision stores the
host name of the machine, the time, and a summary against the head before it.
The History button on the cloud sync card above the Mods list shows them, newest
first. Restore copies an old setup into a new head revision. Every machine, this
one included, then installs it like any other change, and a restore can itself be
restored.

Revisions marked as not applied never become the head. No machine installs one
unless the user restores it.

## Installing

An install goes into a sibling staging folder. Only a completed install replaces
the live profile. A failed download leaves the working profile and the baseline
intact. A backup directory makes an interrupted directory swap recoverable at the
next startup. Exact versions must still be available, and the chosen dependencies
must satisfy the package metadata. Existing local secrets and paths are retained
when settings are written.

## Code and storage

- `crates/blackforge-api/src/setup.rs`: request and response types, and the
  change summary both sides use.
- `backend/server/src/sync.rs`: authenticated `GET /api/sync`, `POST /api/sync`,
  `GET /api/sync/history` and `POST /api/sync/restore`. A save locks the account
  row, so the compare with the head and the insert cannot interleave.
- Migration `0004_cloud_revisions.sql`: one `cloud_revisions` row per revision.
  It replaced the one row per account of `0003`. Account deletion cascades.
- `crates/blackforge-core/src/cloud`: capture, merge, the `plan` step, the
  baseline file, staged install and recovery.
- `crates/blackforge/src/cloud.rs`: the automatic flow, conflict handling,
  history and restore.
- `crates/blackforge/src/ui/sync_panel.rs`: the cloud sync card with sign in, the
  status line and the History button. `conflict_dialog.rs` and `history_modal.rs`
  are the two modals.

The routes use the authenticated account directly and do not require a public
username. Friends cannot read the private snapshots.

Releases up to 0.1.8 used `GET` and `PUT /api/setup` with a manual review step.
Those routes are gone, so these releases cannot sync until they update.

## Checks

`cargo test --workspace` covers merging, removals, resets, conflicts, install
before upload, an account with nothing saved, first sign in on a fresh machine
and with own mods, account isolation of the baseline, portable settings, fresh
config files, failed installation and recovery, the change summary, and rejection
of anonymous API requests. Database-backed route checks and a physical round trip
between two machines remain separate from these automated tests.
