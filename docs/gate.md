# Gate

Blackforge is the only way onto the game servers, Durka and Arkham Asylum.
There is no password for players. The owner of a server keeps a list of who may
join, and every join is checked live with the backend.

## One join

1. The player clicks a join button in the Valheim menu, see `join.md`.
2. The join plugin asks the running Blackforge app for the rules of the
   server, over `http://127.0.0.1:<port>/rules/<server id>`. It sends the key
   the app gave the game at start. The app is signed in with Google and asks
   `POST /api/servers/{id}/rules`. That checks the membership and gives the
   rules of a competitive server, see `competitive.md`. It makes no code. A
   stranger hears here that they are no member.
3. Character select opens. At Start the plugin asks the app for a one time
   code at `/join/<server id>`, the app asks `POST /api/servers/{id}/join`.
   The code is made only now, so the time a player spends in character select
   never eats into its 2 minutes. A failed request keeps the player in
   character select and shows the reason.
4. The plugin puts the code into the invite key of the game with
   `ZNet.SetInviteSecretKey`. The game sends that key to the server in its
   handshake. No vanilla code sets that key, so it is free to use.
5. The server plugin, see `status.md`, trades the code at
   `POST /api/gate/verify` for the username behind it. A good code lets the
   player skip the password, a bad or missing one gets refused.

The server plugin sends its reason for a refusal in the `BlackforgeGate` rpc
before the game's own error. The join plugin shows that text in place of the
game's error text.

A join right after a drop fails in a way the other mods explain badly. The
server keeps a dropped connection for 90 seconds in case the player comes
back, and a new join inside that time hooks onto the dead connection and
starts the 90 seconds again. The server's answers go down the dead
connection, so Conditional Config Sync on the client says "No version
handshake was received from the server" in tiny text. For a join through a
button the plugin puts one short line in place of it, that the server still
holds the last connection and to wait 2 minutes.

Every failed join and every drop also sends a report with the game logs to
the backend, see `reports.md`.

## The local connection

`crates/blackforge/src/bridge.rs` listens on 127.0.0.1 only, on a free port
picked at the first start of the game. Every start of the Valheim client gets a
new random key. The launch passes both as `-blackforge-bridge <port>
-blackforge-key <key>`, see `bridge_args` in `crates/blackforge-core/src/join.rs`.
A request needs the key in `X-Blackforge-Key`, so no other program on the
machine gets a code. The key stops working at the next start of the game.

On Windows the start is `steam.exe -applaunch`, which hands the game to the
running Steam and exits at once. The app then waits on the game itself, found
by its program name in `crates/blackforge/src/game_process.rs`, and the
launch plan names it in `handed_to`. Until the game exits the run button
stays off, so a second start cannot give a new key to a game that still holds
the old one. A game that does not show up within 2 minutes counts as not
started.

A wrong key gets a refusal that says why. A key from an older start means the
app started Valheim again after this game opened. No key at all in the app
means the app was restarted after the game opened. Both tell the player to
close Valheim and press Play in Blackforge.

The app must stay open while the game runs. A game started without the app has
no key, and its join buttons say so.

## The backend

`backend/server/src/gate.rs` holds the routes, migration `0008_gate.sql` the
tables.

- `GET /api/members`, `POST /api/members` and `DELETE /api/members/{username}`
  are the member list of the admin, no other account may use them. One list
  covers every server of the admin. The admin is always let in and is never on
  the list.
- `POST /api/servers/{id}/rules` gives the owner of the server or a member of
  the owner's list the rules of the server, `JoinRules`. It makes no code.
- `POST /api/servers/{id}/join` gives such a player a code, `JoinCode`. A code
  is 2 random uuids. Only its sha256 is stored, it lives `CODE_SECONDS`, 2
  minutes. The trade refuses an expired or used code like a stranger, so the
  refusal text of the server plugin names both causes.
- `POST /api/gate/verify` wants `Authorization: Bearer <BLACKFORGE_GATE_SECRET>`.
  It deletes the code on its first try, then checks that it has not expired,
  that it is for the server of that name, and that the player is still a
  member. The answer is the username. Without the secret in the environment
  the route refuses everything.

Removing a member also deletes that member's open codes, so the next join is
refused.

## The Members window

`crates/blackforge/src/ui/members_modal.rs`, the Members button of the Servers
page, shown only to the admin. Its field searches people by the start
of their username, the same `GET /api/users/search` the Friends page uses. While
the field holds a search the table shows the people found, each with an Add
button, or "Already a member". An empty field shows the members, each with a
Remove button. A member row carries the Google picture, a backend before the
`picture` field of `Member` sends none and the row shows the first letter.

## The secrets

`BLACKFORGE_GATE_SECRET` is in the Blackforge Infisical project for the backend,
and the same value is in the Valheim project for the game servers. The Valheim
project also holds `SERVER_PASS`, a random password nobody is given. It keeps
the server locked if the gate plugin is not loaded. The game stacks use their
own project because every secret of a project lands in the `.env` of a stack.

## Rules

- A server without the gate settings or with a failed patch stops, see
  `status.md`. It must never run with the gate off.
- A backend that cannot be reached means new joins are refused. Players already
  on stay.
- Only a player's first peer info after a good code skips the password. The
  ban list and the version check still run.

## Tests

`cargo test -p blackforge-server gate` checks that the routes refuse a request
without a login or the secret. `cargo test -p blackforge-core join` checks the
start arguments. The join itself is checked by hand, on a throwaway server
first, see `status.md`.
