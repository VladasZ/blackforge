using System.Collections;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Text;
using BepInEx;
using HarmonyLib;
using Unity.Collections;
using UnityEngine;
using UnityEngine.Rendering;

namespace Blackforge
{
    // Pictures of what the player sees of a train, frame by frame, to find a
    // flicker by eye. The console command `railshot 3` crops the screen around
    // the nearest train on every frame for 3 seconds and tiles the crops into
    // sheets of 30, left to right and top to bottom, in
    // BepInEx/rail-shots/<time>/. The crop follows the train, or stays put
    // while the player rides, so a shake against the camera shows too.
    // `railshape off` shows the models without their baked bend and
    // `railmat _ValueNoise 0` changes the train materials, to compare.
    public class RailShot : MonoBehaviour
    {
        // Crops of 256 by 256 in sheets of 6 by 5, or the whole screen at 512 by
        // 288 in sheets of 3 by 6.
        public static bool FullScreen;
        private int m_tileWidth = 256;
        private int m_tileHeight = 256;
        private int m_columns = 6;
        private int m_rows = 5;

        private float m_until;
        private string m_folder;
        private RenderTexture m_screen;
        private RenderTexture m_tile;
        private RenderTexture m_sheet;
        private int m_index;
        private int m_sheets;
        private bool m_fixed;
        private Rect m_rect;
        private StringBuilder m_frames;

        private static RailShot instance;

        private void Awake()
        {
            instance = this;
            StartCoroutine(EndOfFrames());
        }

        // The train to shoot, the nearest one when none.
        public static ZDOID Target = ZDOID.None;

        public static bool Busy => instance != null && instance.m_frames != null;

        public static void Run(float seconds, string label)
        {
            if (instance != null)
            {
                instance.Begin(seconds, label);
            }
        }

        private void Begin(float seconds, string label = null)
        {
            string name = System.DateTime.Now.ToString("yyyyMMdd-HHmmss") + (string.IsNullOrEmpty(label) ? "" : "-" + label);
            m_folder = Path.Combine(Paths.BepInExRootPath, "rail-shots", name);
            Directory.CreateDirectory(m_folder);
            m_until = Time.time + seconds;
            m_index = 0;
            m_sheets = 0;
            m_rect = Rect.zero;
            m_tileWidth = FullScreen ? 512 : 256;
            m_tileHeight = FullScreen ? 288 : 256;
            m_columns = FullScreen ? 3 : 6;
            m_rows = FullScreen ? 6 : 5;
            m_fixed = Player.m_localPlayer != null && Player.m_localPlayer.IsAttached();
            m_frames = new StringBuilder("sheet,tile,time,dt,speed,owner,x,y,size\n");
            ScreenCapture.CaptureScreenshot(Path.Combine(m_folder, "screen.png"));
            RailPlugin.Log.LogInfo($"rail shots for {seconds:0} s into {m_folder}, the crop {(m_fixed ? "stays put" : "follows the train")}");
        }

        private IEnumerator EndOfFrames()
        {
            WaitForEndOfFrame end = new WaitForEndOfFrame();
            while (true)
            {
                yield return end;
                if (m_frames == null)
                {
                    continue;
                }
                Shoot();
                if (Time.time >= m_until)
                {
                    Flush();
                    File.WriteAllText(Path.Combine(m_folder, "frames.csv"), m_frames.ToString());
                    m_frames = null;
                    RailPlugin.Log.LogInfo($"rail shots done, {m_sheets} sheets in {m_folder}");
                }
            }
        }

