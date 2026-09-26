using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text.RegularExpressions;
using BepInEx;
using BepInEx.Logging;
using HarmonyLib;
using Newtonsoft.Json;
using TMPro;
using UnityEngine;
using UnityEngine.TextCore;
using UnityEngine.UI;

namespace Blackforge
{
    // Animated emojis. A player types `:petuh` and the chat window shows the
    // rooster, and a big rooster shows above the head of the player. The app
    // turns every GIF into a sprite sheet and lists the sheets in
    // emojis.json, Unity cannot read a GIF.
    //
    // The message goes over the network as the plain text `:petuh`, so a
    // player without the plugin sees the code, and the server needs nothing.
    [BepInPlugin("xyz.vladas.blackforge.emoji", "Blackforge Emoji", "1.0.0")]
    public class EmojiPlugin : BaseUnityPlugin
    {
        private const string ListFile = "emojis.json";
        private const string BigName = "BlackforgeEmoji";

        // About 6 lines of chat text.
        private const float ChatScale = 6.4f;

        // In units of the chat canvas.
        private const float BigHeight = 160f;

        public class EmojiList
        {
            public List<EmojiSheet> emojis = new List<EmojiSheet>();
        }

        public class EmojiSheet
        {
            public string name;
            public string file;
            public int width;
            public int height;
            public int columns;
            public int frames;
            public int fps;
        }

        private class Emoji
        {
            public EmojiSheet sheet;
            public Sprite[] frames;
            public TMP_SpriteAsset asset;
        }

        private static ManualLogSource log;
        private static readonly Dictionary<string, Emoji> emojis = new Dictionary<string, Emoji>();
        private static TMP_SpriteAsset rootAsset;
        private static Regex code;

        private void Awake()
        {
            log = Logger;
            string dir = Path.GetDirectoryName(Info.Location);
            try
            {
                Load(dir);
            }
            catch (Exception error)
            {
                log.LogError($"emojis are off, {ListFile} could not be read: {error}");
                return;
            }
            if (emojis.Count == 0)
            {
                log.LogWarning($"{ListFile} lists no emoji, emojis are off");
                return;
            }
            // A whole word: `hi :petuh!` matches, `a:petuh` and `:petuhs` do
            // not. The chat line has the text inside color tags, so a `>`
            // before the colon still counts as a word start.
            string names = string.Join("|", emojis.Keys.Select(Regex.Escape));
            code = new Regex($@"(?<![\w:]):({names})(?!\w)", RegexOptions.IgnoreCase);

            Harmony harmony = new Harmony(Info.Metadata.GUID);
            harmony.Patch(
                AccessTools.Method(typeof(Chat), nameof(Chat.Awake)),
                postfix: new HarmonyMethod(typeof(EmojiPlugin), nameof(AttachSprites)));
            harmony.Patch(
                AccessTools.Method(typeof(Terminal), nameof(Terminal.AddString), new[] { typeof(string) }),
                prefix: new HarmonyMethod(typeof(EmojiPlugin), nameof(ChatLine)));
            harmony.Patch(
                AccessTools.Method(typeof(Chat), nameof(Chat.AddInworldText)),
                postfix: new HarmonyMethod(typeof(EmojiPlugin), nameof(HeadText)));
            log.LogInfo($"emojis ready: {string.Join(", ", emojis.Keys)}");
        }

        private static void Load(string dir)
        {
            EmojiList list = JsonConvert.DeserializeObject<EmojiList>(File.ReadAllText(Path.Combine(dir, ListFile)));
            foreach (EmojiSheet sheet in list?.emojis ?? new List<EmojiSheet>())
            {
                if (string.IsNullOrEmpty(sheet?.name) || sheet.frames < 1 || sheet.columns < 1)
                {
                    continue;
                }
                Texture2D texture = new Texture2D(2, 2, TextureFormat.RGBA32, false);
                texture.LoadImage(File.ReadAllBytes(Path.Combine(dir, sheet.file)), true);
                texture.wrapMode = TextureWrapMode.Clamp;
                texture.filterMode = FilterMode.Bilinear;
                texture.name = sheet.name;

                Emoji emoji = new Emoji { sheet = sheet, frames = new Sprite[sheet.frames] };
                for (int i = 0; i < sheet.frames; i++)
                {
                    emoji.frames[i] = Sprite.Create(texture, FrameRect(sheet, texture, i), new Vector2(0.5f, 0.5f), 100f);
                }
                emoji.asset = SpriteAsset(sheet, texture);
                emojis[sheet.name.ToLowerInvariant()] = emoji;
                if (rootAsset == null)
                {
                    rootAsset = emoji.asset;
                }
                else
                {
                    rootAsset.fallbackSpriteAssets.Add(emoji.asset);
                }
            }
        }

