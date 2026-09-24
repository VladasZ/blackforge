using System;
using System.Collections.Generic;
using System.Linq;
using HarmonyLib;
using Newtonsoft.Json;
using TMPro;
using UnityEngine;
using UnityEngine.UI;
using Object = UnityEngine.Object;

namespace Blackforge
{
    // The client half of a competitive server, see docs/competitive.md.
    //
    // A join to such a server brings its forbidden materials and its world id.
    // Character select then lists every forbidden item the character carries
    // right of the character and turns Start off. An item carries the world id
    // as its tag when it was found or made in that world, and such an item is
    // allowed there. In the game the server asks for the inventory, and once it
    // cleared the player every new item gets the tag of that world.
    public static class Competitive
    {
        private const string CheckRpc = "BlackforgeCheck";
        private const string InventoryRpc = "BlackforgeInventory";
        private const string ClearedRpc = "BlackforgeCleared";
        private const float TickInterval = 1f;
        // Rows visible at once, more scroll.
        private const int ShownMax = 12;
        private const float Padding = 16f;
        private const float TitleHeight = 64f;
        private const float BarWidth = 8f;
        private const float PanelWidth = 440f;
        private const float RowHeight = 34f;
        private const float IconSize = 30f;
        private const float TitleSize = 24f;
        private const float RowTextSize = 18f;

        // The part of the join answer of the app this file reads.
        public class Ticket
        {
            public bool competitive;
            public string world;
            public List<ForbiddenItem> forbidden = new List<ForbiddenItem>();
            public List<string> allowed = new List<string>();
        }

        // One item of the report to the server, see Competitive.cs of the
        // server plugin.
        private class Carried
        {
            public string n;
            public string t;
        }

        private class Waiting
        {
            public string server;
            public string world;
            public Dictionary<string, ForbiddenItem> forbidden;
        }

        private static BepInEx.Logging.ManualLogSource log;
        // The competitive join that waits in character select.
        private static Waiting waiting;
        private static bool blocked;
        private static GameObject panel;
        // The world of the session that cleared this player, new items get it.
        private static string cleared;
        private static ZRpc server;
        private static float next;
        private static string lastReport;

        public static void Patch(Harmony harmony, BepInEx.Logging.ManualLogSource logger)
        {
            log = logger;
            // Character select opens on the character the menu already shows,
            // without a new preview, so the check runs at both.
            harmony.Patch(
                AccessTools.Method(typeof(FejdStartup), nameof(FejdStartup.ShowCharacterSelection)),
                postfix: new HarmonyMethod(typeof(Competitive), nameof(CheckPreview)));
            harmony.Patch(
                AccessTools.Method(typeof(FejdStartup), nameof(FejdStartup.SetupCharacterPreview)),
                postfix: new HarmonyMethod(typeof(Competitive), nameof(CheckPreview)));
            harmony.Patch(
                AccessTools.Method(typeof(FejdStartup), nameof(FejdStartup.OnCharacterStart)),
                prefix: new HarmonyMethod(typeof(Competitive), nameof(HoldStart)));
            harmony.Patch(
                AccessTools.Method(typeof(FejdStartup), nameof(FejdStartup.OnSelelectCharacterBack)),
                postfix: new HarmonyMethod(typeof(Competitive), nameof(Forget)));
            // Every way an item joins a stack. A stack keeps the tag only when
            // what joins it has the same one, so a tagged stack cannot launder
            // items from another world.
            harmony.Patch(
                AccessTools.Method(typeof(Inventory), nameof(Inventory.AddItem), new[] { typeof(ItemDrop.ItemData) }),
                prefix: new HarmonyMethod(typeof(Competitive), nameof(TaintStacks)));
            harmony.Patch(
                AccessTools.Method(typeof(Inventory), nameof(Inventory.AddItem), new[] { typeof(ItemDrop.ItemData), typeof(Vector2i) }),
                prefix: new HarmonyMethod(typeof(Competitive), nameof(TaintStacks)));
            harmony.Patch(
                AccessTools.Method(
                    typeof(Inventory),
                    nameof(Inventory.AddItem),
                    new[] { typeof(ItemDrop.ItemData), typeof(int), typeof(int), typeof(int), typeof(bool) }),
                prefix: new HarmonyMethod(typeof(Competitive), nameof(TaintSlot)));
        }

        // The join answer of the app arrived, the join goes on in character select.
        public static void Expect(string serverName, Ticket ticket)
        {
            if (ticket == null || !ticket.competitive)
            {
                waiting = null;
                return;
            }
            waiting = new Waiting
            {
                server = serverName,
                world = ticket.world,
                forbidden = Tiers.Expand(ticket.forbidden, ticket.allowed, log.LogWarning),
            };
            log.LogInfo($"{serverName} is competitive, {waiting.forbidden.Count} items are forbidden");
        }

        // Back in the menu, nothing waits any more.
        public static void Forget()
        {
            waiting = null;
            blocked = false;
            if (panel != null)
            {
                Object.Destroy(panel);
                panel = null;
            }
        }