        private void Shoot()
        {
            Camera camera = Utils.GetMainCamera();
            Player player = Player.m_localPlayer;
            if (camera == null || player == null || ZNetScene.instance == null)
            {
                return;
            }
            ZDO loco = Train.Trains()
                .Where(z => ZNetScene.instance.FindInstance(z) != null && (Target == ZDOID.None || z.m_uid == Target))
                .OrderBy(z => (z.GetPosition() - player.transform.position).sqrMagnitude)
                .FirstOrDefault();
            if (loco == null)
            {
                return;
            }
            // The whole train on the screen.
            List<Renderer> renderers = new List<Renderer>();
            foreach (ZDO part in Train.WagonIds(loco).Select(id => ZDOMan.instance.GetZDO(id)).Where(z => z != null).Prepend(loco))
            {
                ZNetView view = ZNetScene.instance.FindInstance(part);
                if (view != null)
                {
                    renderers.AddRange(view.GetComponentsInChildren<MeshRenderer>());
                }
            }
            Rect rect = ScreenRect(camera, renderers);
            if (rect.width <= 0f)
            {
                return;
            }
            if (FullScreen)
            {
                m_rect = new Rect(0f, 0f, Screen.width, Screen.height);
            }
            else if (!m_fixed || m_rect == Rect.zero)
            {
                float size = Mathf.Clamp(Mathf.Max(rect.width, rect.height) * 1.15f, 64f, Mathf.Min(Screen.width, Screen.height));
                m_rect = new Rect(rect.center.x - size / 2f, rect.center.y - size / 2f, size, size);
            }

            if (m_screen == null || m_screen.width != Screen.width || m_screen.height != Screen.height || m_tile.width != m_tileWidth || m_tile.height != m_tileHeight)
            {
                if (m_screen != null)
                {
                    m_screen.Release();
                }
                m_screen = new RenderTexture(Screen.width, Screen.height, 0, RenderTextureFormat.ARGB32);
                m_tile = new RenderTexture(m_tileWidth, m_tileHeight, 0, RenderTextureFormat.ARGB32);
                m_sheet = new RenderTexture(m_tileWidth * m_columns, m_tileHeight * m_rows, 0, RenderTextureFormat.ARGB32);
            }
            ScreenCapture.CaptureScreenshotIntoRenderTexture(m_screen);
            Vector2 scale = new Vector2(m_rect.width / Screen.width, m_rect.height / Screen.height);
            Vector2 offset = new Vector2(m_rect.x / Screen.width, m_rect.y / Screen.height);
            if (SystemInfo.graphicsUVStartsAtTop)
            {
                // Metal keeps the captured screen upside down, read the rows the
                // other way round.
                offset.y = 1f - m_rect.y / Screen.height;
                scale.y = -scale.y;
            }
            Graphics.Blit(m_screen, m_tile, scale, offset);
            int column = m_index % m_columns;
            int row = m_index / m_columns;
            // The sheet reads top to bottom, and a texture starts at the bottom.
            Graphics.CopyTexture(m_tile, 0, 0, 0, 0, m_tileWidth, m_tileHeight, m_sheet, 0, 0, column * m_tileWidth, (m_rows - 1 - row) * m_tileHeight);
            CultureInfo inv = CultureInfo.InvariantCulture;
            m_frames.Append($"{m_sheets},{m_index},{Time.time.ToString("0.0000", inv)},{Time.deltaTime.ToString("0.0000", inv)},{loco.GetFloat(Train.Speed).ToString("0.00", inv)},{(loco.IsOwner() ? "me" : "other")},{m_rect.x.ToString("0", inv)},{m_rect.y.ToString("0", inv)},{m_rect.width.ToString("0", inv)}\n");
            m_index++;
            if (m_index >= m_columns * m_rows)
            {
                Flush();
            }
        }

        private static Rect ScreenRect(Camera camera, List<Renderer> renderers)
        {
            Vector2 min = new Vector2(float.MaxValue, float.MaxValue);
            Vector2 max = new Vector2(float.MinValue, float.MinValue);
            foreach (Renderer renderer in renderers)
            {
                Bounds b = renderer.bounds;
                for (int i = 0; i < 8; i++)
                {
                    Vector3 corner = b.center + Vector3.Scale(b.extents, new Vector3((i & 1) == 0 ? -1 : 1, (i & 2) == 0 ? -1 : 1, (i & 4) == 0 ? -1 : 1));
                    Vector3 p = camera.WorldToScreenPoint(corner);
                    if (p.z <= 0f)
                    {
                        continue;
                    }
                    min = Vector2.Min(min, p);
                    max = Vector2.Max(max, p);
                }
            }
            if (min.x > max.x)
            {
                return Rect.zero;
            }
            min = Vector2.Max(min, Vector2.zero);
            max = Vector2.Min(max, new Vector2(Screen.width, Screen.height));
            return max.x > min.x && max.y > min.y ? Rect.MinMaxRect(min.x, min.y, max.x, max.y) : Rect.zero;
        }

