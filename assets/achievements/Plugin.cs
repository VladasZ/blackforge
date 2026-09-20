using System;
using System.Reflection;
using BepInEx;
using HarmonyLib;

namespace Blackforge
{
    // Valheim 1.0 treats a modded game as a cheated one. Achievements.IsCheatedAtAll
    // ends with `|| Game.isModded`, and BepInEx sets that flag at start. The flag is
    // hidden only while that one check runs. The devcommand, world modifier and
    // spawned item checks still count, and the menu still says the game is modded.
    // The game types are found by name, so the build needs no game files.
    [BepInPlugin("xyz.vladas.blackforge.achievements", "Blackforge Achievements", "1.0.0")]
    public class AchievementsPlugin : BaseUnityPlugin
    {
        private static FieldInfo isModded;

        private void Awake()
        {
            MethodInfo check = Type.GetType("Achievements, assembly_valheim")
                ?.GetMethod("IsCheatedAtAll", BindingFlags.Public | BindingFlags.Static);
            isModded = Type.GetType("Game, assembly_valheim")
                ?.GetField("isModded", BindingFlags.Public | BindingFlags.Static);

            if (check == null || isModded == null || isModded.FieldType != typeof(bool))
            {
                Logger.LogWarning("this game version has no known achievement check, nothing is patched");
                return;
            }

            new Harmony(Info.Metadata.GUID).Patch(
                check,
                prefix: new HarmonyMethod(typeof(AchievementsPlugin), nameof(Hide)),
                finalizer: new HarmonyMethod(typeof(AchievementsPlugin), nameof(Restore)));
            Logger.LogInfo("mods no longer block achievements, real cheats still do");
        }

        private static void Hide(out bool __state)
        {
            __state = (bool)isModded.GetValue(null);
            isModded.SetValue(null, false);
        }

        // A finalizer also runs when the check throws, so the flag always comes back.
        private static void Restore(bool __state)
        {
            isModded.SetValue(null, __state);
        }
    }
}
