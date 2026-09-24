using System;
using System.Collections;
using System.Collections.Generic;
using System.IO;
using BepInEx;
using HarmonyLib;
using Newtonsoft.Json;
using TMPro;
using UnityEngine;
using UnityEngine.Networking;
using UnityEngine.UI;
using Object = UnityEngine.Object;

namespace Blackforge
{
    // Main menu buttons that join the servers in servers.json, which the app
    // writes next to this dll before every start. A click first asks the
    // running app for a one time code. The app is signed in with Google, and the
    // servers let in nobody without a code, see docs/gate.md. The code goes into
    // the invite key, which the game sends to the server in its handshake. With
    // crossplay the servers share one public address, the port in it is only a
    // lobby label, so a button finds its PlayFab lobby by the address and the
    // server name. Then it queues a join to that host, the same queue a Steam
    // invite fills, and the game shows character select.
    [BepInPlugin("xyz.vladas.blackforge.join", "Blackforge Join", "3.0.0")]
    public class JoinPlugin : BaseUnityPlugin
    {
        private const string ListFile = "servers.json";
        private static readonly Vector2 Size = new Vector2(360f, 80f);
        private static readonly Vector2 Margin = new Vector2(40f, -40f);
        private const float Gap = 16f;
        private const float FontSize = 36f;
        // Room left and right of the label inside the frame.
        private const float TextPadding = 80f;
        // The start arguments the app adds, see join.rs in blackforge-core.
        private const string PortArg = "-blackforge-bridge";
        private const string KeyArg = "-blackforge-key";
        private const string KeyHeader = "X-Blackforge-Key";
        private const int AskTimeout = 20;
        // The server plugin sends its reason for a refusal in this rpc.
        private const string GateRpc = "BlackforgeGate";
        private const string NotFromApp = "Start Valheim from Blackforge to join this server.";
        private const string AppClosed = "Blackforge is not running. Keep it open while you play, it lets you in.";

        public class JoinList
        {
            public List<JoinServer> servers = new List<JoinServer>();
        }

        public class JoinServer
        {
            public string id;
            public string name;
            public string address;
        }

        private static BepInEx.Logging.ManualLogSource log;
        private static List<JoinServer> servers = new List<JoinServer>();
        private static JoinPlugin instance;
        private static string bridgePort;
        private static string bridgeKey;
        private static bool asking;
        // The reason the server gave for the last refusal, shown in place of
        // the game's own error text.
        private static string gateMessage;

        private void Awake()
        {
            log = Logger;
            instance = this;
            ReadBridge();
            servers = ReadList(Path.Combine(Path.GetDirectoryName(Info.Location), ListFile));
            if (servers.Count == 0)
            {
                log.LogWarning("servers.json lists no server, no join button is added");
                return;
            }
            Harmony harmony = new Harmony(Info.Metadata.GUID);
            harmony.Patch(
                AccessTools.Method(typeof(FejdStartup), nameof(FejdStartup.SetupGui)),
                postfix: new HarmonyMethod(typeof(JoinPlugin), nameof(AddButtons)));
            harmony.Patch(
                AccessTools.Method(typeof(ZNet), nameof(ZNet.OnNewConnection)),
                postfix: new HarmonyMethod(typeof(JoinPlugin), nameof(ListenToGate)));
            harmony.Patch(
                AccessTools.Method(typeof(FejdStartup), nameof(FejdStartup.ShowConnectError)),
                postfix: new HarmonyMethod(typeof(JoinPlugin), nameof(ShowGateMessage)));
        }

        private static void ReadBridge()
        {
            string[] args = Environment.GetCommandLineArgs();
            for (int i = 0; i + 1 < args.Length; i++)
            {
                if (args[i] == PortArg)
                {
                    bridgePort = args[i + 1];
                }
                else if (args[i] == KeyArg)
                {
                    bridgeKey = args[i + 1];
                }
            }
            if (bridgePort == null || bridgeKey == null)
            {
                log.LogWarning("the game was not started by Blackforge, a join button cannot get a code");
            }
        }

        private static void ListenToGate(ZNet __instance, ZNetPeer peer)
        {
            if (!__instance.IsServer())
            {
                peer.m_rpc.Register<string>(GateRpc, (rpc, text) => gateMessage = text);
            }
        }

        private static void ShowGateMessage(FejdStartup __instance)
        {
            if (gateMessage != null)
            {
                __instance.m_connectionFailedError.text = gateMessage;
                gateMessage = null;
            }
        }

        private static List<JoinServer> ReadList(string path)
        {
            List<JoinServer> found = new List<JoinServer>();
            if (!File.Exists(path))
            {
                return found;
            }
            try
            {
                // Not Unity's JsonUtility, in a plugin it gave back an empty
                // list without an error. The game ships Newtonsoft.
                JoinList list = JsonConvert.DeserializeObject<JoinList>(File.ReadAllText(path));
                if (list?.servers == null)
                {
                    return found;
                }
                foreach (JoinServer server in list.servers)
                {
                    if (!string.IsNullOrEmpty(server?.name) && !string.IsNullOrEmpty(server.address))
                    {
                        found.Add(server);
                    }
                }
            }
            catch (Exception error)
            {
                log.LogWarning($"{path} did not read: {error.Message}");
            }
            return found;
        }

