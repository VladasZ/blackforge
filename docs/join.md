# Join buttons

The Valheim main menu gets a big join button per server in the top left corner,
like "Join Durka" and "Join Arkham Asylum". A click takes the player to character
select. After Start the game joins that server. There is no password. A click
gets the rules of the server from the running app, Start gets a one time code,
and the server lets in only a code from blackforge, see `gate.md`.

## The parts

- `assets/join/Plugin.cs` is a small `BepInEx` plugin. A Harmony postfix on
  `FejdStartup.SetupGui` copies the framed Start button of character select into
  the main menu once per server, stacks the copies in the top left and gives
  each its label. Their gamepad shortcut is removed, so the A button in the menu
  does not trigger a join.
- `assets/join/BlackforgeJoin.dll` is the built plugin. It is committed, the app
  embeds it.
- `crates/blackforge-core/src/join.rs` writes the dll and `servers.json` to
  `BepInEx/plugins/blackforge-join` before every start of the Valheim client.
  There is no setting, every Valheim client started from the app gets it. A
  server gets nothing.
- `assets/join/Competitive.cs` is the client half of competitive servers, the
  check in character select and the world tags, see `competitive.md`.

The folder is not `BepInEx/plugins/blackforge` on purpose. That one belongs to
the achievements plugin, and it is removed when that setting is off.

## The server list

The buttons come from the registered servers, see `servers.md`. A server with a
join address gets a button. Only the account `JOIN_ADMIN` in
`crates/blackforge-api/src/servers.rs` may set an address, since every player
gets the button. The app fetches the list before every start, waits at most 5
seconds, and writes it as `servers.json`:

```json
{ "servers": [{ "id": "e3ce14b4-...", "name": "Durka", "address": "86.100.76.6:2456" }] }
```

The plugin reads it with the Newtonsoft library the game ships. Unity's
`JsonUtility` gave back an empty list in the plugin without any error, so never
go back to it. A failed fetch keeps the file of the last start. No file, or no
server in it, means no button.

The buttons go oldest registration first, so Durka stays on top. Every button
takes the width of the longest label, so a long name is not cut.

So a new server needs no release. Register it on the Servers page and give it
the address in the form, the next start of the game shows its button.

## How the join works

Every server runs with crossplay on. At every start it registers a PlayFab lobby
that stores its public address as `ip:port` and its `SERVER_NAME`. With
crossplay the players go through the PlayFab relay, so no port on pc1 is open
and the port in the address is only a label. All servers use the same address,
`86.100.76.6:2456`.

A click first gets the rules of the server from the app, see `gate.md`. Then it searches
the lobbies by the address, the server name, and the active
flag, the keys are `ServerIpSearchKey`, `ServerNameSearchKey` and
`IsActiveSearchKey` of `ZPlayFabMatchmaking`. The stock join by address alone
cannot be used, with several lobbies on one address it joins the newest one.
The found lobby names its host player. The plugin puts a join to that host into
`ZSteamMatchmaking.m_joinData`, the same queue a Steam invite fills. The game
picks the queue up on its own, shows the EULA if needed, then character select,
and joins through the relay after Start.

If the player is not logged in to PlayFab yet, the game's own popup waits for
the login and then runs the join. If no lobby is found, the menu shows
"<name> is not online".

The name in the list must match `SERVER_NAME` of the server exactly. A rename on
the server side without the same name on the Servers page leaves a button that
says the server is not online.

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
a Linux box and build there. The plugin takes `assets/shared/Tiers.cs` too, the target copies it. A local
`dotnet build` in `assets/join` works too,
the csproj takes the folder from the `ValheimManaged` property.

Build the app again after a new dll, the Rust side embeds it at compile time.

## Tests

`cargo test -p blackforge-core join` checks that the dll is written, that only
the Valheim client gets it, and which servers land in `servers.json`. The
buttons themselves are checked by hand in the running game.