        private static void CheckPreview(FejdStartup __instance)
        {
            if (panel != null)
            {
                Object.Destroy(panel);
                panel = null;
            }
            blocked = false;
            Player player = __instance.GetPreviewPlayer();
            if (waiting == null || !__instance.m_queuedJoinServer.IsValid || player == null)
            {
                __instance.m_csStartButton.interactable = true;
                return;
            }
            List<ItemDrop.ItemData> found = player.GetInventory().GetAllItems()
                .Where(item => IsForbidden(item, waiting.forbidden, waiting.world))
                .ToList();
            blocked = found.Count > 0;
            __instance.m_csStartButton.interactable = !blocked;
            if (blocked)
            {
                panel = Panel(__instance, found);
            }
        }

        // The button is off, this also stops the gamepad key and any mod that
        // clicks it.
        private static bool HoldStart(FejdStartup __instance)
        {
            return !(blocked && __instance.m_queuedJoinServer.IsValid);
        }

        private static bool IsForbidden(ItemDrop.ItemData item, Dictionary<string, ForbiddenItem> forbidden, string world)
        {
            string name = item.m_dropPrefab != null ? item.m_dropPrefab.name : null;
            return name != null && forbidden.ContainsKey(name) && (world == null || Tiers.Tag(item) != world);
        }

        private static GameObject Panel(FejdStartup menu, List<ItemDrop.ItemData> found)
        {
            TMP_Text model = menu.m_csStartButton.GetComponentInChildren<TMP_Text>(true);
            GameObject root = new GameObject("BlackforgeForbidden", typeof(RectTransform), typeof(Image));
            root.transform.SetParent(menu.m_selectCharacterPanel.transform, false);
            root.GetComponent<Image>().color = new Color(0f, 0f, 0f, 0.75f);
            RectTransform rect = root.GetComponent<RectTransform>();
            rect.anchorMin = new Vector2(1f, 0.5f);
            rect.anchorMax = new Vector2(1f, 0.5f);
            rect.pivot = new Vector2(1f, 0.5f);
            rect.anchoredPosition = new Vector2(-60f, 40f);

            List<string> names = found
                .Select(item => item.m_dropPrefab.name)
                .Distinct()
                .OrderByDescending(name => waiting.forbidden[name].tier)
                .ThenBy(name => name)
                .ToList();
            float listHeight = Math.Min(names.Count, ShownMax) * RowHeight;
            rect.sizeDelta = new Vector2(PanelWidth, Padding * 2 + TitleHeight + listHeight);

            Text(root, model, $"{waiting.server} is competitive. Leave these items in another world:",
                TitleSize, new Vector2(Padding, -Padding), new Vector2(PanelWidth - Padding * 2, TitleHeight));

            // A long list scrolls with the mouse wheel or the bar on the right.
            float listWidth = PanelWidth - Padding * 2 - BarWidth - 6f;
            GameObject viewport = Box(root, "Viewport", new Vector2(Padding, -Padding - TitleHeight),
                new Vector2(listWidth, listHeight), typeof(RectMask2D));
            GameObject content = Box(viewport, "Content", Vector2.zero,
                new Vector2(listWidth, names.Count * RowHeight));
            for (int i = 0; i < names.Count; i++)
            {
                string name = names[i];
                ItemDrop.ItemData item = found.First(candidate => candidate.m_dropPrefab.name == name);
                float y = -RowHeight * i;
                GameObject icon = Box(content, "Icon", new Vector2(0f, y - 2f), new Vector2(IconSize, IconSize), typeof(Image));
                icon.GetComponent<Image>().sprite = item.GetIcon();
                Text(content, model, $"{Tiers.Title(name)}, needs {waiting.forbidden[name].boss}",
                    RowTextSize, new Vector2(IconSize + 10f, y), new Vector2(listWidth - IconSize - 10f, RowHeight));
            }
            ScrollRect scroll = viewport.AddComponent<ScrollRect>();
            scroll.viewport = viewport.GetComponent<RectTransform>();
            scroll.content = content.GetComponent<RectTransform>();
            scroll.horizontal = false;
            scroll.vertical = true;
            scroll.movementType = ScrollRect.MovementType.Clamped;
            scroll.scrollSensitivity = RowHeight;
            if (names.Count > ShownMax)
            {
                GameObject track = Box(root, "Bar", new Vector2(PanelWidth - Padding - BarWidth, -Padding - TitleHeight),
                    new Vector2(BarWidth, listHeight), typeof(Image));
                track.GetComponent<Image>().color = new Color(1f, 1f, 1f, 0.1f);
                GameObject handle = new GameObject("Handle", typeof(RectTransform), typeof(Image));
                handle.transform.SetParent(track.transform, false);
                handle.GetComponent<Image>().color = new Color(1f, 1f, 1f, 0.45f);
                RectTransform handleRect = handle.GetComponent<RectTransform>();
                handleRect.anchorMin = Vector2.zero;
                handleRect.anchorMax = Vector2.one;
                handleRect.sizeDelta = Vector2.zero;
                Scrollbar bar = track.AddComponent<Scrollbar>();
                bar.handleRect = handleRect;
                bar.targetGraphic = handle.GetComponent<Image>();
                bar.direction = Scrollbar.Direction.BottomToTop;
                scroll.verticalScrollbar = bar;
                scroll.verticalScrollbarVisibility = ScrollRect.ScrollbarVisibility.Permanent;
            }
            return root;
        }

