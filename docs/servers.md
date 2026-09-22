# Servers

A registered game server is a name and the mods a player needs to join it, with
exact versions. A player opens the Servers page, sees every server, and one
button installs what their profile lacks. Nobody types mod names by hand.

The address and the password of a server are not part of this on purpose.
Players join through the game as before, the list only carries the mods.

## The parts

- `crates/blackforge-api/src/servers.rs` holds the wire types and the one check
  both sides run, `validate`. A name is trimmed and at most 40 characters, a
  server has 1 to 200 mods, every mod has an id and a version.
- `backend/server/src/servers.rs` is the routes. Migration `0005` adds the
  `servers` table, one row per server, `mods` stored as the JSON the app sent.
  The pair owner, game and name is unique.
- `crates/blackforge-core/src/servers.rs` is the client side with no window in
  it. `fetch` reads the public list over the plain HTTP client. `needs` compares
  a profile with a server, `Forge::install_server` applies the list.
- `crates/blackforge/src/ui/servers_page.rs` is the page, `server_modal.rs` the
  form that registers or edits a server.

## Routes

- `GET /api/servers` is the whole list, sorted by name. It is public, no login,
  so a player can install the mods of a server before they ever sign in. Each
  row has the id, the name, the game, the owner's username, the mods and the
  time of the last save.
- `POST /api/servers` registers one. Wants a login and a username.
- `PUT /api/servers/{id}` and `DELETE /api/servers/{id}` change or remove an
  own server. A server of somebody else gets the same 404 as one that does not
  exist.

The body of a save is `SaveServer`, the name, the game label and the mods. The
answer of a save is the stored row.

## Rules of the feature

- The list is public. The privacy page says so, keep the two in step.
- Install changes only what the server needs. A mod the player lacks is added
  at the server's version and pinned, a mod at another version is moved to the
  server's version and pinned, a disabled one is switched on. Every other mod
  of the profile stays. It is `Forge::add` per mod and one `sync` at the end,
  so the install goes through the same path as the Browse page.
- The versions of a registered server are the ones in the owner's lock at the
  time of the save. Editing a server after a mod update moves the server to the
  new versions, a player then sees the install button again.
- A mod the server lists that left the owner's profile stays on the server
  until the owner switches it off in the form.
- The window marks a row as mine by comparing the owner's username with
  `GET /api/me`. Signed out, no row is mine and the register button explains
  that registering needs an account.
- The window has no profiles, so a server is registered from the `default`
  profile. The CLI has no server commands.

## Tests

`cargo test -p blackforge-api servers` covers the checks and the wire shape.
`cargo test -p blackforge-core servers` covers the compare of a profile with a
server. The backend test checks that the writes refuse an anonymous request.
