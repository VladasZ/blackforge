# Railroads

Players build stone track, stations, a coal locomotive and wagons with the
hammer. A player clicks a locomotive, picks a station, and the train drives
there and waits. A player can also sit on the locomotive and drive it by hand,
like in Factorio. Trains keep moving when no player is near. Wagons are loaded
and unloaded by hand, like a vanilla cart.

Everything is written. The logic has not run on a server yet, it is tested on
the test server `Test` first, see `status.md`.

## Playing

- **Track.** 18 pieces in the Transport tab of the build menu: straights of 1,
  2 and 4 m, curves of 8 m at 22.5, 45 and 90 degrees, of 16 m at 22.5 and 45
  and of 32 m at 22.5, a gentle and a steep slope, a left and a right switch
  and a crossing. A slope bottom and a slope top of 4 m for each slope bend
  the track evenly between flat and slope, so a hill is flat, bottom, slopes,
  top, flat, and the same pieces turned round go down. Every end is a snap point. A curve turns right, placed the
  other way round it turns left. Every curve ends on a 22.5 degree step, the
  rotation step of the hammer.
- **Support.** Track and stations need support like a wood floor. A piece on
  the ground holds itself, a piece far out in the air breaks and drops its
  materials. Nothing else damages them, no hits, fire or weather.
- **Stations.** A station is placed beside the track, its origin on the
  track. The use key names it through the vanilla text field. A name another
  station has is refused, a station with no name is not in the menu.
- **Locomotive and wagons.** They go only onto track, the ghost sits on the
  centerline under the cursor and is red off the track. The hammer rotation
  picks which way they face. Like the vanilla cart they have a WearNTear, so
  the hammer highlights and removes them, but they need no support and take
  no damage. A body sits between its outer axles on the track and faces from
  one to the other, so a kink between pieces turns it gradually.
- **Coal.** The alternative use key on the locomotive loads all the coal the
  player carries, up to 100. With free crafting or free building, like on
  `Test`, it fills up for nothing. The chimney smokes while there is coal,
  the smoke of the vanilla smelter, bigger and more with speed. Burnt food on a cooking station gives coal from
  day 1. A trip costs 1 coal per 100 m of track and is paid at departure,
  driving by hand burns the same as it goes.
- **Sending a train.** The use key on the locomotive opens the menu, every
  named station with its coal cost. A click sends the train there at 6 m/s,
  it stops at the station and waits for the next command. Anyone can command
  any train. A route only goes forward, a station behind the train needs a
  loop.
- **Driving.** The use key on the bench at the back of the locomotive sits
  down to drive. Forward and backward set the speed, up to 10 m/s forward and
  4 m/s back. Left or right picks the branch at the next switch, straight on
  otherwise. Jump gets up.
- **Wagons.** Up to 3. A wagon placed on the track within 2.5 m of the back
  of a train couples with the alternative use key, the same key on the last
  wagon uncouples it. The use key opens its 6 by 3 storage.

## Trains share the track

Before a train leaves, the server reserves all the track to its station in
one step. The train waits until no other train stands on that track or has
reserved it, so trains never overlap and never jam. A train driven by hand
stops before track another train covers.

Reserved track can still be removed with the hammer. The train then stops
before the gap and waits, and goes on once the gap is filled. The route is
kept as the end points of its lines, so a rebuilt piece in the same place
fills the gap. A new command also frees it.

The list of trains that wait for track is kept in the memory of the server,
a server restart forgets it. The command is then given again.

## How it works

The state of a train lives in the saved data of its locomotive: the line it
is on, how far along, which way it faces, the lines behind it, the route, the
coal, the wagons and the driver. Whoever owns that data moves the train, each
frame, in `Train.Step`.

- The client of a player near the train owns it, as the game does for every
  object. A driver's client takes it over when the driver sits down, so the
  controls answer at once.
- The server takes every train nobody near owns and moves it on its saved
  data alone. Nothing of it has to be loaded. A player who comes near sees the
  train at the right spot, and the game hands the train to that player.
- A client takes over a train the server moves within 80 m of its player, the
  game alone leaves it with the server near the world center. So the train
  moves every frame for that player. Other players see it glide on at its
  speed between the network updates.- The locomotive sets the position of its wagons, walked back along the lines
  it came over.

`Graph` builds the track network from the saved data of every placed track
piece, rebuilt once a second. Each piece brings the centerlines of its model,
a switch or a crossing has two. Lines join where their ends meet within
0.3 m and the direction goes on, so the two lines of a switch never lead into
each other.

