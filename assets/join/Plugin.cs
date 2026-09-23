using BepInEx;
using HarmonyLib;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Blackforge
{
    // A main menu button that joins the Durka server. It queues a join by address,
    // the same one a Steam invite or `+connect` makes, so the game itself shows
    // character select, finds the server's PlayFab lobby by this address and asks
    // for the password. The address is the public one the server registers its
    // crossplay lobby with, the ports on pc1 stay closed.
    [BepInPlugin("xyz.vladas.blackforge.join", "Blackforge Join", "1.1.0")]
    public class JoinPlugin : BaseUnityPlugin
    {
        private const string Address = "86.100.76.6:2456";
        private const string Label = "Join Durka";
        private static readonly Vector2 Size = new Vector2(360f, 80f);
        private static readonly Vector2 Margin = new Vector2(40f, -40f);
        private const float FontSize = 36f;

        private static BepInEx.Logging.ManualLogSource log;

        private void Awake()
        {
            log = Logger;
            new Harmony(Info.Metadata.GUID).Patch(
                AccessTools.Method(typeof(FejdStartup), nameof(FejdStartup.SetupGui)),
                postfix: new HarmonyMethod(typeof(JoinPlugin), nameof(AddButton)));
        }

        private static void AddButton(FejdStartup __instance)
        {
            // The menu entries are bare text, the character select Start button
            // has a frame, so that one is the model.
            Button template = __instance.m_csStartButton;
            if (template == null)
            {
                log.LogWarning("the character select has no start button, no join button is added");
                return;
            }

            GameObject copy = Object.Instantiate(template.gameObject, __instance.m_mainMenu.transform, false);
            copy.name = "BlackforgeJoin";
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
            rect.anchoredPosition = Margin;
            rect.sizeDelta = Size;
            rect.localScale = Vector3.one;

            TMP_Text text = copy.GetComponentInChildren<TMP_Text>(true);
            text.text = Label;
            text.enableAutoSizing = false;
            text.fontSize = FontSize;

            Button button = copy.GetComponent<Button>();
            button.interactable = true;
            // A new event drops the click handlers the copy brought from the template.
            button.onClick = new Button.ButtonClickedEvent();
            button.onClick.AddListener(() => ZSteamMatchmaking.instance.QueueServerJoin(Address));
        }
    }
}
