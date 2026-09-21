# Friends

Sign in with Google, friends, a friend's mods and their changed settings. Signing in is
optional, the app works the same without an account. What users are told about their
data is in `web/privacy.html`, keep the two in step.

## The parts

- `backend/server` is the server. It is built on `hilen-server` and also serves the
  landing page, there is no nginx anymore.
- `crates/blackforge-api` holds every request and answer type and the username rule. The
  server and the app both build against it, so the two sides cannot drift apart. A change
  of a type there is a change of the wire, old released apps still send the old shape.
- `crates/blackforge-core/src/social` is the client side with no window in it.
  - `client.rs` is the calls to the server.
  - `secret.rs` decides which settings never leave the computer.
  - `shared.rs` builds what a profile shows to friends.
  - `picker.rs` is the merge behind the "copy config" dialog.
- `crates/blackforge/src/social.rs` is the glue in the window: who is signed in, the
  uploads, the in game reports.
- `crates/blackforge/src/ui` has `friends_page.rs`, `friend_mods_page.rs` and
  `config_picker.rs`.

The login itself is a part of the hilen engine, the `login` feature on the app side and
`hilen_server::auth` on the server side. Read `docs/login.md` in the hilen repo and the
`google-login.md` chapter of the hilen skill.

## Routes

From the engine: `/auth/google`, `/auth/google/callback`, `/auth/poll`, `/auth/me`,
`/auth/logout`, plus `/api/health` and `/api/hello`.

From `backend/server/src/routes.rs`, every one wants `Authorization: Bearer <session>`:

- `GET /api/me` is my username, `null` before the first pick.
- `POST /api/me/username` sets it, once. It cannot be changed later.
- `GET /api/friends` is the friends with their in game mark, and the requests in both
  directions.
- `POST /api/friends/request`, `/accept`, `/decline`, `/remove` take a username. Two
  people who ask each other become friends at once. Remove ends a friendship for both
  sides, and it also takes back a request of my own.
- `PUT /api/profile` uploads my mods and changed settings.
- `GET /api/friends/{username}/profile` reads a friend's. Anybody who is not an accepted
  friend gets the same 404 as for a name that does not exist.
- `POST /api/status` is the in game report.

From `backend/server/src/site.rs`, the jobs the old nginx config did: the landing page
from the embedded `web/dist`, `manifest.json`, `updater.json` and the download click
reports passed on to studio, and a redirect of everything else under `/download/` to
studio. Installed apps have this host in their updater address.

A friend sees a username, the mods, the shared settings and the in game mark. Never the
email, the Google name or the picture. The privacy page promises that.

## Rules of the feature

- A username is Latin letters, digits and the underscore, 3 to 20 characters, stored in
  lower case. The rule lives in `blackforge_api::username`.
- The app uploads the profile after every change of it, at launch, and after the game
  exits. `backend::change` is the one hook for the changes. An upload equal to the last
  one is skipped, the last one is kept in `gui-last-shared-profile.json`.
- Only settings that differ from their `# Default value:` are shared. A setting that
  looks like a secret stays local: a key with `password`, `token`, `secret`, `webhook` or
  `apikey`, or a value that is a web address.
- In game means the app started the game. It reports at the start, once a minute, and at
  the exit. The server reads 2 minutes of silence as not in game.
- A friend's mod is added the way Browse adds one, the newest version with what it
  needs. There is no install of a whole list.
- The picker shows one row per setting that differs. The friend's value starts picked
  where the friend changed it, mine where only I did. Only rows set to the friend's side
  are written, so a plain apply never loses anything of mine.
- The Friends page asks the server again every 30 seconds while it is open.

## Database

Postgres 17, the `db` service of `docker-compose.yml`, data in `./data/postgres` on the
node. The engine migrations run first and make `users`, `sessions` and
`pending_logins`. `backend/server/migrations` makes `profiles`, `friend_requests`,
`friendships`, `shared_profiles` and `game_status`. A friendship is one row per pair with
the smaller user id first. A shared profile is stored as the JSON the app sent.

## Secrets

All in the prod env of the Infisical project `Blackforge`,
`a2066daf-13f4-4831-9d06-8ee963560c03`. The machine identities `beekeeper` and
`gebling-ci` both have Viewer access to it.

The server gets these from beekeeper at deploy time, through `docker-compose.yml`:

- `GOOGLE_CLIENT_ID` and `GOOGLE_CLIENT_SECRET`, the Google client.
- `DB_PASSWORD`, the Postgres password.

The release build of the app gets these through `build/release/with-secrets.sh`:

- `HILEN_SESSION_KEY`, the built in half of the key that seals the stored session. A
  release without it fails on purpose. Changing it signs every user out.
- `BLACKFORGE_SENTRY_URL` and `BLACKFORGE_UPDATE_KEY`, older than this feature.

The Google client lives in the Cloud project `blackforge-509310` under
`vladaszakrevskis@gmail.com`, type web, redirect
`https://blackforge.vladas.xyz/auth/google/callback`. The app is published, any Google
user can sign in. Google wanted the privacy policy link before it allowed that.

## Deploy and tests

A push to `main` is the deploy, beekeeper builds `backend/Dockerfile` and starts the
stack. The image builds the landing page first and then the server with the page
embedded in it.

The server has unit tests for its pure parts only. It was never run against a test
Postgres, that was a decision, the real check is production. The core and the api crate
have normal unit tests, `cargo test -p blackforge-core social` and
`cargo test -p blackforge-api`.

To look at the pages without touching the real profile, start a debug build with
`BLACKFORGE_HOME` set to an empty folder.
