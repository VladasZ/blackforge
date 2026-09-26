# No Cmd+Q

On macOS, Cmd+Q never closes a game started from blackforge. It is too easy to
hit by accident in the middle of a fight. The Quit button of the game and
Quit in the Dock menu still close it.

## The parts

- `assets/quit/Plugin.cs` is a small `BepInEx` plugin. Cmd+Q is only the
  key of the Quit item in the app menu. In its first frames the plugin finds
  that item through the Objective-C runtime and clears its key, so macOS
  never starts a quit. The log says `Cmd+Q stays in the game`, or why it could
  not.
- Never refuse the quit in `Application.wantsToQuit` instead. Unity on macOS
  still sends `OnApplicationQuit`, Valheim stops its terrain builder, and the
  game stays open half dead with a `NullReferenceException` in
  `FejdStartup.OnApplicationQuit` on every press.
- `assets/quit/BlackforgeQuit.dll` is the built plugin. It is committed, the
  app embeds it.
- `crates/blackforge-core/src/quit.rs` writes the dll to
  `BepInEx/plugins/blackforge-quit` before every start of the Valheim client on
  macOS. The flag is `quit` in `ClientPlugins` of the launch plan. Windows and Linux never
  get it, and a server gets nothing. There is no setting.

## Building the dll

It compiles against the Unity dlls of the game, like the ravens plugin, see
`hugin.md`.

```sh
make quit-plugin VALHEIM_MANAGED=/path/to/Valheim/valheim_Data/Managed
```

Build the app again after a new dll, the Rust side embeds it at compile time.

## Tests

`cargo test -p blackforge-core quit` checks that the dll is written and that
only a Mac gets it. The key itself is checked by hand on a Mac: Cmd+Q in the
menu and in a world does nothing, the Quit button still closes the game.
`BepInEx/LogOutput.log` of the profile has the line `Cmd+Q stays in the game`
and no `Stopping build thread` after a press.
