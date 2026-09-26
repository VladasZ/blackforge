using System.Linq;
using HarmonyLib;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Blackforge
{
    // The station menu of a locomotive: one button per named station with the
    // coal the trip costs. A click sends the train there. The server sends
    // the list, see Network.OnList.
    public static class StationMenu
    {
        private static GameObject root;
        private static ZDOID loco;

        public static bool IsOpen => root != null && root.activeSelf;

        public static void Show(ZDOID train, string rows)
        {
            loco = train;
            Close();
            root = new GameObject("BlackforgeRailMenu");
            Canvas canvas = root.AddComponent<Canvas>();
            canvas.renderMode = RenderMode.ScreenSpaceOverlay;
            canvas.sortingOrder = 50;
            CanvasScaler scaler = root.AddComponent<CanvasScaler>();
            scaler.uiScaleMode = CanvasScaler.ScaleMode.ScaleWithScreenSize;
            scaler.referenceResolution = new Vector2(1920, 1080);
            root.AddComponent<GraphicRaycaster>();

            string[] lines = string.IsNullOrEmpty(rows) ? new string[0] : rows.Split('\n');
            float height = 150f + Mathf.Max(1, lines.Length) * 56f;
            RectTransform panel = Box(root.transform, "panel", new Vector2(560, height), new Color(0.08f, 0.06f, 0.05f, 0.94f));
            Label(panel, "Send the train to", 34, new Vector2(0, height / 2 - 45), new Color(1f, 0.63f, 0.24f));

            float y = height / 2 - 105;
            if (lines.Length == 0)
            {
                Label(panel, "No station has a name yet", 24, new Vector2(0, y), Color.white);
            }
            foreach (string line in lines)
            {
                string[] parts = line.Split('\t');
                if (parts.Length != 3)
                {
                    continue;
                }
                ZDOID station = Train.ParseId(parts[0]);
                bool reachable = parts[2] != "-";
                string text = reachable ? $"{parts[1]}   {parts[2]} coal" : $"{parts[1]}   no track ahead";
                Button(panel, text, new Vector2(0, y), reachable, () =>
                {
                    ZRoutedRpc.instance.InvokeRoutedRPC(Network.Server, "bf_request", loco, station);
                    Close();
                });
                y -= 56f;
            }
            Button(panel, "Close", new Vector2(0, -height / 2 + 40), true, Close);
        }

        public static void Close()
        {
            if (root != null)
            {
                Object.Destroy(root);
                root = null;
            }
        }

        private static TMP_FontAsset Font()
        {
            return Hud.instance != null ? Hud.instance.GetComponentsInChildren<TMP_Text>(true).Select(t => t.font).FirstOrDefault(f => f != null) : null;
        }

        private static RectTransform Box(Transform parent, string name, Vector2 size, Color color)
        {
            GameObject go = new GameObject(name);
            go.transform.SetParent(parent, false);
            RectTransform rect = go.AddComponent<RectTransform>();
            rect.sizeDelta = size;
            go.AddComponent<Image>().color = color;
            return rect;
        }

        private static void Label(Transform parent, string text, float size, Vector2 at, Color color)
        {
            GameObject go = new GameObject("label");
            go.transform.SetParent(parent, false);
            RectTransform rect = go.AddComponent<RectTransform>();
            rect.sizeDelta = new Vector2(520, 50);
            rect.anchoredPosition = at;
            TextMeshProUGUI label = go.AddComponent<TextMeshProUGUI>();
            label.font = Font();
            label.fontSize = size;
            label.alignment = TextAlignmentOptions.Center;
            label.color = color;
            label.text = text;
            label.raycastTarget = false;
        }

        private static void Button(Transform parent, string text, Vector2 at, bool enabled, UnityEngine.Events.UnityAction click)
        {
            RectTransform rect = Box(parent, "button", new Vector2(500, 48), enabled ? new Color(0.3f, 0.22f, 0.14f, 1f) : new Color(0.18f, 0.16f, 0.15f, 1f));
            rect.anchoredPosition = at;
            Button button = rect.gameObject.AddComponent<Button>();
            button.interactable = enabled;
            button.onClick.AddListener(click);
            Label(rect, text, 26, Vector2.zero, enabled ? Color.white : new Color(0.6f, 0.6f, 0.6f));
        }

        // While the menu is open the mouse is free and the player stands still.
        [HarmonyPatch(typeof(GameCamera), "UpdateMouseCapture")]
        private static class FreeMouse
        {
            private static void Postfix()
            {
                if (IsOpen)
                {
                    Cursor.lockState = CursorLockMode.None;
                    Cursor.visible = true;
                }
            }
        }

        [HarmonyPatch(typeof(Player), "TakeInput")]
        private static class NoInput
        {
            private static void Postfix(ref bool __result)
            {
                if (IsOpen)
                {
                    __result = false;
                }
            }
        }

        [HarmonyPatch(typeof(Player), "Update")]
        private static class CloseOnEscape
        {
            private static void Postfix()
            {
                if (IsOpen && (Input.GetKeyDown(KeyCode.Escape) || Player.m_localPlayer == null || Player.m_localPlayer.IsDead()))
                {
                    Close();
                }
            }
        }
    }
}
