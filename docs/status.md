# Server plugin

A small `BepInEx` plugin that runs inside the dedicated Valheim servers, Durka
and Arkham Asylum. It is the gate of the server, see `gate.md`, and it writes who
is on. Every 2 seconds it writes the peers the game has connected
right now to a JSON file. It is the fast and exact answer to "is anybody on the
server", which decides if a server may restart.

The game itself has no such answer. Its `Connections N` line comes only every
10 minutes, and its join and leave lines disagree with each other. The Steam
server query on the query port gets no answer with crossplay on.

## The file

```json
{ "updated": 1790189793, "players": [{ "name": "Motvaizer", "id": "123", "user": "motvaizer" }] }
```

`updated` is unix seconds. Every connected peer is listed, also one that still
picks a character. `user` is the blackforge username the gate let in, a peer
still in the handshake has none. The file is replaced in one step, so a reader
never sees half of it.

A file older than about 15 seconds means the server is not running, is still
starting, or the plugin is not loaded. Treat that as unknown, never as empty.

On the node the file sits at `~/deployments/<name>/data/svlog/blackforge-status.json`.
The server writes it to `/var/log/supervisor`, the folder the metrics sidecar
mounts as `/logs`, and the path comes from `BLACKFORGE_STATUS_FILE`. Without
that variable the plugin writes nothing.

## The gate

The plugin patches `ZNet.RPC_ServerHandshake` and `ZNet.RPC_PeerInfo`. It asks
the backend at `BLACKFORGE_GATE_URL` about every join, with
`BLACKFORGE_GATE_SECRET`. Both come from the compose, the secret from the
Valheim Infisical project. The server still runs with `SERVER_PASS`, a random
password nobody is given, so a server without the plugin lets nobody in.

The plugin stops the server when either gate variable is missing or a patch
fails, for example after a game update renamed a method. `BepInEx/LogOutput.log`
then names the reason. A gate that is off must never run.
## The parts

- `assets/status/Plugin.cs` is the plugin, `assets/status/BlackforgeStatus.dll`
  the built one. It is committed because the servers download it from here.
- The composes in `games/valheim` and `games/valheim-hard` of the `local` repo
  set `BLACKFORGE_STATUS_URL`, `BLACKFORGE_STATUS_SHA256` and the gate
  variables. The install hook
  downloads the dll from that url, a raw GitHub link pinned to a commit, checks
  the sha256 and installs it next to the Thunderstore mods. A failed download
  keeps the copy that is there.
- The metrics sidecar reads the file. While it is fresh, `valheim_players` and
  `valheim_player_online` come from it, else from the log lines as before.
  `valheim_status_age_seconds` is its age, -1 when it is missing.

The plugin is never installed on a game client. The app installs only the join
and achievements plugins there.

## Changing the plugin

Build it like the join plugin, it compiles against the game dlls and takes
`assets/shared/Tiers.cs` too:

```sh
make status-plugin VALHEIM_MANAGED=/path/to/Valheim/valheim_Data/Managed
```

A local `dotnet build -c Release` in `assets/status` works too.

The plugin also runs the server half of competitive servers, it reports the
dead bosses to the backend and checks what every player carries, see
`competitive.md`.

Commit the new dll, then point both composes at the new commit and the new
sha256. Pushing the composes restarts both servers, so first check that nobody
is on them, see Rule 1 of the beekeeper skill.

Test a change on a throwaway server first, never on Durka. A plain
`docker run` of `ghcr.io/lloesche/valheim-server` on a free node, with
`BEPINEX=true`, the dll in `/config/bepinex/plugins/blackforge-status/`,
`BLACKFORGE_STATUS_FILE` and the two gate variables set, shows the file within a
minute of the world load. Its server name must be one registered on the Servers
page, a code is for one server only.
