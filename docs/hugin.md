# No ravens

Hugin and Munin, the tutorial ravens, never show up in a game started from
blackforge. The texts they would say still go into the compendium.

## The parts

- `assets/hugin/Plugin.cs` is a small `BepInEx` plugin. Both birds appear only
  through `Raven.Spawn`. A Harmony prefix skips that call and does what a talk
  with the bird does: the tutorial counts as seen, and its text goes into the
  compendium with `Player.AddKnownText`.
- `assets/hugin/BlackforgeHugin.dll` is the built plugin. It is committed, the
  app embeds it.
- `crates/blackforge-core/src/hugin.rs` writes the dll to
  `BepInEx/plugins/blackforge-hugin` before every start of the Valheim client.
  There is no setting. It goes in together with the join plugin, both hang on
  the `client_plugins` flag of the launch plan. A server gets nothing.

The game's own Tutorials switch in the settings only skips the bird, the text
is lost. That is why the plugin marks the text seen and adds it by hand. A
seen tutorial also leaves the queue, else the game would try the spawn every
second.

## Building the dll

It compiles against the game dlls, like the join plugin, see `join.md`.

```sh
make hugin-plugin VALHEIM_MANAGED=/path/to/Valheim/valheim_Data/Managed
```

On a Mac the folder is
`~/Library/Application Support/Steam/steamapps/common/Valheim/valheim.app/Contents/Resources/Data/Managed`.

Build the app again after a new dll, the Rust side embeds it at compile time.

## Tests

`cargo test -p blackforge-core hugin` checks that the dll is written. The
birds themselves are checked by hand: a new character in a new world sees no
raven at the spawn stones, and the compendium still gets the first entries.
`BepInEx/LogOutput.log` of the profile has the line `the ravens stay away`.
