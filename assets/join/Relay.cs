using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Reflection;
using System.Text;
using HarmonyLib;
using PartyCSharpSDK;
using PlayFab.Party;

namespace Blackforge
{
    // Logs of the network layer of the game, see docs/reports.md. It changes
    // nothing in the game. The game logs little here: the PlayFab socket
    // closes without a line in some paths, and PlayFab Party logs its own
    // state changes only at a verbose level that is off. So this logs every
    // Party state change with its reason and error code, every close of a
    // socket with who called it, every ack outside the messages in flight,
    // and keeps the last messages in and out in a ring the report carries.
    public static class Relay
    {
        private const int RingSize = 200;
        // Types from ZPlayFabSocket, the last byte of every plain message.
        private const byte Data = 17;
        private const byte Ack = 42;
        private const byte Internal = 64;

        // One message in the ring. Kept raw, a report formats them, so the
        // ring costs almost nothing per message.
        private struct Entry
        {
            public long ticks;
            public bool incoming;
            public bool compressed;
            public byte type;
            public uint id;
            public int length;
            public string note;
        }

        private static BepInEx.Logging.ManualLogSource log;
        private static readonly Entry[] ring = new Entry[RingSize];
        private static int next;
        private static int count;

        // Party state changes that come per message or are about voice and
        // text chat, which the game does not use for the connection.
        private static readonly HashSet<PARTY_STATE_CHANGE_TYPE> quiet = new HashSet<PARTY_STATE_CHANGE_TYPE>
        {
            PARTY_STATE_CHANGE_TYPE.PARTY_STATE_CHANGE_TYPE_ENDPOINT_MESSAGE_RECEIVED,
            PARTY_STATE_CHANGE_TYPE.PARTY_STATE_CHANGE_TYPE_DATA_BUFFERS_RETURNED,
            PARTY_STATE_CHANGE_TYPE.PARTY_STATE_CHANGE_TYPE_CHAT_TEXT_RECEIVED,
            PARTY_STATE_CHANGE_TYPE.PARTY_STATE_CHANGE_TYPE_VOICE_CHAT_TRANSCRIPTION_RECEIVED,
            PARTY_STATE_CHANGE_TYPE.PARTY_STATE_CHANGE_TYPE_LOCAL_CHAT_AUDIO_INPUT_CHANGED,
            PARTY_STATE_CHANGE_TYPE.PARTY_STATE_CHANGE_TYPE_LOCAL_CHAT_AUDIO_OUTPUT_CHANGED,
            PARTY_STATE_CHANGE_TYPE.PARTY_STATE_CHANGE_TYPE_SYNTHESIZE_TEXT_TO_SPEECH_COMPLETED,
        };

        public static void Patch(Harmony harmony, BepInEx.Logging.ManualLogSource logger)
        {
            log = logger;
            harmony.Patch(
                AccessTools.Method(typeof(ZPlayFabSocket), nameof(ZPlayFabSocket.ProcessAck)),
                prefix: new HarmonyMethod(typeof(Relay), nameof(CheckAck)));
            harmony.Patch(
                AccessTools.Method(typeof(ZPlayFabSocket), nameof(ZPlayFabSocket.Dispose)),
                prefix: new HarmonyMethod(typeof(Relay), nameof(OnDispose)));
            harmony.Patch(
                AccessTools.Method(typeof(ZPlayFabSocket), nameof(ZPlayFabSocket.OnDataMessageReceived)),
                prefix: new HarmonyMethod(typeof(Relay), nameof(OnReceived)));
            harmony.Patch(
                AccessTools.Method(typeof(ZPlayFabSocket), nameof(ZPlayFabSocket.InternalSend)),
                prefix: new HarmonyMethod(typeof(Relay), nameof(OnSend)));
            harmony.Patch(
                AccessTools.Method(typeof(ZPlayFabSocket), nameof(ZPlayFabSocket.VersionMatch)),
                prefix: new HarmonyMethod(typeof(Relay), nameof(OnVersionMatch)));
            harmony.Patch(
                AccessTools.Method(typeof(SDK), nameof(SDK.PartyStartProcessingStateChanges)),
                postfix: new HarmonyMethod(typeof(Relay), nameof(OnStateChanges)));
        }

        // The ring as lines, oldest first.
        public static List<string> Traffic()
        {
            List<string> lines = new List<string>(count);
            for (int i = 0; i < count; i++)
            {
                Entry entry = ring[(next - count + i + RingSize) % RingSize];
                string time = new DateTime(entry.ticks, DateTimeKind.Utc).ToString("HH:mm:ss.fff");
                lines.Add(entry.note != null ? $"{time} {entry.note}" : $"{time} {Describe(entry)}");
            }
            return lines;
        }

        private static void Add(Entry entry)
        {
            entry.ticks = DateTime.UtcNow.Ticks;
            ring[next] = entry;
            next = (next + 1) % RingSize;
            count = Math.Min(count + 1, RingSize);
        }

        private static void Note(string text)
        {
            Add(new Entry { note = text });
        }

        private static string Describe(Entry entry)
        {
            string way = entry.incoming ? "in " : "out";
            if (entry.compressed)
            {
                return $"{way} {entry.length} bytes compressed";
            }
            if (entry.length < 5)
            {
                return $"{way} {entry.length} bytes, too short";
            }
            string name = entry.type == Data ? "data" : entry.type == Ack ? "ack" : entry.type == Internal ? "internal" : $"type {entry.type}";
            return $"{way} {name} id {entry.id}, {entry.length} bytes";
        }

