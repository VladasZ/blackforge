Read `docs/data.md` before looking at the user's data folder or profiles.
Read `docs/sync.md` before touching cloud sync.
Read `docs/servers.md` before touching the server list.
Read `docs/join.md` before touching the join button plugin.
Read `docs/hugin.md` before touching the plugin that keeps the ravens away.
Read `docs/status.md` before touching the server status plugin.
Read `docs/gate.md` before touching the join codes, members or the server gate.
Read `docs/competitive.md` before touching competitive servers or `tiers.toml`.
Read `docs/reports.md` before touching the connection reports.

## Never release unverified

A release tag ships to every player through the self updater, so a release is
only cut after the change was seen working for real, never on tests alone.

- Run `make check`, then build the app and start the game from that build.
- Start the local app only with `make run`. It builds and runs with the
  Sentry setup from Infisical, like a shipped build. Never start
  `target/release/blackforge-gui` or `cargo run` by hand.
- Start it once and leave the rest to the user. Never wait on it in a loop
  or poll its output.
- Check every changed behavior in the running app and the running game, the
  way a player uses it. For a plugin change, read `BepInEx/LogOutput.log` of
  the profile for its load line, errors and warnings, and look at the menu.
- A plugin compiles against the game but only runs inside it. A green compile
  and green unit tests prove nothing about the plugin.
- Anything that cannot be checked on this machine is named to the user before
  the tag, with what is unverified. The user decides, never tag past it.
- "deploy all" or "release" is not permission to skip this.
