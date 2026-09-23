using System;
using System.Collections.Generic;
using System.IO;
using BepInEx;
using HarmonyLib;
using Newtonsoft.Json;
using TMPro;
using UnityEngine;
using UnityEngine.UI;
using Object = UnityEngine.Object;

namespace Blackforge
{
    // Main menu buttons that join the servers in servers.json, which the app
    // writes next to this dll before every start. With crossplay the servers
    // share one public address, the port in it is only a lobby label, so a
    // button finds its PlayFab lobby by the address and the server name. Then it
    // queues a join to that host, the same queue a Steam invite fills, and the
    // game shows character select and asks for the password itself.
    [BepInPlugin("xyz.vladas.blackforge.join", "Blackforge Join", "2.0.0")]
    public class JoinPlugin : BaseUnityPlugin
    {
        private const string ListFile = "servers.json";
        private static readonly Vector2 Size = new Vector2(360f, 80f);
        private static readonly Vector2 Margin = new Vector2(40f, -40f);
        private const float Gap = 16f;
        private const float FontSize = 36f;
        // Room left and right of the label inside the frame.
        private const float TextPadding = 80f;

        public class JoinList
        {
            public List<JoinServer> servers = new List<JoinServer>();
        }

        public class JoinServer
        {
            public string name;
            public string address;
        }

        private static BepInEx.Logging.ManualLogSource log;
        private static List<JoinServer> servers = new List<JoinServer>();

        private void Awake()
        {
            log = Logger;
            servers = ReadList(Path.Combine(Path.GetDirectoryName(Info.Location), ListFile));
            if (servers.Count == 0)
            {
                log.LogWarning("servers.json lists no server, no join button is added");
                return;
            }
            new Harmony(Info.Metadata.GUID).Patch(
                AccessTools.Method(typeof(FejdStartup), nameof(FejdStartup.SetupGui)),
                postfix: new HarmonyMethod(typeof(JoinPlugin), nameof(AddButtons)));
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
            UnifiedPopup.Push(new WarningPopup(
                server.name,
                server.name + " is not online",
                () => UnifiedPopup.Pop(),
                localizeText: false));
        }
    }
}
