# Competitive servers

On a competitive server a player may not bring items from a tier the world has
not reached. Arkham Asylum is one. A tier is a boss and the biome its death
unlocks. Before The Elder dies, nobody may bring iron, iron gear, swamp food or
a Draugr trophy.

## What a player sees

1. The player clicks the join button of the server in the Valheim menu.
2. Character select opens. If the character carries a forbidden item, Start is
   off. A panel right of the character lists every forbidden item with its
   icon and the boss that frees it.
3. The player leaves those items in another world, or picks another character.

Items found or made in the competitive world itself are always allowed there,
see "The world tag" below.

## The tiers

`backend/server/tiers.toml` holds the tiers of each game, kept by hand. A tier
is the global key the game sets when its boss dies, the name of the boss, and
the materials of the biome it unlocks. Only raw materials, boss drops and
trophies are listed. The names are prefab names of the game.

The plugins find everything else by themselves in `assets/shared/Tiers.cs`.
An item is forbidden when it is made from a forbidden material through a
recipe, a smelter, a fermenter or a cooking station. So bronze, a bronze sword,
a bronze idol and cooked lox meat are all caught by the few rows for copper,
tin and lox meat. An item with several recipes is forbidden only when every
recipe is. An ingredient needed only for upgrades does not count. An item made
from materials of several tiers names the latest boss.

The Valheim bosses in order, with their keys: Eikthyr `defeated_eikthyr`,
The Elder `defeated_gdking`, Bonemass `defeated_bonemass`, Moder
`defeated_dragon`, Yagluth `defeated_goblinking`, The Queen `defeated_queen`,
Fader `defeated_fader`, and the Frozen King of the Deep North,
`defeated_frozenking_p3` for its last phase. The names come from a dump of
the game data of version 1.0.15.

A change of the file is a backend push, no app release and no server restart.
The servers ask the backend again every 5 minutes. The server plugin logs any
name of the file the game does not know.

## The world tag

An item picked up, crafted or taken from a chest in a competitive world gets a
tag with the id of that world, `blackforge.world` in the custom data the game
saves with every item. A tagged item is allowed in its world, so fair loot
stays with the player across logins.

- Tagging starts only after the server cleared the player, see below. Before
  that nothing is tagged.
- A stack keeps its tag only when what joins it has the same tag. So 1 tagged
  iron cannot turn 100 iron from another world into tagged iron. This works in
  every world, also in chests, as long as the game runs with the join plugin.
- An upgraded item is a new item for the game and loses the tag. In the
  competitive world it gets the tag again at once.
- A new world has a new id. A wiped world makes all old tags useless.

## The server check

The server does not trust character select alone.

1. After a player spawns, the server asks the join plugin for the whole
   inventory, each item with its prefab name and tag.
2. A forbidden item without the tag of this world kicks the player. The kick
   text names the items, the join plugin shows it in the menu.
3. No answer within 30 seconds kicks the player too. An old app or a game
   started without Blackforge cannot answer.
4. A clean report clears the player, the join plugin then starts to tag.
5. Every 2 seconds the server also looks at the worn items, which every client
   sees on the body of the player. A worn forbidden item kicks the player
   unless the report listed a tagged copy of it.
6. The join plugin sends the inventory again whenever it changes.

The report comes from the player's own game, so a modified plugin can lie. The
check stops honest mistakes and old apps, not a determined cheater.

## The parts

- `crates/blackforge-api/src/competitive.rs`: the tiers, `Progress` and
  `Rules`, and `forbidden`, which picks the tiers of the living bosses.
- `backend/server/src/tiers.rs` loads `tiers.toml`, a test checks every row.
- `backend/server/src/gate.rs`: `POST /api/gate/progress` with the gate
  secret. The server sends its name, its world id and its boss keys, the
  answer is `Rules`. `POST /api/servers/{id}/join` adds `competitive`, `world`
  and `forbidden` to the code.
- Migration `0009` adds `competitive`, `world`, `boss_keys` and `progress_at`
  to `servers`.
- The Competitive switch of the server form sets the flag, only the admin sees
  it. No server restart is needed.
- `crates/blackforge/src/bridge.rs` hands the whole join answer to the plugin
  as JSON.
- `assets/join/Competitive.cs`: character select, the panel, the tags and the
  reports.
- `assets/status/Competitive.cs`: the progress, the inventory check and the
  kicks.

## Tests

`cargo test -p blackforge-api competitive` checks the pick of the tiers and the
wire shapes. `cargo test -p blackforge-server tiers` checks the file. The plugins
are checked in the game, a throwaway server first, see `status.md`.
