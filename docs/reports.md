# Connection reports

Every failed join and every dropped connection in the game goes to the
backend with what the game saw and what the app saw. The server log only
shows that a PlayFab connection died, the reason is on the side of the
player. So the player's game and app send it.

## The path

1. `assets/join/Report.cs` in the join plugin keeps a trail from the click on a
   join button: the answers of the app, the lobby found, the join code, the
   character, the socket opening, every change of `ZNet.ConnectionStatus`, the
   first frame in the world and each disconnect. Each line has the UTC time
   and the seconds since the click.
2. `assets/join/Relay.cs` logs the network layer, which the game logs little
   of. It changes nothing in the game.
   - Every PlayFab Party state change with all its values, the reason and the
     error code with its text. Party logs these itself only at a verbose level
     that is off. The per message and the voice chat changes are left out.
   - Every close of the PlayFab socket with the stack of who called it, the
     messages in flight, the next id it waits for and the time since the last
     message in. Some closes in the game and in mods log nothing else.
   - Every ack outside the messages in flight. The game then closes the
     socket without a line.
   - The last 200 messages in and out of the socket, type, id and size, kept
     raw in a ring and formatted only for a report.
3. A failure sends one report. The triggers are a connect error in the menu,
   also the one after a drop in the world, `NotOnline` when no lobby is found,
   and `Refused<code>` when the app refused the join. A connect error is the
   status name, like `ErrorConnectFailed`. The same status and text within 5
   seconds is sent once.
4. The report carries the character, the local PlayFab id that the game
   server logs as `playfab/<id>`, the Steam id, the game and plugin versions,
   the system, every loaded plugin with its version, the trail, the socket
   ring, the newest 320 KB of `BepInEx/LogOutput.log` and 160 KB of the Unity
   `Player.log`. The plugin posts it to the app at
   `http://127.0.0.1:<port>/report`. That door skips the key check, a game with
   a stale key is a failure worth a report too.
5. `crates/blackforge/src/bridge.rs` adds the app version and the newest 128 KB
   of the app log, then sends it in the background to `POST /api/reports`.
6. `backend/server/src/reports.rs` stores it in `connection_reports`,
   migrations `0010` and `0011`, with the account id of the login, and writes
   one `connection report from <username> <account id>, character ...` line
   to the backend log.

The wire type is `ConnectionReport` in `crates/blackforge-api/src/report.rs`.
The plugin writes it by hand, so every field has a default.

## Nothing is lost offline

- A report the app did not take waits in `BepInEx/blackforge-reports` of the
  profile. It goes out after the next report that gets through, and at every
  start of the game from the app. The newest 50 wait.
- A report the server did not take, also while nobody is signed in, waits in
  `reports` of the data folder. It goes out after the next report that gets
  through. The newest 50 wait.

## The app log

The app log, which every report carries, has a line for every request of the
game at the local door with its status and time, `the game asked ...`, and a
line for every start of the game with the program, the Steam handoff and the
profile, `the game starts: ...`. The start arguments are not logged, they hold
the key of the door.

## The gate log

The backend names the player and the account id at every rules request, join
code and refusal of a stranger, and the server name and username at every
trade of a code. So a report lines up with the gate by account and time.

## Limits

- A report is deleted after 30 days, and one player keeps at most 500. The
  privacy page says what a report holds, keep the two in step.
- The game server's own view of a join is only in its docker log, see
  `status.md`. A redeploy of the game stack drops that log.
- A game that crashes or is quit during a connect sends nothing. A failed
  PlayFab login sends nothing.

If a patch of the reports fails at the start of the game, the plugin logs
`connection reports are off` and the join buttons still work.

## Reading them

Only the admin reads them. `GET /api/reports` lists the newest 200 without the
logs, with the account id and the character. `GET /api/reports/{id}` gives one
whole report.

Straight from the database, on the node of the `blackforge` deployment:

```sh
docker exec blackforge-db-1 psql -U blackforge -c "SELECT id, created_at, user_id, character, server, status, message, in_world, round(seconds) FROM connection_reports ORDER BY id DESC LIMIT 20"
docker exec blackforge-db-1 psql -U blackforge -At -c "SELECT jsonb_array_elements_text(report->'trail') FROM connection_reports WHERE id = 1"
docker exec blackforge-db-1 psql -U blackforge -At -c "SELECT jsonb_array_elements_text(report->'traffic') FROM connection_reports WHERE id = 1"
docker exec blackforge-db-1 psql -U blackforge -At -c "SELECT report->>'log' FROM connection_reports WHERE id = 1"
docker exec blackforge-db-1 psql -U blackforge -At -c "SELECT report->>'app_log' FROM connection_reports WHERE id = 1"
```

The quick look is the backend log:

```sh
docker logs blackforge-server-1 2>&1 | grep "connection report"
```

## Tests

`cargo test -p blackforge-api report` checks the shape the plugin writes and
the trim. `cargo test -p blackforge-server reports` checks that the routes
refuse a request without a login.
