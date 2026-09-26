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
        // Reports the app did not take wait here, under the BepInEx folder.
        private const string UnsentFolder = "blackforge-reports";
        private const int KeepFiles = 50;

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
            public string character = "";
            public string playfab_id = "";
            public string platform_id = "";
            public List<string> mods = new List<string>();
            public List<string> trail = new List<string>();
            public List<string> traffic = new List<string>();
            public string log = "";
        }

        private static BepInEx.Logging.ManualLogSource log;
        private static string pluginVersion;
        private static readonly List<string> trail = new List<string>();
        private static float started = -1f;
        private static ZNet.ConnectionStatus lastStatus = ZNet.ConnectionStatus.None;
        private static bool inWorld;
        // The character of the last join, from character select or the world.
        private static string character = "";
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
                character = Player.m_localPlayer.GetPlayerName() ?? character;
                Step($"in the world as {character}");
            }
        }

        private static void OnConnect(string remotePlayerId)
        {
            // The menu is gone at the connect, the game holds the profile.
            character = Game.instance?.GetPlayerProfile()?.GetName() ?? "";
            Step($"character {character}");
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
                character = character,
                playfab_id = PlayFabId(),
                platform_id = PlatformId(),
                mods = Mods(),
                trail = new List<string>(trail),
                traffic = Relay.Traffic(),
            };
            log.LogInfo($"connection report: {body.server} {body.character} {status} after {body.seconds:0.0}s, {body.message}");
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
            string json = JsonConvert.SerializeObject(body);
            bool sent = false;
            yield return Post(json, ok => sent = ok);
            if (!sent)
            {
                Keep(json);
                yield break;
            }
            yield return SendUnsent();
        }

        // Reports the app did not take wait on disk and go out with the next
        // report that gets through, or at the next start from the app.
        public static IEnumerator SendUnsent()
        {
            string[] files;
            try
            {
                files = Directory.Exists(UnsentDir) ? Directory.GetFiles(UnsentDir, "*.json") : new string[0];
            }
            catch (Exception error)
            {
                log.LogWarning($"the unsent reports did not list: {error.Message}");
                yield break;
            }
            Array.Sort(files);
            foreach (string file in files)
            {
                string json;
                try
                {
                    json = File.ReadAllText(file);
                }
                catch (Exception error)
                {
                    log.LogWarning($"{file} did not read: {error.Message}");
                    continue;
                }
                bool sent = false;
                yield return Post(json, ok => sent = ok);
                if (!sent)
                {
                    yield break;
                }
                try
                {
                    File.Delete(file);
                    log.LogInfo($"sent the kept report {Path.GetFileName(file)}");
                }
                catch (Exception error)
                {
                    log.LogWarning($"{file} was sent but not deleted: {error.Message}");
                }
            }
        }

        private static string UnsentDir => Path.Combine(Paths.BepInExRootPath, UnsentFolder);

        private static void Keep(string json)
        {
            try
            {
                Directory.CreateDirectory(UnsentDir);
                string[] kept = Directory.GetFiles(UnsentDir, "*.json");
                Array.Sort(kept);
                for (int i = 0; i <= kept.Length - KeepFiles; i++)
                {
                    File.Delete(kept[i]);
                }
                string file = Path.Combine(UnsentDir, $"{DateTime.UtcNow:yyyyMMdd-HHmmss-fff}.json");
                File.WriteAllText(file, json);
                log.LogInfo($"the connection report waits in {file}");
            }
            catch (Exception error)
            {
                log.LogWarning($"the connection report is lost, it did not save: {error.Message}");
            }
        }

        private static IEnumerator Post(string json, Action<bool> done)
        {
            string url = JoinPlugin.BridgeUrl(Door);
            if (url == null)
            {
                log.LogInfo("the connection report waits, the game was not started by Blackforge");
                done(false);
                yield break;
            }
            using (UnityWebRequest request = new UnityWebRequest(url, "POST"))
            {
                request.uploadHandler = new UploadHandlerRaw(Encoding.UTF8.GetBytes(json));
                request.downloadHandler = new DownloadHandlerBuffer();
                request.SetRequestHeader("Content-Type", "application/json");
                JoinPlugin.SignRequest(request);
                request.timeout = SendTimeout;
                yield return request.SendWebRequest();
                bool ok = request.result == UnityWebRequest.Result.Success;
                if (!ok)
                {
                    log.LogWarning($"the connection report did not reach the app: {request.responseCode} {request.error} {request.downloadHandler.text}");
                }
                done(ok);
            }
        }

        private static string PlayFabId()
        {
            try
            {
                return PlayFab.Party.PlayFabMultiplayerManager.Get()?.LocalPlayer?.EntityKey?.Id ?? "";
            }
            catch (Exception error)
            {
                return "unreadable: " + error.Message;
            }
        }

        private static string PlatformId()
        {
            try
            {
                return Splatform.PlatformManager.DistributionPlatform?.LocalUser?.PlatformUserID.ToString() ?? "";
            }
            catch (Exception error)
            {
                return "unreadable: " + error.Message;
            }
        }

        // Every plugin BepInEx loaded, with its version.
        private static List<string> Mods()
        {
            List<string> mods = new List<string>();
            foreach (BepInEx.PluginInfo plugin in BepInEx.Bootstrap.Chainloader.PluginInfos.Values)
            {
                mods.Add($"{plugin.Metadata.GUID} {plugin.Metadata.Name} {plugin.Metadata.Version}");
            }
            mods.Sort();
            return mods;
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