        // A child anchored at the top left of its parent.
        private static GameObject Box(GameObject parent, string name, Vector2 at, Vector2 size, params Type[] components)
        {
            GameObject box = new GameObject(name, new[] { typeof(RectTransform) }.Concat(components).ToArray());
            box.transform.SetParent(parent.transform, false);
            RectTransform rect = box.GetComponent<RectTransform>();
            rect.anchorMin = new Vector2(0f, 1f);
            rect.anchorMax = new Vector2(0f, 1f);
            rect.pivot = new Vector2(0f, 1f);
            rect.anchoredPosition = at;
            rect.sizeDelta = size;
            return box;
        }

        private static void Text(GameObject parent, TMP_Text model, string text, float size, Vector2 at, Vector2 box)
        {
            GameObject label = new GameObject("Text", typeof(RectTransform));
            label.transform.SetParent(parent.transform, false);
            TextMeshProUGUI tmp = label.AddComponent<TextMeshProUGUI>();
            if (model != null)
            {
                tmp.font = model.font;
            }
            tmp.text = text;
            tmp.fontSize = size;
            tmp.textWrappingMode = TextWrappingModes.Normal;
            tmp.overflowMode = TextOverflowModes.Ellipsis;
            tmp.alignment = TextAlignmentOptions.MidlineLeft;
            tmp.color = Color.white;
            RectTransform rect = label.GetComponent<RectTransform>();
            rect.anchorMin = new Vector2(0f, 1f);
            rect.anchorMax = new Vector2(0f, 1f);
            rect.pivot = new Vector2(0f, 1f);
            rect.anchoredPosition = at;
            rect.sizeDelta = box;
        }

        // A postfix of ZNet.OnNewConnection through the join plugin.
        public static void Listen(ZNetPeer peer)
        {
            cleared = null;
            lastReport = null;
            server = peer.m_rpc;
            peer.m_rpc.Register<string>(CheckRpc, (rpc, world) => Report(rpc, true));
            peer.m_rpc.Register<string>(ClearedRpc, (rpc, world) =>
            {
                cleared = world;
                log.LogInfo("the server cleared this character, new items get its world tag");
            });
        }

        private static void Report(ZRpc rpc, bool asked)
        {
            Player player = Player.m_localPlayer;
            if (player == null)
            {
                return;
            }
            List<Carried> carried = player.GetInventory().GetAllItems()
                .Where(item => item.m_dropPrefab != null)
                .Select(item => new Carried { n = item.m_dropPrefab.name, t = Tiers.Tag(item) })
                .ToList();
            string json = JsonConvert.SerializeObject(carried);
            if (!asked && json == lastReport)
            {
                return;
            }
            lastReport = json;
            rpc.Invoke(InventoryRpc, json);
        }

        // Once a second while cleared on a competitive server: every item
        // gets the world tag, then a changed inventory goes to the server.
        public static void Tick()
        {
            if (Time.unscaledTime < next)
            {
                return;
            }
            next = Time.unscaledTime + TickInterval;
            if (cleared == null)
            {
                return;
            }
            if (ZNet.instance == null || server == null || !server.IsConnected())
            {
                cleared = null;
                server = null;
                return;
            }
            Player player = Player.m_localPlayer;
            if (player == null)
            {
                return;
            }
            foreach (ItemDrop.ItemData item in player.GetInventory().GetAllItems())
            {
                if (Tiers.Tag(item) != cleared)
                {
                    item.m_customData[Tiers.TagKey] = cleared;
                }
            }
            Report(server, false);
        }

        private static void TaintStacks(Inventory __instance, ItemDrop.ItemData item)
        {
            if (item == null || item.m_shared.m_maxStackSize <= 1)
            {
                return;
            }
            string tag = Tiers.Tag(item);
            foreach (ItemDrop.ItemData stack in __instance.GetAllItems())
            {
                if (stack != item
                    && stack.m_shared.m_name == item.m_shared.m_name
                    && stack.m_quality == item.m_quality
                    && stack.m_stack < stack.m_shared.m_maxStackSize)
                {
                    Taint(stack, tag);
                }
            }
        }

        private static void TaintSlot(Inventory __instance, ItemDrop.ItemData item, int x, int y)
        {
            ItemDrop.ItemData stack = item == null ? null : __instance.GetItemAt(x, y);
            if (stack != null && stack != item && stack.IsSameType(item))
            {
                Taint(stack, Tiers.Tag(item));
            }
        }

        private static void Taint(ItemDrop.ItemData stack, string incoming)
        {
            string tag = Tiers.Tag(stack);
            if (tag != null && tag != incoming)
            {
                stack.m_customData.Remove(Tiers.TagKey);
            }
        }
    }
}
