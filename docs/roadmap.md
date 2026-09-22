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

Agreed UI changes from the September 2026 review that are not built yet. Each line is one decision.

- Configs adapts to a narrow column. The value field shrinks to a minimum width, then moves under the description. Today setting names vanish and descriptions wrap one letter per line.
- Configs names a mod with `names::title`, so `BepInExPack_Valheim` reads as on every other page.
- Configs shows a dropdown when the file lists the accepted values, and toggle pills when several values can be picked at once, like `Warn, Error`. The core already reads them into `Setting::acceptable`.
- Configs refuses a value outside an accepted range like `From 1 to 10`, with a short note under the field. The text field stays.
- The achievements hint in Settings wraps under the switch label, and the card grows to fit.
- Settings gets an Account card with the username and sign in or sign out.
- Settings shows the app version in small gray text, next to a link that checks for an app update.
- Friends when signed out shows a card with 3 lines on what signing in gives, see friends' mods, copy their configs and sync the setup, with the Google button inside it.
- Share lines its buttons and the switch up with the card text. Today they start about 14 points to the left.
- Server pills mark a missing or wrong version mod with an orange border and a short note like `missing`. Mods the profile has stay plain.
- Sentence case for every text that is still lowercase, the subtitles of Configs, Friends, Servers, Share and Settings, the dim labels and the modal texts.
- The busy state on the rest of the buttons that change the profile: Add in the mod details, Add and Copy config on a friend's mods, Install on a server and Deploy on Share. Each one calls `busy::track` once and `busy::press` before `backend::change`.
- Check the new pieces in the running app, the sidebar badges, the Run game states, the wrapped toasts, the Browse pages and header, and the Doctor fix buttons. They build and pass the gate but were not looked at on screen yet.
