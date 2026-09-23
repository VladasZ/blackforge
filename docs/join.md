# Join button

The Valheim main menu gets a big "Join Durka" button in the top left corner. A
click takes the player to character select. After Start the game joins the Durka
server and asks for the server password. The player types the password, the mod
never knows it.

## The parts

- `assets/join/Plugin.cs` is a small `BepInEx` plugin. A Harmony postfix on
  `FejdStartup.SetupGui` copies the framed Start button of character select into
  the main menu, anchors it to the top left and gives it the label. Its gamepad
  shortcut is removed, so the A button in the menu does not trigger a join.
- `assets/join/BlackforgeJoin.dll` is the built plugin. It is committed, the app
  embeds it.
- `crates/blackforge-core/src/join.rs` writes the dll to
  `BepInEx/plugins/blackforge-join` before every start of the Valheim client.
  There is no setting, every Valheim client started from the app gets it. A
  server gets nothing.

The folder is not `BepInEx/plugins/blackforge` on purpose. That one belongs to
the achievements plugin, and it is removed when that setting is off.

## How the join works

The click calls `ZSteamMatchmaking.instance.QueueServerJoin("86.100.76.6:2456")`.
That is the same queue a Steam invite or the `+connect` launch argument fills.
The game picks the queue up on its own, shows the EULA if needed, then character
select. After Start, `FejdStartup.JoinServer` looks for a PlayFab lobby with that
address and joins through the PlayFab relay.

This works because the server runs with crossplay on. At every start it
registers a PlayFab lobby with its public address, `86.100.76.6:2456`. The ports
on pc1 stay closed, the relay needs none. The join code of the lobby changes at
every server restart, so the button does not use it.

The address is fixed in the plugin. The home IP does not change. If it ever does,
change `Address` in `Plugin.cs`, rebuild the dll and ship a release.

If the player is not logged in to PlayFab when the join starts, the game tries a
direct Steam connection instead. That fails, since the ports are closed.

## Building the dll

The plugin compiles against the game dlls, with
[BepInEx.AssemblyPublicizer](https://github.com/BepInEx/BepInEx.AssemblyPublicizer)
so it can call private game members. So the build needs the `valheim_Data/Managed`
folder of an installed Valheim. The game dlls are never committed.

```sh
make join-plugin VALHEIM_MANAGED=/path/to/Valheim/valheim_Data/Managed
```

The target runs the dotnet SDK in Docker and needs Linux containers. Docker for
Windows in Windows container mode cannot run it. Then copy the Managed folder to
a Linux box and build there. A local `dotnet build` in `assets/join` works too,
the csproj takes the folder from the `ValheimManaged` property.

Build the app again after a new dll, the Rust side embeds it at compile time.

## Tests

`cargo test -p blackforge-core join` checks that the dll is written and that
only the Valheim client gets it. The button itself was checked by hand in the
running game.
