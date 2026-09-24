using System;
using System.Collections;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Text;
using BepInEx;
using HarmonyLib;
using Newtonsoft.Json;
using UnityEngine;
using UnityEngine.Networking;

namespace Blackforge
{
    // Runs on the dedicated server only. It is the gate of the server and it
    // writes who is on.
    //
    // The gate: a player gets in only with a one time code from blackforge, see
    // docs/gate.md. The join plugin of the app puts the code into the invite
    // key of the game, which the game sends in its handshake. The plugin trades
    // the code at the backend for the username behind it, then lets the player
    // skip the password. Anybody else gets refused. The server keeps a random
    // password nobody knows, so a server without this plugin lets nobody in,
    // and a plugin that cannot patch the game or has no gate settings stops
    // the server.
    //
    // The status: every 2 seconds the connected peers go to the file named by
    // BLACKFORGE_STATUS_FILE, the one exact and fast answer to who is on. A
    // file older than a few seconds means the server is not running or not
    // ready, and a reader must treat it as unknown.
    [BepInPlugin("xyz.vladas.blackforge.status", "Blackforge Server", "2.0.0")]
    public class StatusPlugin : BaseUnityPlugin
    {
        private const float Interval = 2f;
        private const int VerifyTimeout = 10;
        // The client plugin shows this text in place of the game's own error.
        private const string GateRpc = "BlackforgeGate";
        private const string NoCode = "This server has no password. Join it with the button Blackforge adds to the main menu.";
        private const string Refused = "Blackforge did not let you in. Ask the owner of the server to add you as a member.";
        private const string Unreachable = "Blackforge is not reachable, try again later.";

        private class VerifyBody
        {
            public string server;
            public string code;
        }

        private class Verified
        {
            public string username { get; set; }
        }

        private static StatusPlugin instance;
        private static string gateUrl;
        private static string gateSecret;
        // Peers whose code was good and who still have to send their peer info.
        private static readonly Dictionary<ZNetPeer, string> pending = new Dictionary<ZNetPeer, string>();
        // The blackforge username of every peer that got in.
        private static readonly Dictionary<ZNetPeer, string> usernames = new Dictionary<ZNetPeer, string>();

        private string path;
        private float next;

        private void Awake()
        {
            instance = this;
            path = Environment.GetEnvironmentVariable("BLACKFORGE_STATUS_FILE");
            if (string.IsNullOrEmpty(path))
            {
                Logger.LogWarning("BLACKFORGE_STATUS_FILE is not set, no status is written");
            }
            gateUrl = Environment.GetEnvironmentVariable("BLACKFORGE_GATE_URL")?.TrimEnd('/');
            gateSecret = Environment.GetEnvironmentVariable("BLACKFORGE_GATE_SECRET");
            if (string.IsNullOrEmpty(gateUrl) || string.IsNullOrEmpty(gateSecret))
            {
                Stop("BLACKFORGE_GATE_URL or BLACKFORGE_GATE_SECRET is not set");
                return;
            }
            try
            {
                Harmony harmony = new Harmony(Info.Metadata.GUID);
                harmony.Patch(
                    Method(typeof(ZNet), nameof(ZNet.RPC_ServerHandshake)),
                    prefix: new HarmonyMethod(typeof(StatusPlugin), nameof(Handshake)));
                harmony.Patch(
                    Method(typeof(ZNet), nameof(ZNet.RPC_PeerInfo)),
                    prefix: new HarmonyMethod(typeof(StatusPlugin), nameof(PeerInfoPrefix)),
                    finalizer: new HarmonyMethod(typeof(StatusPlugin), nameof(PeerInfoFinalizer)));
            }
            catch (Exception error)
            {
                Stop($"the gate did not patch the game: {error}");
                return;
            }
            Logger.LogInfo($"the gate asks {gateUrl} about every join");
        }

        private static MethodInfo Method(Type type, string name)
        {
            MethodInfo method = AccessTools.Method(type, name);
            if (method == null)
            {
                throw new MissingMethodException(type.Name, name);
            }
            return method;
        }

        // A server without a working gate must not run, it would let in
        // whoever the password lets in.
        private void Stop(string reason)
        {
            Logger.LogError($"{reason}, the server stops");
            Application.Quit(1);
        }

        private static bool Handshake(ZNet __instance, ZRpc rpc, string secretKey)
        {
            ZNetPeer peer = __instance.GetPeer(rpc);
            if (peer == null)
            {
                return false;
            }
            __instance.ClearPlayerData(peer);
            if (string.IsNullOrEmpty(secretKey))
            {
                instance.Logger.LogInfo($"{peer.m_socket.GetEndPointString()} came without a code");
                Refuse(rpc, NoCode, ZNet.ConnectionStatus.ErrorPassword);
                return false;
            }
            instance.StartCoroutine(Verify(__instance, peer, rpc, secretKey));
            return false;
        }

