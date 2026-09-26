using System;
using System.Collections;
using System.Collections.Generic;
using System.IO;
using System.Text;
using BepInEx;
using HarmonyLib;
using Newtonsoft.Json;
using UnityEngine;
using UnityEngine.Networking;

namespace Blackforge
{
    // Every failed join and every dropped connection goes to the backend with
    // what the game saw, so a bad connection can be debugged after the fact,
    // see docs/reports.md. The plugin keeps a trail of the steps since the
    // click on a join button and sends it with the newest lines of the game
    // logs to the running app, which sends it on.
    public static class Report
    {
        private const string Door = "report";
        private const int SendTimeout = 10;
        // The backend keeps 512 KB of logs, both tails together stay below.
        private const int BepInExTail = 320 * 1024;
        private const int PlayerTail = 160 * 1024;
        private const int TrailLines = 200;
        // The menu can show the same error twice in a row.
        private const float RepeatSeconds = 5f;

        // The body of POST /report, `ConnectionReport` of blackforge-api.
        private class Body
        {
            public string server_id = "";
            public string server = "";
            public string status = "";
            public string message = "";
            public bool in_world;
            public double seconds;
            public string game_version = "";
            public string plugin_version = "";
            public string os = "";
            public List<string> trail = new List<string>();
            public string log = "";
        }

        private static BepInEx.Logging.ManualLogSource log;
        private static string pluginVersion;
        private static readonly List<string> trail = new List<string>();
        private static float started = -1f;
        private static ZNet.ConnectionStatus lastStatus = ZNet.ConnectionStatus.None;
        private static bool inWorld;
        private static string lastSent;
        private static float lastSentAt = -100f;

        public static void Patch(Harmony harmony, BepInEx.Logging.ManualLogSource logger, string version)
        {
            log = logger;
            pluginVersion = version;
            harmony.Patch(
                AccessTools.Method(typeof(ZNet), nameof(ZNet.Connect), new[] { typeof(string) }),
                prefix: new HarmonyMethod(typeof(Report), nameof(OnConnect)));
            harmony.Patch(
                AccessTools.Method(typeof(ZNet), nameof(ZNet.Disconnect), new[] { typeof(ZNetPeer) }),
                prefix: new HarmonyMethod(typeof(Report), nameof(OnDisconnect)));
            harmony.Patch(
                AccessTools.Method(typeof(ZNet), nameof(ZNet.RPC_Error)),
                postfix: new HarmonyMethod(typeof(Report), nameof(OnError)));
        }

        // A click on a join button starts a new trail.
        public static void Begin(JoinPlugin.JoinServer server)
        {
            trail.Clear();
            started = Time.realtimeSinceStartup;
            inWorld = false;
            Step($"join {server.name} at {server.address}");
        }

        public static void Step(string text)
        {
            float since = started < 0f ? 0f : Time.realtimeSinceStartup - started;
            trail.Add($"{DateTime.UtcNow:HH:mm:ss.fff} +{since:0.0}s {text}");
            if (trail.Count > TrailLines)
            {
                trail.RemoveAt(0);
            }
        }

        // Called every frame. The game changes its connection status in many
        // places, a watch catches them all.
        public static void Tick()
        {
            ZNet.ConnectionStatus status = ZNet.GetConnectionStatus();
            if (status != lastStatus)
            {
                Step($"status {lastStatus} -> {status}");
                lastStatus = status;
            }
            if (!inWorld && status == ZNet.ConnectionStatus.Connected && Player.m_localPlayer != null)
            {
                inWorld = true;
                Step("in the world");
            }
        }

        private static void OnConnect(string remotePlayerId)
        {
            Step($"connect to playfab host {remotePlayerId}");
        }

        private static void OnDisconnect(ZNet __instance, ZNetPeer peer)
        {
            if (!__instance.IsServer() && peer != null)
            {
                bool connected = peer.m_rpc != null && peer.m_rpc.IsConnected();
                Step($"disconnect {peer.m_socket?.GetEndPointString()}, socket connected {connected}");
            }
        }

        private static void OnError(int error)
        {
            Step($"the server sent error {(ZNet.ConnectionStatus)error}");
        }

        // Sends one report. The status is a ZNet.ConnectionStatus or one of
        // the plugin's own, the message is what the player saw.
        public static void Failed(JoinPlugin.JoinServer server, string status, string message)
        {
            string key = status + message;
            if (key == lastSent && Time.realtimeSinceStartup - lastSentAt < RepeatSeconds)
            {
                return;
            }
            lastSent = key;
            lastSentAt = Time.realtimeSinceStartup;
            Step($"failed {status}: {message}");
            Body body = new Body
            {
                server_id = server?.id ?? "",
                server = server?.name ?? "",
                status = status,
                message = message ?? "",
                in_world = inWorld,
                seconds = started < 0f ? 0f : Time.realtimeSinceStartup - started,
                game_version = Version.GetVersionString(),
                plugin_version = pluginVersion,
                os = SystemInfo.operatingSystem,
                trail = new List<string>(trail),
            };
            log.LogInfo($"connection report: {body.server} {status} after {body.seconds:0.0}s, {body.message}");
            // The next join starts clean, an in world drop is already known.
            inWorld = false;
            JoinPlugin.Run(Send(body));
        }

        private static IEnumerator Send(Body body)
        {
            // BepInEx writes its log file on a timer, a frame lets the last
            // lines in.
            yield return null;
            body.log = Tail("BepInEx/LogOutput.log", Path.Combine(Paths.BepInExRootPath, "LogOutput.log"), BepInExTail)
                + Tail("Player.log", Application.consoleLogPath, PlayerTail);
            string url = JoinPlugin.BridgeUrl(Door);
            if (url == null)
            {
                log.LogInfo("the connection report is not sent, the game was not started by Blackforge");
                yield break;
            }
            byte[] bytes = Encoding.UTF8.GetBytes(JsonConvert.SerializeObject(body));
            using (UnityWebRequest request = new UnityWebRequest(url, "POST"))
            {
                request.uploadHandler = new UploadHandlerRaw(bytes);
                request.downloadHandler = new DownloadHandlerBuffer();
                request.SetRequestHeader("Content-Type", "application/json");
                JoinPlugin.SignRequest(request);
                request.timeout = SendTimeout;
                yield return request.SendWebRequest();
                if (request.result != UnityWebRequest.Result.Success)
                {
                    log.LogWarning($"the connection report did not reach the app: {request.responseCode} {request.error} {request.downloadHandler.text}");
                }
            }
        }

        // The newest part of a log file, cut at a line start. The game and
        // BepInEx still write the files, so they open shared.
        private static string Tail(string label, string path, int bytes)
        {
            try
            {
                if (string.IsNullOrEmpty(path) || !File.Exists(path))
                {
                    return $"==== {label} missing at {path}\n";
                }
                using (FileStream file = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete))
                {
                    long start = Math.Max(0, file.Length - bytes);
                    file.Seek(start, SeekOrigin.Begin);
                    byte[] buffer = new byte[file.Length - start];
                    int read = 0;
                    while (read < buffer.Length)
                    {
                        int got = file.Read(buffer, read, buffer.Length - read);
                        if (got <= 0)
                        {
                            break;
                        }
                        read += got;
                    }
                    string text = Encoding.UTF8.GetString(buffer, 0, read);
                    int line = text.IndexOf('\n');
                    if (start > 0 && line >= 0)
                    {
                        text = text.Substring(line + 1);
                    }
                    return $"==== {label}\n{text}\n";
                }
            }
            catch (Exception error)
            {
                return $"==== {label} did not read: {error.Message}\n";
            }
        }
    }
}
