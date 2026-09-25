# Data folder

Everything lives in `~/.config/blackforge`, or in `BLACKFORGE_HOME` when set.

## config.toml

The machine config. It holds the active profile name and a `[game_dirs]` table.
An older `keep_achievements` flag here is only read for a profile without a
`launch.toml`. The table is only for game folders the user
gave by hand, like a server installed with `SteamCMD`. An empty table is
normal. The game is found through `Steam` by itself, see `game/locate.rs`.

## profiles/<name>

`blackforge.toml` is the mods the user asked for. A mod follows the newest
version, written `"*"`, or is pinned for a server, written
`{ version = "1.3.1", server = "Durka" }`. A pin always names its server, and
only a server install makes one. An older file has pins with no server, they
are all Durka pins and read as such.
`blackforge.lock` is the exact version of every package, dependencies included.
`launch.toml` is how the game of the profile starts, the extra game arguments and
the achievements switch. Cloud sync carries it, see `sync.md`.
`installed.json` is what is unpacked in the profile and which files belong to
which package. When all 3 agree, the profile is complete. A package that
left the lock loses its config and whatever else it wrote into the profile
at the next sync. A disabled mod stays in the lock, so its files stay.

Mod files stay in the profile under `BepInEx/plugins`. The game folder gets
only the doorstop loader. Launch points doorstop at the profile, so nothing
is copied into the game folder. `deploy` is the exception, it writes a plain
copy of a profile for a server that runs elsewhere.

`BepInEx/plugins/blackforge-join` is the join button plugin with its
`servers.json`, see `join.md`. Both are written before every start of the
Valheim client.

`BepInEx/plugins/blackforge-hugin` is the plugin that keeps the tutorial
ravens away, see `hugin.md`. It is written before every start of the Valheim
client too.

`BepInEx/plugins/blackforge` is the app's own achievements plugin. It is not a
package and is not in the lock.

## Other files

`cache` keeps downloaded zips. `icons` keeps mod icons. `index` keeps the
package list from Thunderstore, and `broken.json` next to it, the list of
broken mods from the blackforge server, fetched again after an hour. When
the server cannot be reached the old copy stays in use. `sync-<account-id>.json` is the cloud sync
baseline, see `sync.md`.
