# Connection reports

Every failed join and every dropped connection in the game goes to the
backend with what the game saw. The server log only shows that a PlayFab
connection died, the reason is on the side of the player. So the player's game
sends it.

## The path

1. `assets/join/Report.cs` in the join plugin keeps a trail from the click on a
   join button: the answers of the app, the lobby found, the join code, the
   socket opening, every change of `ZNet.ConnectionStatus`, the first frame in
   the world and each disconnect. Each line has the UTC time and the seconds
   since the click.
2. A failure sends one report. The triggers are a connect error in the menu,
   also the one after a drop in the world, `NotOnline` when no lobby is found,
   and `Refused<code>` when the app refused the join. A connect error is the
   status name, like `ErrorConnectFailed`. The same status and text within 5
   seconds is sent once.
3. The plugin adds the newest 320 KB of `BepInEx/LogOutput.log` and 160 KB of
   the Unity `Player.log`, then posts it to the app at
   `http://127.0.0.1:<port>/report`. That door skips the key check, a game with
   a stale key is a failure worth a report too.
4. `crates/blackforge/src/bridge.rs` adds the app version and sends it on in
   the background to `POST /api/reports`. It needs a signed in app. A report
   while nobody is signed in stays only in the app log.
5. `backend/server/src/reports.rs` stores it in `connection_reports`,
   migration `0010`, and writes one `connection report from <username>` line
   to the backend log.

The wire type is `ConnectionReport` in `crates/blackforge-api/src/report.rs`.
The plugin writes it by hand, so every field has a default. The backend keeps
the newest 512 KB of the logs and the newest 200 trail lines.

A report is deleted after 30 days, and one player keeps at most 500. The
privacy page says what a report holds, keep the two in step.

If a patch of the reports fails at the start of the game, the plugin logs
`connection reports are off` and the join buttons still work.

## Reading them

Only the admin reads them. `GET /api/reports` lists the newest 200 without the
logs, `GET /api/reports/{id}` gives one whole report.

Straight from the database, on the node of the `blackforge` deployment:

```sh
docker exec blackforge-db-1 psql -U blackforge -c "SELECT id, created_at, server, status, message, in_world, round(seconds) FROM connection_reports ORDER BY id DESC LIMIT 20"
docker exec blackforge-db-1 psql -U blackforge -At -c "SELECT report->>'log' FROM connection_reports WHERE id = 1"
```

The quick look is the backend log:

```sh
docker logs blackforge-server-1 2>&1 | grep "connection report"
```

## Tests

`cargo test -p blackforge-api report` checks the shape the plugin writes and
the trim. `cargo test -p blackforge-server reports` checks that the routes
refuse a request without a login.
