using HarmonyLib;
using PlayFab.Party;

namespace Blackforge
{
    // A player who starts the game again and joins within about 90 seconds
    // meets the socket the server still holds from the last game, see
    // docs/gate.md. The game then resumes that old connection, the numbers of
    // its messages do not match the new game, and every join fails until the
    // held socket times out. The game has a check for a restarted game, but it
    // only runs after a recovery flag that nothing sets.
    //
    // The first message of every game session is message 0 of the internal
    // type, which carries the platform id. A connection that only resumes
    // after a network blip goes on with its old numbers and never sends it
    // again. So when a held socket gets that message, it starts over.
    public static class Rejoin
    {
        private const byte Internal = 64;
        private static BepInEx.Logging.ManualLogSource log;

        public static void Patch(Harmony harmony, BepInEx.Logging.ManualLogSource logger)
        {
            log = logger;
            harmony.Patch(
                AccessTools.Method(typeof(ZPlayFabSocket), nameof(ZPlayFabSocket.OnDataMessageReceived)),
                prefix: new HarmonyMethod(typeof(Rejoin), nameof(OnReceived)));
        }

        private static void OnReceived(ZPlayFabSocket __instance, PlayFabPlayer from, byte[] compressedBuffer)
        {
            if (__instance.m_isClient || __instance.m_next == 0 || from.EntityKey.Id != __instance.m_remotePlayerId)
            {
                return;
            }
            byte[] buffer = compressedBuffer;
            if (buffer == null || buffer.Length < 5 || buffer[buffer.Length - 1] != Internal)
            {
                return;
            }
            int at = buffer.Length - 5;
            uint id = (uint)(buffer[at] | buffer[at + 1] << 8 | buffer[at + 2] << 16 | buffer[at + 3] << 24);
            if (id != 0)
            {
                return;
            }
            log.LogInfo($"a new game of {__instance.GetEndPointString()} joins on the socket the server still holds, it starts over");
            __instance.ResetAll();
        }
    }
}
