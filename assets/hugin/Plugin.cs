using BepInEx;
using HarmonyLib;

namespace Blackforge
{
    // Keeps Hugin and Munin, the tutorial ravens, from ever showing up. Both
    // birds appear only through Raven.Spawn, so the patch skips it and does
    // what a talk with the bird would do: the tutorial counts as seen and its
    // text goes into the compendium. The game's own Tutorials switch only
    // skips the bird and loses the text.
    [BepInPlugin("xyz.vladas.blackforge.hugin", "Blackforge Hugin", "1.0.0")]
    public class HuginPlugin : BaseUnityPlugin
    {
        private void Awake()
        {
            Harmony harmony = new Harmony(Info.Metadata.GUID);
            harmony.Patch(
                AccessTools.Method(typeof(Raven), nameof(Raven.Spawn)),
                prefix: new HarmonyMethod(typeof(HuginPlugin), nameof(SkipSpawn)));
            Logger.LogInfo("the ravens stay away");
        }

        private static bool SkipSpawn(Raven.RavenText text)
        {
            Player player = Player.m_localPlayer;
            if (player == null)
            {
                return false;
            }
            // Seen takes a temp text off the queue, so the game stops asking
            // for this spawn every second.
            player.SetSeenTutorial(text.m_key);
            if (text.m_label.Length > 0)
            {
                player.AddKnownText(text.m_label, text.m_text);
            }
            return false;
        }
    }
}
