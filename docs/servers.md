# Servers

A registered game server is a name and the mods a player needs to join it, with
exact versions. A player opens the Servers page, sees every server, and one
button installs what their profile lacks. Nobody types mod names by hand.

Players join a server without a password. Blackforge lets them in, the owner
keeps the list of who may join, see `gate.md`. A server
of the join admin can also carry a join address. The app then gives it a join
button in the Valheim menu, see `join.md`. Any other server carries only the
mods.

## The parts

- `crates/blackforge-api/src/servers.rs` holds the wire types and the one check
  both sides run, `validate`. A name is trimmed and at most 40 characters, a
  server has 1 to 200 mods, every mod has an id and a version. A join address
  is an ipv4 address and a port, `normalize_address` checks it.
- `backend/server/src/servers.rs` is the routes. Migration `0005` adds the
  `servers` table, one row per server, `mods` stored as the JSON the app sent.
  The pair owner, game and name is unique. Migration `0007` adds `address`.
- `crates/blackforge-core/src/servers.rs` is the client side with no window in
  it. `fetch` reads the public list over the plain HTTP client. `needs` compares
  a profile with a server, `Forge::install_server` applies the list.
- `crates/blackforge/src/ui/servers_page.rs` is the page, `server_modal.rs` the
  form that registers or edits a server. `members_modal.rs` is the member list
  of the admin.

## Routes

- `GET /api/servers` is the whole list, sorted by name. It is public, no login,
  so a player can install the mods of a server before they ever sign in. Each
  row has the id, the name, the game, the owner's username, the mods, the
  time of the last save, the time of the registration and the join address
  when there is one.
- `POST /api/servers` registers one. Only the admin, see below.
- `PUT /api/servers/{id}` and `DELETE /api/servers/{id}` change or remove an
  own server of the admin. A server of somebody else gets the same 404 as one that does not
  exist.

The body of a save is `SaveServer`, the name, the game label, the mods and the
join address. No address field keeps the stored one, so an app from before the
field does not wipe it. An empty address removes it. The answer of a save is the
stored row.

## Rules of the feature

- The list is public. The privacy page says so, keep the two in step.
- Install changes only what the server needs. A mod the player lacks is added
  at the server's version and pinned for the server, a mod at another version
  is moved to the server's version and pinned, a disabled one is switched on.
  Every other mod of the profile stays. It is `Forge::add_for_server` per mod
  and one `sync` at the end. A pin carries the server name, the Mods page shows
  `pinned for Durka`.
- A server name is unique per game, `UNIQUE (game, name)` from migration
  `0006`, since a pin finds its server by name. For the same reason a server
  keeps the name it was registered with, the update route refuses a new name
  and the form hides the field when editing.
- The versions of a registered server are the ones in the owner's lock at the
  time of the save. Editing a server after a mod update moves the server to the
  new versions, a player then sees the install button again.
- A mod the server lists that left the owner's profile stays on the server
  until the owner switches it off in the form.
- Only `JOIN_ADMIN` in `crates/blackforge-api/src/servers.rs` registers,
  changes and removes servers, and only that account gives one a join address.
  Every player gets a join button for such a server, so a stranger must not add
  one. Any other account gets `only the admin registers servers` and the like.
- The window marks a row as mine by comparing the owner's username with
  `GET /api/me`. The Register and Members buttons show only to the admin, and
  only after that lookup.
- The window has no profiles, so a server is registered from the `default`
  profile. The CLI has no server commands.

## Tests

`cargo test -p blackforge-api servers` covers the checks, the address and the
wire shape.
`cargo test -p blackforge-core servers` covers the compare of a profile with a
server. The backend test checks that the writes refuse an anonymous request.