        private static Entry Read(byte[] buffer, bool incoming, bool compressed)
        {
            Entry entry = new Entry { incoming = incoming, compressed = compressed, length = buffer?.Length ?? 0 };
            if (!compressed && buffer != null && buffer.Length >= 5)
            {
                int at = buffer.Length - 5;
                entry.id = (uint)(buffer[at] | buffer[at + 1] << 8 | buffer[at + 2] << 16 | buffer[at + 3] << 24);
                entry.type = buffer[buffer.Length - 1];
            }
            return entry;
        }

        private static void Tell(string text, bool warning = false)
        {
            if (warning)
            {
                log.LogWarning(text);
            }
            else
            {
                log.LogInfo(text);
            }
            Report.Step(text);
            Note(text);
        }

        private static void CheckAck(ZPlayFabSocket __instance, uint msgId)
        {
            if (!__instance.m_isClient)
            {
                return;
            }
            ZPlayFabSocket.InFlightQueue queue = __instance.m_inFlightQueue;
            // Unsigned, so an id below the tail wraps to a huge distance.
            if (msgId - queue.Tail <= queue.Head - queue.Tail)
            {
                return;
            }
            Tell($"ack {msgId} is outside the messages in flight {queue.Tail} to {queue.Head}, the game closes the socket", true);
        }

        private static void OnDispose(ZPlayFabSocket __instance)
        {
            if (!__instance.m_isClient || __instance.m_state == ZPlayFabSocketState.CLOSED)
            {
                return;
            }
            ZPlayFabSocket.InFlightQueue queue = __instance.m_inFlightQueue;
            double quiet = (DateTime.UtcNow - ZPlayFabSocket.s_lastReception).TotalSeconds;
            // Skip this method and the Harmony frame, keep the callers.
            string callers = new StackTrace(2, false).ToString().Replace("\r", "").Replace("\n", " <- ");
            Tell($"socket to {__instance.m_remotePlayerId} {__instance.m_state} closes, compression {__instance.m_useCompression}, "
                + $"in flight {queue.Tail} to {queue.Head} with {queue.Bytes} bytes, next in {__instance.m_next}, "
                + $"last message in {quiet:0.0}s ago, called from {callers}");
        }

        private static void OnReceived(ZPlayFabSocket __instance, PlayFabPlayer from, byte[] compressedBuffer)
        {
            if (__instance.m_isClient && from?.EntityKey?.Id == __instance.m_remotePlayerId)
            {
                Add(Read(compressedBuffer, true, __instance.m_useCompression));
            }
        }

        private static void OnSend(ZPlayFabSocket __instance, byte[] payload)
        {
            if (__instance.m_isClient)
            {
                // The payload is still plain here, compression comes after.
                Add(Read(payload, false, false));
            }
        }

        private static void OnVersionMatch(ZPlayFabSocket __instance)
        {
            if (__instance.m_isClient)
            {
                Tell("socket compression on");
            }
        }

        // Every Party state change with all its values, the reasons and the
        // error codes are what the game never logs.
        // The list is an out parameter of the SDK call, Harmony hands it by
        // ref. The join plugin runs only in the game client, and a drop ends
        // with ZNet gone, so no check of ZNet here.
        private static void OnStateChanges(ref List<PARTY_STATE_CHANGE> stateChanges)
        {
            if (stateChanges == null)
            {
                return;
            }
            foreach (PARTY_STATE_CHANGE change in stateChanges)
            {
                if (change == null || quiet.Contains(change.StateChangeType))
                {
                    continue;
                }
                Tell("party " + Values(change), IsFailure(change));
            }
        }

        private static string Values(PARTY_STATE_CHANGE change)
        {
            StringBuilder text = new StringBuilder(change.StateChangeType.ToString().Replace("PARTY_STATE_CHANGE_TYPE_", ""));
            foreach (PropertyInfo property in change.GetType().GetProperties(BindingFlags.Public | BindingFlags.Instance))
            {
                Type type = property.PropertyType;
                if (property.Name == nameof(PARTY_STATE_CHANGE.StateChangeType)
                    || !(type.IsPrimitive || type.IsEnum || type == typeof(string)))
                {
                    continue;
                }
                object value;
                try
                {
                    value = property.GetValue(change);
                }
                catch (Exception error)
                {
                    value = "unreadable: " + error.Message;
                }
                text.Append($" {property.Name}={value}");
                if (property.Name == "errorDetail" && value is uint code && code != 0)
                {
                    text.Append($" ({ErrorText(code)})");
                }
            }
            return text.ToString();
        }

        private static bool IsFailure(PARTY_STATE_CHANGE change)
        {
            PropertyInfo detail = change.GetType().GetProperty("errorDetail");
            return detail != null && detail.GetValue(change) is uint code && code != 0;
        }

        private static string ErrorText(uint code)
        {
            try
            {
                return SDK.PartyGetErrorMessage(code, out string message) == 0 ? message : "no text";
            }
            catch (Exception error)
            {
                return "no text: " + error.Message;
            }
        }
    }
}