        private static void AddButtons(FejdStartup __instance)
        {
            // Back in the menu, the code of an earlier join must not go to
            // another server.
            ZNet.SetInviteSecretKey("");
            // The menu entries are bare text, the character select Start button
            // has a frame, so that one is the model.
            Button template = __instance.m_csStartButton;
            if (template == null)
            {
                log.LogWarning("the character select has no start button, no join button is added");
                return;
            }
            // Every button takes the width of the longest label, so a long
            // server name is not cut and the stack stays even.
            List<RectTransform> rects = new List<RectTransform>();
            float width = Size.x;
            for (int i = 0; i < servers.Count; i++)
            {
                TMP_Text text = AddButton(__instance, template, servers[i], i, rects);
                width = Mathf.Max(width, text.GetPreferredValues(text.text).x + TextPadding);
            }
            foreach (RectTransform rect in rects)
            {
                rect.sizeDelta = new Vector2(width, Size.y);
            }
        }

        private static TMP_Text AddButton(
            FejdStartup menu, Button template, JoinServer server, int index, List<RectTransform> rects)
        {
            GameObject copy = Object.Instantiate(template.gameObject, menu.m_mainMenu.transform, false);
            copy.name = "BlackforgeJoin" + index;
            copy.SetActive(true);

            // The Start button answers a gamepad key, the copy would steal it in the menu.
            foreach (UIGamePad pad in copy.GetComponentsInChildren<UIGamePad>(true))
            {
                if (pad.m_hint != null)
                {
                    Object.Destroy(pad.m_hint);
                }
                Object.Destroy(pad);
            }
            foreach (LayoutElement element in copy.GetComponentsInChildren<LayoutElement>(true))
            {
                element.ignoreLayout = true;
            }

            RectTransform rect = copy.GetComponent<RectTransform>();
            rect.anchorMin = new Vector2(0f, 1f);
            rect.anchorMax = new Vector2(0f, 1f);
            rect.pivot = new Vector2(0f, 1f);
            rect.anchoredPosition = Margin + new Vector2(0f, -(Size.y + Gap) * index);
            rect.sizeDelta = Size;
            rect.localScale = Vector3.one;
            rects.Add(rect);

            TMP_Text text = copy.GetComponentInChildren<TMP_Text>(true);
            text.text = "Join " + server.name;
            text.enableAutoSizing = false;
            text.fontSize = FontSize;

            Button button = copy.GetComponent<Button>();
            button.interactable = true;
            // A new event drops the click handlers the copy brought from the template.
            button.onClick = new Button.ButtonClickedEvent();
            button.onClick.AddListener(() => Join(menu, server));
            return text;
        }

        private static void Join(FejdStartup menu, JoinServer server)
        {
            if (!PlayFabManager.IsLoggedIn)
            {
                // The game's own popup waits for the login, then the join runs.
                menu.ContinueWhenLoggedInPopup(() => Join(menu, server));
                return;
            }
            if (bridgePort == null || bridgeKey == null)
            {
                Warn(server, NotFromApp);
                return;
            }
            if (asking)
            {
                return;
            }
            instance.StartCoroutine(AskAndJoin(server));
        }

        // The app gets a one time code from blackforge for this server. The
        // code lives two minutes, the join below uses it right away.
        private static IEnumerator AskAndJoin(JoinServer server)
        {
            asking = true;
            string url = $"http://127.0.0.1:{bridgePort}/join/{UnityWebRequest.EscapeURL(server.id)}";
            using (UnityWebRequest request = new UnityWebRequest(url, "POST"))
            {
                request.downloadHandler = new DownloadHandlerBuffer();
                request.SetRequestHeader(KeyHeader, bridgeKey);
                request.timeout = AskTimeout;
                yield return request.SendWebRequest();
                asking = false;

                if (request.result == UnityWebRequest.Result.ConnectionError)
                {
                    log.LogInfo($"no answer from the app for {server.name}: {request.error}");
                    Warn(server, AppClosed);
                    yield break;
                }
                string text = request.downloadHandler.text;
                if (request.responseCode != 200 || string.IsNullOrEmpty(text))
                {
                    log.LogInfo($"no code for {server.name}: {request.responseCode} {text}");
                    Warn(server, string.IsNullOrEmpty(text) ? AppClosed : text);
                    yield break;
                }
                ZNet.SetInviteSecretKey(text);
            }
            FindAndJoin(server);
        }

        private static void FindAndJoin(JoinServer server)
        {
            string filter =
                $"{ZPlayFabMatchmaking.ServerIpSearchKey} eq '{Quote(server.address)}'"
                + $" and {ZPlayFabMatchmaking.ServerNameSearchKey} eq '{Quote(server.name)}'"
                + $" and {ZPlayFabMatchmaking.IsActiveSearchKey} eq '{true}'";
            ZPlayFabMatchmaking.FindHostSession(
                filter,
                found =>
                {
                    if (string.IsNullOrEmpty(found?.remotePlayerId))
                    {
                        NotOnline(server, "the lobby has no host");
                        return;
                    }
                    ZSteamMatchmaking.instance.m_joinData =
                        new ServerJoinData(new ServerJoinDataPlayFabUser(found.remotePlayerId));
                },
                reason => NotOnline(server, reason.ToString()),
                false);
        }

        // Search values sit in single quotes, a quote inside is doubled.
        private static string Quote(string value)
        {
            return value.Replace("'", "''");
        }

        private static void NotOnline(JoinServer server, string reason)
        {
            log.LogInfo($"no lobby for {server.name} at {server.address}: {reason}");
            ZNet.SetInviteSecretKey("");
            Warn(server, server.name + " is not online");
        }

        private static void Warn(JoinServer server, string text)
        {
            UnifiedPopup.Push(new WarningPopup(
                server.name,
                text,
                () => UnifiedPopup.Pop(),
                localizeText: false));
        }
    }
}
