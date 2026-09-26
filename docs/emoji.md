# Emojis

A player types `:petuh` in the Valheim chat and gets the rooster, a GIF that
plays in the chat window and above the head of that player. Today the rooster
is the only emoji.

## How it looks

- The code is one colon and the name, matched as a whole word, in any case.
  `hi :petuh` and `:petuh!` show the rooster. `a:petuh` and `:petuhs` stay
  text.
- In the chat window the rooster stands inside the line, about 6 text lines
  tall, also in the middle of a sentence.
- Above the head the code is taken out of the bubble, and a big rooster shows
  above the bubble. A message that is only `:petuh` shows no bubble, just the
  rooster. It goes away with the bubble, after about 5 seconds.
- More than one code in a message shows each one in the chat, and the first
  one above the head.

The message goes over the network as the plain text `:petuh`, so the server
needs nothing. Every player who started the game from blackforge sees the
rooster. A player without the plugin sees the text `:petuh`. Normal chat
reaches only players close by, a shout with `/s` reaches everyone, as in the
plain game.

## The parts

- `assets/emoji/Plugin.cs` is a small `BepInEx` plugin. Three Harmony patches:
  - A postfix on `Chat.Awake` hands the chat text its sprite asset.
  - A prefix on `Terminal.AddString(string)` swaps each code in a chat line for
    a TextMeshPro `<sprite anim>` tag. The console is a Terminal too, it is
    left alone.
  - A postfix on `Chat.AddInworldText` takes the code out of the head bubble,
    hides an empty bubble and puts the big emoji above it. The game reuses one
    bubble per player, so every message sets it again.
- `assets/emoji/BlackforgeEmoji.dll` is the built plugin. It is committed, the
  app embeds it.
- `crates/blackforge-core/src/emoji.rs` writes the dll, one PNG sprite sheet
  per emoji and `emojis.json` to `BepInEx/plugins/blackforge-emoji` before
  every start of the Valheim client. There is no setting. It goes in with the
  join and hugin plugins, on the `client_plugins` flag of the launch plan. A
  server gets nothing.

Unity cannot read a GIF. So the app turns every GIF into a sheet at each
start, the frames in rows of 8 from the top left. The plugin plays the whole
GIF at one speed, the average of its frame times. This takes about 0.1 second
in a release build.

```json
{ "emojis": [{ "name": "petuh", "file": "petuh.png", "width": 320, "height": 200, "columns": 8, "frames": 67, "fps": 10 }] }
```

The plugin builds its TextMeshPro sprite assets at runtime from the sheets,
one asset per emoji, since an asset has one texture. The first frame carries
the name of the emoji, the `anim` attribute plays the rest.

## The sizes

Both are constants at the top of `Plugin.cs`.

- `ChatScale` is the height in the chat as a multiple of the font size. It is
  6.4 now.
- `BigHeight` is the height above the head, in units of the chat canvas. It
  is 160 now.

## Adding an emoji

Put the GIF in `assets` and add a line to `EMOJIS` in `emoji.rs` with its
name. The name is the code without the colon. The plugin needs no change, it
reads the list. Build the app again, it embeds the GIF.

## Building the dll

It compiles against the game dlls, like the join plugin, see `join.md`.

```sh
make emoji-plugin VALHEIM_MANAGED=/path/to/Valheim/valheim_Data/Managed
```

Build the app again after a new dll, the Rust side embeds it at compile time.

## Tests

`cargo test -p blackforge-core emoji` checks that the dll, the sheet and the
list are written, and the size and frame count of the rooster sheet. The
emojis themselves are checked by hand in the running game. `hi :petuh` shows a
moving rooster in the chat and above the head, a message of only `:petuh`
shows no bubble, and the next plain message brings the bubble back with no
rooster. `BepInEx/LogOutput.log` of the profile has the line
`emojis ready: petuh`.