`Network` carries the requests between the players, the owner of a train and
the server: the menu list, a trip, coal, coupling, driving and station names.
The server plans routes with the shortest path over the graph, checks the
coal and reserves the track, then tells the owner to go. Only the owner of a
locomotive writes its data.

## The parts

- `assets/rail/Plugin.cs` starts the plugin and puts the pieces on the hammer.
- `Pieces.cs` builds the prefabs under an inactive holder with their
  components, the vanilla materials, box colliders and snap points.
- `Models.cs` reads the embedded meshes and icons.
- `RailTrack.cs` and `Placement.cs`, the track lines of a placed piece and the
  rule that trains go onto track only.
- `RailPiece.cs` keeps hits off rail pieces.
- `Graph.cs`, `TrackCursor.cs` and `Train.cs`, the network, a place on it and
  the train itself.
- `Network.cs`, the requests, the route planner, the reservation and the
  `Simulation` that moves owned trains each frame.
- `Station.cs`, `Vehicles.cs` and `StationMenu.cs`, the station, the
  locomotive, seat, wagons and coupling, and the menu.
- `crates/blackforge-core/src/rail.rs` writes the dll to
  `BepInEx/plugins/blackforge-rail` before every start of the Valheim client.

A server needs the same dll, it plans the routes and moves trains nobody is
near. The test server gets it like the status plugin, a raw GitHub link pinned
to a commit and its sha256, see `status.md`.

## The models

The models are Blender Python scripts in `assets/rail/models`. `tracks.py`
builds every track piece along a path of sleepers and stone rail blocks, the
others have one script each. `parts.py` holds the sizes every model shares:
the gauge of 1.4 m, the wheels and the deck height.

A script builds only the shape. Every part has a slot, `wood`, `stone` or
`iron`, and the plugin puts the vanilla material of that slot on it at
runtime: `woodwall` from `wood_beam`, `stone_mat` from `stone_wall_1x1` and
`Ironbeam_mat` from `woodiron_beam`. So the pieces look like vanilla pieces
and the repo ships no game textures.

```sh
make rail-models
```

This runs `models/export.py` in Blender. It writes `assets/rail/meshes/*.bfm`
and the hammer icons in `assets/rail/icons`, both committed and embedded by
the plugin build. A `.bfm` file has a mesh part per slot, a box collider per
model part, the track ends and the track centerlines. Blender has z up and is
right handed, so the export swaps y and z and writes every triangle in
reverse order, else the game shows the inside of the models.

The game places a piece by its convex colliders and skips a concave mesh
collider. So every part is also a box collider, a mesh collider alone puts
the ghost far off the cursor.

### Previews

```sh
cd assets/rail/models && blender -b --python render.py -- wagon /tmp/previews
```

This renders a model from 4 sides, a wagon or locomotive on track next to a
vanilla cart for scale. The previews use the real vanilla textures from a
local export of the game, never from the repo:

1. Download [AssetRipper](https://github.com/AssetRipper/AssetRipper) and start
   it headless: `AssetRipper.GUI.Free --headless --port 18765`.
2. Load the game data folder with `POST /LoadFolder` and export it with
   `POST /Export/PrimaryContent` to
   `~/Library/Caches/blackforge-rail/export`.
3. Save the `woodwall`, `stone_mat` and `Ironbeam_mat` textures to
   `~/Library/Caches/blackforge-rail/textures` as `wood.png`, `stone.png` and
   `iron.png`.

`BLACKFORGE_EXPORT` and `BLACKFORGE_TEXTURES` point elsewhere. The preview
only comes close to the game, the final look is judged in the game.

## Things the game version changed

- The build menu of Valheim 1.0 groups pieces by `Piece.m_usage`, the usage
  tags, not by `m_category`. The rail pieces carry the Transport tag.
- The object database of the main menu has a hammer with no piece table, the
  pieces go on the hammer in the game scene.
- `Hoverable` needs `GetHoverOffset`. `Localization` lives in
  `assembly_guiutils`.
- The seat uses `IDoodadController` like the ship's rudder, the game spells
  its method `ApplyControlls`, `_typos.toml` allows it.

## Building the dll

It compiles against the game dlls, like the join plugin, see `join.md`.

```sh
make rail-plugin VALHEIM_MANAGED=/path/to/Valheim/valheim_Data/Managed
```

Run `make rail-models` first when a model changed. Build the app again after
a new dll, the Rust side embeds it at compile time.

## Testing

`cargo test -p blackforge-core rail` checks that the dll is written. The rest
is checked in the game, first in a local world, where the game is its own
server, then on `Test`. `BepInEx/LogOutput.log` has `rails ready`,
`rail network ready`, `rail pieces built` with all 21 prefabs and
`chimney smoke from smelter`.