        // The sheet has its frames in rows from the top left, a texture
        // counts from the bottom left.
        private static Rect FrameRect(EmojiSheet sheet, Texture2D texture, int frame)
        {
            int x = frame % sheet.columns * sheet.width;
            int y = texture.height - (frame / sheet.columns + 1) * sheet.height;
            return new Rect(x, y, sheet.width, sheet.height);
        }

        // One sprite asset per emoji, since a sprite asset has one texture.
        // The frames are its sprites, and `<sprite anim>` plays them.
        private static TMP_SpriteAsset SpriteAsset(EmojiSheet sheet, Texture2D texture)
        {
            TMP_SpriteAsset asset = ScriptableObject.CreateInstance<TMP_SpriteAsset>();
            asset.name = sheet.name;
            asset.m_Version = "1.1.0";
            asset.hashCode = TMP_TextUtilities.GetSimpleHashCode(asset.name);
            asset.spriteSheet = texture;
            asset.material = SpriteMaterial(texture);
            asset.fallbackSpriteAssets = new List<TMP_SpriteAsset>();

            // A point size equal to the frame height makes one frame one em
            // tall. The frame sits 20 percent below the baseline, like text.
            float ascent = sheet.height * 0.8f;
            FaceInfo face = asset.faceInfo;
            face.pointSize = sheet.height;
            face.scale = 1f;
            face.lineHeight = sheet.height;
            face.ascentLine = ascent;
            face.descentLine = ascent - sheet.height;
            face.baseline = 0f;
            asset.faceInfo = face;

            for (int i = 0; i < sheet.frames; i++)
            {
                Rect rect = FrameRect(sheet, texture, i);
                GlyphMetrics metrics = new GlyphMetrics(sheet.width, sheet.height, 0f, ascent, sheet.width);
                GlyphRect glyphRect = new GlyphRect((int)rect.x, (int)rect.y, sheet.width, sheet.height);
                TMP_SpriteGlyph glyph = new TMP_SpriteGlyph((uint)i, metrics, glyphRect, ChatScale, 0);
                asset.spriteGlyphTable.Add(glyph);
                TMP_SpriteCharacter character = new TMP_SpriteCharacter(0xE000 + (uint)i, glyph);
                // Only the first frame has the name, the tag finds it by it.
                character.name = i == 0 ? sheet.name : $"{sheet.name}_{i}";
                asset.spriteCharacterTable.Add(character);
            }
            asset.UpdateLookupTables();
            return asset;
        }

        private static Material SpriteMaterial(Texture2D texture)
        {
            Shader shader = Shader.Find("TextMeshPro/Sprite");
            Material material;
            if (shader != null)
            {
                material = new Material(shader);
            }
            else if (TMP_Settings.defaultSpriteAsset != null && TMP_Settings.defaultSpriteAsset.material != null)
            {
                material = new Material(TMP_Settings.defaultSpriteAsset.material);
            }
            else
            {
                log.LogWarning("the TextMeshPro sprite shader is missing, emojis use the UI shader");
                material = new Material(Shader.Find("UI/Default"));
            }
            material.mainTexture = texture;
            return material;
        }

        private static void AttachSprites(Chat __instance)
        {
            TMP_Text output = __instance.m_output;
            if (output.spriteAsset == null)
            {
                output.spriteAsset = rootAsset;
            }
            else if (!output.spriteAsset.fallbackSpriteAssets.Contains(rootAsset))
            {
                output.spriteAsset.fallbackSpriteAssets.Add(rootAsset);
            }
        }