        private void Flush()
        {
            if (m_index == 0)
            {
                return;
            }
            string path = Path.Combine(m_folder, $"sheet-{m_sheets:000}.png");
            int width = m_sheet.width;
            int height = m_sheet.height;
            AsyncGPUReadback.Request(m_sheet, 0, TextureFormat.RGBA32, request =>
            {
                if (request.hasError)
                {
                    RailPlugin.Log.LogWarning($"rail shot {path} could not be read back");
                    return;
                }
                NativeArray<byte> data = request.GetData<byte>();
                byte[] png = ImageConversion.EncodeNativeArrayToPNG(data, UnityEngine.Experimental.Rendering.GraphicsFormat.R8G8B8A8_UNorm, (uint)width, (uint)height).ToArray();
                File.WriteAllBytes(path, png);
            });
            // A fresh sheet, the old one is read back in the order of the GPU
            // commands.
            RenderTexture active = RenderTexture.active;
            RenderTexture.active = m_sheet;
            GL.Clear(true, true, Color.black);
            RenderTexture.active = active;
            m_sheets++;
            m_index = 0;
        }

        // Shows the train models with or without the bend TrainShape bakes.
        private static void Shape(bool on)
        {
            TrainShape.Enabled = on;
            foreach (TrainShape shape in FindObjectsByType<TrainShape>(FindObjectsSortMode.None))
            {
                shape.Apply();
            }
        }

        [HarmonyPatch(typeof(Terminal), "InitTerminal")]
        private static class Commands
        {
            private static void Postfix()
            {
                new Terminal.ConsoleCommand("railshot", "[seconds] pictures of the nearest train every frame, see docs/rail.md", args =>
                {
                    float seconds = args.Length > 1 && float.TryParse(args[1], NumberStyles.Float, CultureInfo.InvariantCulture, out float value) ? value : 3f;
                    if (instance != null)
                    {
                        instance.Begin(seconds);
                    }
                    args.Context?.AddString($"rail shots for {seconds:0} s, see BepInEx/rail-shots");
                });
                new Terminal.ConsoleCommand("railshape", "on|off, the baked bend of the train models", args =>
                {
                    bool on = args.Length < 2 || args[1] != "off";
                    Shape(on);
                    args.Context?.AddString($"train bend {(on ? "on" : "off")}");
                });
                new Terminal.ConsoleCommand("railmat", "[property value] shows or sets a float of the train materials", args =>
                {
                    if (args.Length >= 3 && float.TryParse(args[2], NumberStyles.Float, CultureInfo.InvariantCulture, out float value))
                    {
                        foreach (Material material in Pieces.Moving)
                        {
                            material.SetFloat(args[1], value);
                        }
                    }
                    foreach (Material material in Pieces.Moving)
                    {
                        Shader shader = material.shader;
                        string values = string.Join(", ", Enumerable.Range(0, shader.GetPropertyCount())
                            .Where(i => shader.GetPropertyType(i) == ShaderPropertyType.Float || shader.GetPropertyType(i) == ShaderPropertyType.Range)
                            .Select(i => $"{shader.GetPropertyName(i)} {material.GetFloat(shader.GetPropertyName(i)):0.###}"));
                        args.Context?.AddString($"{material.name}: {values}");
                        RailPlugin.Log.LogInfo($"railmat {material.name}: {values}");
                    }
                });
            }
        }
    }
}
