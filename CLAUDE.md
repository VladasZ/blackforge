Read `docs/data.md` before looking at the user's data folder or profiles.
Read `docs/sync.md` before touching cloud sync.
Read `docs/servers.md` before touching the server list.
Read `docs/join.md` before touching the join button plugin.
Read `docs/hugin.md` before touching the plugin that keeps the ravens away.
Read `docs/quit.md` before touching the plugin that keeps Cmd+Q from closing the game.
Read `docs/emoji.md` before touching the chat emojis plugin.
Read `docs/rail.md` before touching the rail plugin or its models.
Read `docs/status.md` before touching the server status plugin.
Read `docs/gate.md` before touching the join codes, members or the server gate.
Read `docs/competitive.md` before touching competitive servers or `tiers.toml`.
Read `docs/reports.md` before touching the connection reports.
Read `docs/required.md` before touching the required mods or `required.toml`.

## Never release unverified

A release tag ships to every player through the self updater, so a release is
only cut after the change was seen working for real, never on tests alone.

- Run `make check`, then build the app and start the game from that build.
- Start the local app only with `make run`. It builds and runs with the
  Sentry setup from Infisical, like a shipped build. Never start
  `target/release/blackforge-gui` or `cargo run` by hand.
- The user starts `make run` and the game. Never start it yourself unless
  asked, and never wait on it in a loop or poll its output.
- While the user runs the app, watch its logs live and report every error
  and warning at once, without being asked. Watch both:
  - the app log, the newest `blackforge-gui-*.log` in
    `~/Library/Logs/blackforge-gui`, a new file per start of the app,
  - the game log, `BepInEx/LogOutput.log` in the profile, rewritten at
    every start of the game.
  Use a monitor that follows the file, like `tail -F` through `grep` for
  `ERROR`, `WARN`, `Error`, `Warning`, `Exception` and `Could not load`,
  and the load lines of the changed plugin. Rearm it when it expires.
  `DllNotFoundException: AppleCoreNativeMac` comes from the game's own Game
  Center plugin on a Mac, not from a Blackforge plugin. It shows at every
  start and the game goes on.
- Check every changed behavior in the running app and the running game, the
  way a player uses it. For a plugin change, read `BepInEx/LogOutput.log` of
  the profile for its load line, errors and warnings, and look at the menu.
- A plugin compiles against the game but only runs inside it. A green compile
  and green unit tests prove nothing about the plugin.
- Anything that cannot be checked on this machine is named to the user before
  the tag, with what is unverified. The user decides, never tag past it.
- "deploy all" or "release" is not permission to skip this.

## A throwaway server for changes that meet a server

A change that meets the game servers is tested on a new throwaway server
before it reaches Durka or Arkham Asylum, or goes out in a release tag. The
Sailing mod shipped in v0.1.27 and went on both servers, then refused to load
next to Valheim Plus. A throwaway server would have shown it in a minute.

It meets a server when it changes any of these:

- the mods or versions in the composes of the servers,
- `required.toml` or any other list that puts a mod into every profile,
- the server status plugin, the join plugin, the gate, competitive servers,
  or anything else on the path of a join,
- the mods a player loads next to the server mods.

A change that stays in the app or in the client game, like the Cmd+Q plugin,
needs no server. The rules above are enough: seen working in the running app
and game.

The test:

- Start a new throwaway server, never reuse an old one and never test on
  Durka or Arkham Asylum. The recipe is in `docs/status.md`. Give it the exact
  mods and versions of the real servers plus the change.
- Read its BepInEx log. Every mod must have its `Loading [...]` line and no
  `Could not load` line, no errors from the change.
- Join it with the app built from the release commit, and check the change
  in the game, the way a player uses it.
- Stop and remove the throwaway server when done.
- When the change meets a server, no tag and no compose push until this
  passed. The user saying "release" or "tag now" does not skip it. Name the
  step that did not run and wait.
