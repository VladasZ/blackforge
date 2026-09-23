# Roadmap

Ideas that are not part of the first version.

## Later

- Nexus Mods as a mod source. Every user needs a personal Nexus API key. Direct downloads through the API work only for paid Nexus accounts. Nexus mods have no machine readable dependency list.
- Install a mod from a local zip file, for example one downloaded by hand from Nexus. Such a mod gets no updates and no dependency data.
- Run the Windows build of the game through Proton on Linux. Today `run` refuses it with a clear error, only the native Linux build starts.

## Later, the window

The window app has no profiles, it always works on the profile named `default`. The command line keeps all of them.

- Import of an r2modman code or `.r2z` file. The core can only import into a new profile, which the window cannot show. It needs a core function that replaces the mods of an existing profile, then a button on the Mods page.
- Font weights. The engine embeds one font file, Roboto Regular, so a title cannot be heavier than the gray text under it. Bundling a variable font such as Inter would fix that.
- Characters outside the default font draw as boxes, the phonetic spelling in the Jotunn description for example. This waits for system font discovery in the hilen engine, see its roadmap.
- A save dialog for the export file. The engine has none yet, so the window asks for a folder and names the file itself.

## Next, the window

Left from the September 2026 review.

- Check the rest in the running app. The Run game states, the wrapped toasts, the Doctor fix buttons, the busy state of every button that changes the profile, and the Account card while signed in. They build and pass the gate but were not looked at on screen yet.
- The status bar progress line is the blue engine default, not the accent.