        // Every chat line passes here. The console is a Terminal too and
        // keeps its text as it is.
        private static void ChatLine(Terminal __instance, ref string text)
        {
            if (!(__instance is Chat) || text == null)
            {
                return;
            }
            text = code.Replace(text, match =>
            {
                EmojiSheet sheet = emojis[match.Groups[1].Value.ToLowerInvariant()].sheet;
                return $"<sprite name=\"{sheet.name}\" anim=\"0,{sheet.frames - 1},{sheet.fps}\">";
            });
        }

        // The head bubble loses the code, and a big emoji goes above it. A
        // message of only emojis shows no bubble. The game reuses the bubble
        // of a player for the next message, so every message sets it again.
        private static void HeadText(Chat __instance, long senderID)
        {
            Chat.WorldTextInstance bubble = __instance.FindExistingWorldText(senderID);
            if (bubble == null || bubble.m_type == Talker.Type.Ping)
            {
                return;
            }
            Transform old = bubble.m_gui.transform.Find(BigName);
            if (old != null)
            {
                UnityEngine.Object.Destroy(old.gameObject);
            }

            Match first = code.Match(bubble.m_text);
            string rest = first.Success ? Regex.Replace(code.Replace(bubble.m_text, ""), @"\s+", " ").Trim() : bubble.m_text;
            bool hide = first.Success && rest.Length == 0;
            BubbleState state = bubble.m_gui.GetComponent<BubbleState>();
            if (state == null)
            {
                state = bubble.m_gui.AddComponent<BubbleState>();
            }
            state.Hide(hide);
            if (!first.Success)
            {
                return;
            }
            bubble.m_text = rest;
            __instance.UpdateWorldTextField(bubble);
            AddBig(bubble.m_gui, emojis[first.Groups[1].Value.ToLowerInvariant()], hide);
        }

        private static void AddBig(GameObject gui, Emoji emoji, bool alone)
        {
            GameObject big = new GameObject(BigName, typeof(RectTransform));
            RectTransform rect = (RectTransform)big.transform;
            rect.SetParent(gui.transform, false);
            // Above the bubble, or where the bubble would be.
            Vector2 anchor = alone ? new Vector2(0.5f, 0.5f) : new Vector2(0.5f, 1f);
            rect.anchorMin = anchor;
            rect.anchorMax = anchor;
            rect.pivot = new Vector2(0.5f, 0f);
            rect.anchoredPosition = new Vector2(0f, alone ? 0f : 4f);
            rect.sizeDelta = new Vector2(BigHeight * emoji.sheet.width / emoji.sheet.height, BigHeight);

            Image image = big.AddComponent<Image>();
            image.raycastTarget = false;
            image.preserveAspect = true;
            Flipbook flipbook = big.AddComponent<Flipbook>();
            flipbook.image = image;
            flipbook.frames = emoji.frames;
            flipbook.fps = emoji.sheet.fps;
            image.sprite = emoji.frames[0];
        }
    }

    public class Flipbook : MonoBehaviour
    {
        public Image image;
        public Sprite[] frames;
        public float fps;
        private float time;

        private void Update()
        {
            time += Time.deltaTime;
            image.sprite = frames[(int)(time * fps) % frames.Length];
        }
    }

    // Turns the bubble picture and text of a head bubble off and on again.
    // Only what was on is turned back on.
    public class BubbleState : MonoBehaviour
    {
        private readonly List<Graphic> hidden = new List<Graphic>();

        public void Hide(bool hide)
        {
            foreach (Graphic graphic in hidden)
            {
                if (graphic != null)
                {
                    graphic.enabled = true;
                }
            }
            hidden.Clear();
            if (!hide)
            {
                return;
            }
            foreach (Graphic graphic in GetComponentsInChildren<Graphic>())
            {
                if (graphic.enabled)
                {
                    graphic.enabled = false;
                    hidden.Add(graphic);
                }
            }
        }
    }
}
