using System;
using HarmonyLib;

namespace Blackforge
{
    // A player who starts the game again and joins within about 90 seconds
    // meets the socket the server still holds from the last game, see
    // docs/gate.md. The server answers with the acks of that old connection,
    // and the game closes a socket at once on an ack for a message it never
    // sent. So in the first seconds of a connection such an ack is ignored,
    // until the server sees the first message of this game and starts the
    // held connection over, see the status plugin.
    public static class Rejoin
    {
        private const double Grace = 30;
        private static DateTime connectedAt;
        private static BepInEx.Logging.ManualLogSource log;

        public static void Patch(Harmony harmony, BepInEx.Logging.ManualLogSource logger)
        {
            log = logger;
            harmony.Patch(
                AccessTools.Method(typeof(ZPlayFabSocket), nameof(ZPlayFabSocket.Connect)),
                postfix: new HarmonyMethod(typeof(Rejoin), nameof(OnConnect)));
            harmony.Patch(
                AccessTools.Method(typeof(ZPlayFabSocket), nameof(ZPlayFabSocket.ProcessAck)),
                prefix: new HarmonyMethod(typeof(Rejoin), nameof(SkipOldAck)) { priority = Priority.High });
        }

        private static void OnConnect(ZPlayFabSocket __instance)
        {
            if (__instance.m_isClient)
            {
                connectedAt = DateTime.UtcNow;
            }
        }

        private static bool SkipOldAck(ZPlayFabSocket __instance, uint msgId)
        {
            if (!__instance.m_isClient || (DateTime.UtcNow - connectedAt).TotalSeconds > Grace)
            {
                return true;
            }
            ZPlayFabSocket.InFlightQueue queue = __instance.m_inFlightQueue;
            // Unsigned, so an id below the tail wraps to a huge distance.
            if (msgId - queue.Tail <= queue.Head - queue.Tail)
            {
                return true;
            }
            log.LogWarning($"ack {msgId} is outside the messages in flight {queue.Tail} to {queue.Head}, from the connection the server still holds, ignored");
            return false;
        }
    }
}