        private static IEnumerator Verify(ZNet net, ZNetPeer peer, ZRpc rpc, string code)
        {
            string body = JsonConvert.SerializeObject(new VerifyBody { server = ZNet.m_ServerName, code = code });
            using (UnityWebRequest request = new UnityWebRequest(gateUrl + "/api/gate/verify", "POST"))
            {
                request.uploadHandler = new UploadHandlerRaw(Encoding.UTF8.GetBytes(body));
                request.downloadHandler = new DownloadHandlerBuffer();
                request.SetRequestHeader("Content-Type", "application/json");
                request.SetRequestHeader("Authorization", "Bearer " + gateSecret);
                request.timeout = VerifyTimeout;
                yield return request.SendWebRequest();

                if (!net.m_peers.Contains(peer))
                {
                    yield break;
                }
                string where = peer.m_socket.GetEndPointString();
                if (request.result == UnityWebRequest.Result.Success && request.responseCode == 200)
                {
                    string username = JsonConvert.DeserializeObject<Verified>(request.downloadHandler.text)?.username;
                    if (string.IsNullOrEmpty(username))
                    {
                        instance.Logger.LogWarning($"the gate answered no username for {where}");
                        Refuse(rpc, Unreachable, ZNet.ConnectionStatus.ErrorConnectFailed);
                        yield break;
                    }
                    instance.Logger.LogInfo($"{username} is let in from {where}");
                    pending[peer] = username;
                    rpc.Invoke("ClientHandshake", false, ZNet.ServerPasswordSalt());
                }
                else if (request.responseCode == 403)
                {
                    instance.Logger.LogInfo($"the gate refused the code of {where}");
                    Refuse(rpc, Refused, ZNet.ConnectionStatus.ErrorPassword);
                }
                else
                {
                    instance.Logger.LogWarning($"the gate did not answer for {where}: {request.responseCode} {request.error}");
                    Refuse(rpc, Unreachable, ZNet.ConnectionStatus.ErrorConnectFailed);
                }
            }
        }

        private static void Refuse(ZRpc rpc, string text, ZNet.ConnectionStatus status)
        {
            rpc.Invoke(GateRpc, text);
            rpc.Invoke("Error", (int)status);
        }

        // A player the gate let in sent no password, the game compares it with
        // the server's. For the one peer info of that player the server has
        // none.
        private static void PeerInfoPrefix(ZNet __instance, ZRpc rpc, out string __state)
        {
            __state = null;
            ZNetPeer peer = __instance.GetPeer(rpc);
            if (peer == null || !pending.TryGetValue(peer, out string username))
            {
                return;
            }
            pending.Remove(peer);
            usernames[peer] = username;
            __state = ZNet.m_serverPassword;
            ZNet.m_serverPassword = "";
        }

        private static Exception PeerInfoFinalizer(Exception __exception, string __state)
        {
            if (__state != null)
            {
                ZNet.m_serverPassword = __state;
            }
            return __exception;
        }

        private void Update()
        {
            if (Time.unscaledTime < next)
            {
                return;
            }
            next = Time.unscaledTime + Interval;
            ZNet net = ZNet.instance;
            if (net == null || !net.IsServer() || !net.IsDedicated())
            {
                return;
            }
            Forget(net);
            if (string.IsNullOrEmpty(path))
            {
                return;
            }
            try
            {
                Write(net);
            }
            catch (Exception error)
            {
                Logger.LogWarning($"{path} was not written: {error.Message}");
            }
        }

        // A peer that left is dropped from both lists.
        private static void Forget(ZNet net)
        {
            foreach (ZNetPeer gone in pending.Keys.Concat(usernames.Keys).Where(peer => !net.m_peers.Contains(peer)).ToList())
            {
                pending.Remove(gone);
                usernames.Remove(gone);
            }
        }

        private void Write(ZNet net)
        {
            StringBuilder json = new StringBuilder();
            json.Append("{\"updated\":").Append(DateTimeOffset.UtcNow.ToUnixTimeSeconds());
            json.Append(",\"players\":[");
            bool first = true;
            // Every connected peer counts, also one that is still in the
            // password prompt or character select. For a restart it is a player.
            foreach (ZNetPeer peer in net.GetConnectedPeers())
            {
                if (!first)
                {
                    json.Append(',');
                }
                first = false;
                json.Append("{\"name\":\"").Append(Escape(peer.m_playerName));
                json.Append("\",\"id\":\"").Append(peer.m_uid).Append('"');
                if (usernames.TryGetValue(peer, out string username))
                {
                    json.Append(",\"user\":\"").Append(Escape(username)).Append('"');
                }
                json.Append('}');
            }
            json.Append("]}");

            // A reader never sees half a file, the new one replaces the old in one step.
            string temp = path + ".tmp";
            File.WriteAllText(temp, json.ToString());
            if (File.Exists(path))
            {
                File.Replace(temp, path, null);
            }
            else
            {
                File.Move(temp, path);
            }
        }

        private static string Escape(string value)
        {
            StringBuilder escaped = new StringBuilder();
            foreach (char c in value ?? "")
            {
                if (c == '"' || c == '\\')
                {
                    escaped.Append('\\').Append(c);
                }
                else if (c < ' ')
                {
                    escaped.Append(' ');
                }
                else
                {
                    escaped.Append(c);
                }
            }
            return escaped.ToString();
        }
    }
}
