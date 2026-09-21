# Sync between machines

Signing in with the same Google account saves the desktop app's `default` profile
for other machines. Only the main setup is synced. The CLI can edit that profile;
the desktop app picks up those changes when it next checks.

## What is saved

The private snapshot includes chosen mods, exact locked versions and dependencies,
enabled states, and portable values from every `.cfg` file under `BepInEx/config`.
It includes default values and settings without default comments, so it can restore
a fresh installation. It does not upload game saves, mod archives, login tokens,
game folders or launch arguments. Secret-looking values and absolute local paths
are excluded. This uses a separate route and table from friend sharing.

## Saving and applying

Checks run at launch, after profile changes, after the game exits, and every minute
while signed in and the game is closed. Network failures retain local changes for
the next check. A fresh machine reads the cloud before attempting an upload.

Independent changes merge automatically. Conflicts wait for review in the cloud
sync area above the Mods list. The existing config picker shows local and cloud values for both mods and
settings. Every conflict needs a choice. Applying is always manual; background
saves never change the installed mods or settings.

Each account has a local `cloud-<account-id>.json` history with separate snapshots
of the last observed local setup and last acknowledged cloud setup. Saving local
changes does not mark pending remote changes as installed. Server writes compare
the supplied revision with the stored revision, so stale clients cannot overwrite
newer changes. Apply rechecks the account, revision and local snapshot.

Apply saves the chosen cloud setup and installs it into a sibling staging folder.
Only a completed install replaces the live profile. A failed download leaves the
working profile and sync history intact for retry. A backup directory makes an
interrupted directory swap recoverable at the next startup. Exact versions must
still be available, and the chosen dependencies must satisfy the package metadata.
Existing local secrets and paths are retained when settings are written.

## Code and storage

- `crates/blackforge-api/src/setup.rs`: private request and response types.
- `backend/server/src/setup.rs`: authenticated `GET /api/setup` and `PUT /api/setup`.
- Migration `0003_cloud_setup.sql`: one `cloud_setups` row per account, with a
  revision and JSON snapshot. Account deletion cascades to this row.
- `crates/blackforge-core/src/cloud`: capture, merge, validation, local history,
  staged install and recovery.
- `crates/blackforge/src/cloud.rs`: automatic checks and explicit apply.
- `crates/blackforge/src/ui/sync_panel.rs`: sign in, status and review above the Mods list.

The routes use the authenticated account directly and do not require a public
username. Friends cannot read the private snapshot. Old app releases keep using
the unchanged friend-sharing API.

## Checks

`cargo test --workspace` covers merging, removals, resets, conflict choices,
pending remote changes, account isolation, portable settings, fresh config files,
failed installation and recovery, and rejection of anonymous API requests.
The Windows desktop Mods page was checked using an isolated `BLACKFORGE_HOME`.
Database-backed and physical Windows-to-Mac round-trip checks remain separate
from these automated tests.
