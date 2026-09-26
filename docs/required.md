# Required mods

Some mods every player gets, always. The first one is the Sailing skill,
`Smoothbrain-Sailing`. Nobody adds it by hand, and nobody can remove it or
switch it off.

## The list

`backend/server/required.toml` holds the list per game, kept by hand. Each mod
has one exact version. The server serves it at `GET /api/required` to anybody,
no login, like the broken list. The app keeps it as `required.json` in the
`index` folder and reads it again after an hour. When the server cannot be
reached the old copy stays in use.

A change of the file is a backend push, no app release.

## Keep it in step with the servers

A mod on this list also goes into `THUNDERSTORE_MODS` of the game servers, at
the same version. For Valheim that is `games/valheim` for Durka and
`games/valheim-hard` for Arkham Asylum in the `local` repo. Sailing refuses a
join when the player and the server have different versions, so a new version
goes into the list and into both composes together. Pushing a compose restarts
the server, see the beekeeper skill.

Required mods are not listed in the server entries of the Servers page. Every
player already has them.

## In the profile

A required mod sits in `blackforge.toml` as a pin for the server
`Blackforge`, `REQUIRED_PIN_SERVER` in `crates/blackforge-api/src/required.rs`.
So the lock, cloud sync and `deploy` treat it like any other pinned mod.

- `Forge::sync` calls `Forge::apply_required` first. Every game start, every
  mod change and every deploy syncs, so the list is applied there. It adds a
  missing mod, moves it to the listed version and switches it on.
- When that step fails, the app logs a warning and goes on. A player without
  the mod can still join, only without the skill.
- A mod that left the list keeps its place in the profile but follows the
  newest version again, so the user can remove it.
- `Forge::remove` and switching the mod off are refused. Add and a server
  install leave its version alone.
- The Mods page shows a `required` pill and hides the switch and the trash
  button of the row.

## The parts

- `crates/blackforge-api/src/required.rs`, the wire type and the pin name.
- `backend/server/src/required.rs`, the route and the check of the file.
- `crates/blackforge-core/src/required.rs`, the list read and `apply`.
- `crates/blackforge-core/src/served_list.rs`, the cached fetch the broken and
  required lists share.

## Tests

`cargo test -p blackforge-core required` covers `apply`.
`cargo test -p blackforge-server required` checks the file. The Mods page and
the skill in the game are checked by hand.
