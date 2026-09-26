using System;
using BepInEx;
using BepInEx.Logging;
using HarmonyLib;

namespace Blackforge
{
    // Railroads: stone track, stations, a coal locomotive and wagons, all
    // built with the hammer. See docs/rail.md.
    [BepInPlugin("xyz.vladas.blackforge.rail", "Blackforge Rail", "0.1.0")]
    public class RailPlugin : BaseUnityPlugin
    {
        public static ManualLogSource Log;

        private void Awake()
        {
            Log = Logger;
            try
            {
                new Harmony(Info.Metadata.GUID).PatchAll(typeof(RailPlugin).Assembly);
            }
            catch (Exception error)
            {
                Log.LogError($"rails are off, a patch failed: {error}");
                return;
            }
            Log.LogInfo("rails ready");
        }

        [HarmonyPatch(typeof(ZNetScene), "Awake")]
        private static class RegisterPrefabs
        {
            // Before Awake, which files every prefab of the list by name.
            private static void Prefix(ZNetScene __instance)
            {
                if (!Pieces.Ensure(__instance.m_prefabs))
                {
                    Log.LogError("the vanilla materials were not found, the rail pieces are off");
                    return;
                }
                foreach (var prefab in Pieces.All)
                {
                    if (!__instance.m_prefabs.Contains(prefab))
                    {
                        __instance.m_prefabs.Add(prefab);
                    }
                }
            }
        }

        [HarmonyPatch(typeof(ObjectDB), "Awake")]
        private static class AddToHammer
        {
            private static void Postfix(ObjectDB __instance)
            {
                Pieces.AddToHammer(__instance);
            }
        }

        [HarmonyPatch(typeof(ObjectDB), "CopyOtherDB")]
        private static class AddToCopiedHammer
        {
            private static void Postfix(ObjectDB __instance)
            {
                Pieces.AddToHammer(__instance);
            }
        }
    }
}
