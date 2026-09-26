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
    // running app for the member check and the rules of the server, and Start
    // in character select asks it for a one time code. The app is signed in with
    // Google, and the servers let in nobody without a code, see docs/gate.md.
    // The code goes into the invite key, which the game sends to the server in
    // its handshake. With crossplay the servers share one public address, the
    // port in it is only a lobby label, so a button finds its PlayFab lobby by
    // the address and the server name. Then it queues a join to that host, the
    // same queue a Steam invite fills, and the game shows character select.
    [BepInPlugin("xyz.vladas.blackforge.join", "Blackforge Join", "4.2.0")]
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
        // The doors of the app, see bridge.rs in the blackforge crate.
        private const string RulesDoor = "rules";
        private const string JoinDoor = "join";
        private const int AskTimeout = 20;
        // The server plugin sends its reason for a refusal in this rpc.
        private const string GateRpc = "BlackforgeGate";
        private const string NotFromApp = "Start Valheim from Blackforge to join this server.";
        private const string AppClosed = "Blackforge is not running. Keep it open while you play, it lets you in.";
        // Conditional Config Sync writes this when the server never answered
        // its mod check. The servers run the same mods, so the real cause is a
        // join right after a drop. The server still holds the dropped
        // connection for 90 seconds, a new join restarts that wait, and the
        // server answers down the dead connection.
        private const string NoServerHandshake = "No version handshake was received from the server";
        private const string StuckConnection =
            "{0} did not answer. It still holds your last connection after a drop. Wait 2 minutes, then join again.";

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

        // The answer at the join door, `JoinCode` of blackforge-api.
        public class JoinAnswer
        {
            public string code;
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
        // The server of the last click on a join button.
        private static JoinServer joining;

        private void Awake()
        {
            log = Logger;
            instance = this;
            ReadBridge();
            Harmony harmony = new Harmony(Info.Metadata.GUID);
            // The tags must hold in every world, also without any server.
            Competitive.Patch(harmony, log);
            // A failed patch of the reports must not cost the join buttons.
            try
            {
                Report.Patch(harmony, log, Info.Metadata.Version.ToString());
                Relay.Patch(harmony, log);
            }
            catch (Exception error)
            {
                log.LogError($"connection reports are off, a patch failed: {error}");
            }
            // Reports that waited for the app go out with this start.
            if (bridgePort != null)
            {
                StartCoroutine(Report.SendUnsent());
            }
            harmony.Patch(
                AccessTools.Method(typeof(ZNet), nameof(ZNet.OnNewConnection)),
                postfix: new HarmonyMethod(typeof(JoinPlugin), nameof(ListenToGate)));
            // Last, so the other mods that write into the same error text are done.
            harmony.Patch(
                AccessTools.Method(typeof(FejdStartup), nameof(FejdStartup.ShowConnectError)),
                postfix: new HarmonyMethod(typeof(JoinPlugin), nameof(ShowGateMessage)) { priority = Priority.Last });
            servers = ReadList(Path.Combine(Path.GetDirectoryName(Info.Location), ListFile));
            if (servers.Count == 0)
            {
                log.LogWarning("servers.json lists no server, no join button is added");
                return;
            }
            harmony.Patch(
                AccessTools.Method(typeof(FejdStartup), nameof(FejdStartup.SetupGui)),
                postfix: new HarmonyMethod(typeof(JoinPlugin), nameof(AddButtons)));
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
                Report.Step($"socket to {peer.m_socket?.GetEndPointString()} is open");
                peer.m_rpc.Register<string>(GateRpc, (rpc, text) =>
                {
                    Report.Step($"the gate says: {text}");
                    gateMessage = text;
                });
                Competitive.Listen(peer);
            }
        }

        private void Update()
        {
            Competitive.Tick();
            Report.Tick();
        }

        public static void Run(IEnumerator routine)
        {
            instance.StartCoroutine(routine);
        }

        // The url of a door of the app, null for a game the app did not start.
        public static string BridgeUrl(string path)
        {
            return bridgePort == null ? null : $"http://127.0.0.1:{bridgePort}/{path}";
        }

        public static void SignRequest(UnityWebRequest request)
        {
            request.SetRequestHeader(KeyHeader, bridgeKey ?? "");
        }

        private static void ShowGateMessage(FejdStartup __instance, ZNet.ConnectionStatus statusOverride)
        {
            ZNet.ConnectionStatus status =
                statusOverride == ZNet.ConnectionStatus.None ? ZNet.GetConnectionStatus() : statusOverride;
            TMP_Text error = __instance.m_connectionFailedError;
            string text = gateMessage;
            gateMessage = null;
            if (text == null && joining != null && error.text.Contains(NoServerHandshake))
            {
                log.LogInfo($"{joining.name} sent no mod check answer, its old connection of this player is still open");
                text = string.Format(StuckConnection, joining.name);
            }
            if (text != null)
            {
                error.text = text;
                // A mod without a priority can still write after this postfix.
                instance.StartCoroutine(KeepText(error, text));
            }
            // The menu runs this at every load, also after a plain logout.
            if (status > ZNet.ConnectionStatus.Connected)
            {
                Report.Failed(joining, status.ToString(), text ?? error.text);
            }
        }

        private static IEnumerator KeepText(TMP_Text error, string text)
        {
            yield return null;
            error.text = text;
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
            Competitive.Forget();
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
            joining = server;
            Report.Begin(server);
            // The click gets only the member check and the rules. The code
            // comes at Start in character select, see Competitive.HoldStart,
            // since a code lives only two minutes.
            instance.StartCoroutine(Ask<Competitive.Ticket>(RulesDoor, server, ticket =>
            {
                Competitive.Expect(server, ticket);
                FindAndJoin(server);
            }, null));
        }

        // A one time code for the server, asked at Start in character select.
        // It goes into the invite key the game sends in its handshake.
        public static void AskCode(JoinServer server, Action onCode, Action onFail)
        {
            instance.StartCoroutine(Ask<JoinAnswer>(JoinDoor, server, answer =>
            {
                if (string.IsNullOrEmpty(answer.code))
                {
                    Warn(server, AppClosed);
                    onFail?.Invoke();
                    return;
                }
                ZNet.SetInviteSecretKey(answer.code);
                Report.Step("got a join code");
                onCode();
            }, onFail));
        }

        // Asks the running app at one of its doors. A failure shows its
        // reason in a popup and calls onFail.
        private static IEnumerator Ask<T>(string door, JoinServer server, Action<T> onAnswer, Action onFail)
            where T : class
        {
            asking = true;
            string url = BridgeUrl($"{door}/{UnityWebRequest.EscapeURL(server.id)}");
            T answer = null;
            Report.Step($"ask the app at {door}");
            using (UnityWebRequest request = new UnityWebRequest(url, "POST"))
            {
                request.downloadHandler = new DownloadHandlerBuffer();
                SignRequest(request);
                request.timeout = AskTimeout;
                yield return request.SendWebRequest();
                asking = false;

                string text = request.downloadHandler.text;
                if (request.result == UnityWebRequest.Result.ConnectionError)
                {
                    // No report, it would go through the same app.
                    log.LogInfo($"no answer from the app at {door} for {server.name}: {request.error}");
                    Warn(server, AppClosed);
                }
                else if (request.responseCode != 200 || string.IsNullOrEmpty(text))
                {
                    log.LogInfo($"refused at {door} for {server.name}: {request.responseCode} {text}");
                    string shown = string.IsNullOrEmpty(text) ? AppClosed : text;
                    Warn(server, shown);
                    Report.Failed(server, $"Refused{request.responseCode}", shown);
                }
                else
                {
                    try
                    {
                        answer = JsonConvert.DeserializeObject<T>(text);
                    }
                    catch (Exception error)
                    {
                        log.LogWarning($"the app answered nothing usable at {door} for {server.name}: {error.Message}");
                        Warn(server, AppClosed);
                    }
                }
            }
            if (answer == null)
            {
                onFail?.Invoke();
                yield break;
            }
            onAnswer(answer);
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
                    Report.Step($"lobby found, host {found.remotePlayerId}");
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
            Competitive.Forget();
            Warn(server, server.name + " is not online");
            Report.Failed(server, "NotOnline", reason);
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
