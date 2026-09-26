using System.Collections;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Text;
using BepInEx;
using HarmonyLib;
using UnityEngine;

namespace Blackforge
{
    // Debug tools for trains that look wrong in the game. The console command
    // `railtrace 15` records every frame for 15 seconds, after rendering: the
    // camera, the local player and every locomotive and wagon near it. It
    // writes a csv to BepInEx/rail-trace-*.csv and a summary to the log.
    // Owner changes and trains made again by the scene are always logged.
    public class RailTrace : MonoBehaviour
    {
        private const float Radius = 80f;

        public static RailTrace Instance;

        private readonly Dictionary<ZDOID, long> m_owners = new Dictionary<ZDOID, long>();
        private float m_until;
        private StringBuilder m_csv;
        private string m_path;

        private class Track
        {
            public Vector3 lastPos;
            public Vector3 lastRel;
            public Vector3 lastStep;
            public int frames;
            public int backward;
            public int hidden;
            public float maxJump;
            public float jitterSum;
            public float jitterMax;
            public int jitterCount;
            public float stepErrorSum;
            public float stepErrorMax;
            public int stepCount;
        }

        private readonly Dictionary<ZDOID, Track> m_tracks = new Dictionary<ZDOID, Track>();
        private int m_ownerChanges;
        private int m_recreated;

        private void Awake()
        {
            Instance = this;
            StartCoroutine(EndOfFrames());
        }

        public static void Run(float seconds)
        {
            if (Instance == null)
            {
                return;
            }
            Instance.Begin(seconds);
        }

        private void Begin(float seconds)
        {
            m_until = Time.time + seconds;
            m_csv = new StringBuilder("time,dt,id,kind,owner,is_owner,revision,speed,zdo_x,zdo_y,zdo_z,x,y,z,cam_x,cam_y,cam_z,player_x,player_y,player_z,attached,visible,instance\n");
            m_path = Path.Combine(Paths.BepInExRootPath, $"rail-trace-{System.DateTime.Now:yyyyMMdd-HHmmss}.csv");
            m_tracks.Clear();
            m_ownerChanges = 0;
            m_recreated = 0;
            RailPlugin.Log.LogInfo($"rail trace started for {seconds:0} s");
        }

        // A locomotive or wagon made by the scene. A train that is made again
        // while in view blinks.
        public static void Made(ZNetView view, bool made)
        {
            if (view == null || !view.IsValid())
            {
                return;
            }
            ZDO zdo = view.GetZDO();
            Player player = Player.m_localPlayer;
            float distance = player != null ? Vector3.Distance(player.transform.position, zdo.GetPosition()) : -1f;
            RailPlugin.Log.LogInfo($"{Kind(zdo)} {Train.IdText(zdo.m_uid)} {(made ? "made" : "removed")} by the scene, {distance:0} m from the player");
            if (Instance != null && Instance.m_csv != null && made)
            {
                Instance.m_recreated++;
            }
        }

        private static string Kind(ZDO zdo)
        {
            return zdo.GetPrefab() == Train.LocomotiveHash ? "locomotive" : "wagon";
        }

        private static string Peer(long uid)
        {
            if (uid == 0L)
            {
                return "nobody";
            }
            if (uid == ZDOMan.GetSessionID())
            {
                return "me";
            }
            if (ZNet.instance != null && !ZNet.instance.IsServer() && uid == Network.Server)
            {
                return "server";
            }
            return uid.ToString(CultureInfo.InvariantCulture);
        }

        private void Update()
        {
            if (ZDOMan.instance == null)
            {
                return;
            }
            foreach (ZDO loco in Train.Trains())
            {
                long owner = loco.GetOwner();
                if (m_owners.TryGetValue(loco.m_uid, out long before) && before != owner)
                {
                    Player player = Player.m_localPlayer;
                    float distance = player != null ? Vector3.Distance(player.transform.position, loco.GetPosition()) : -1f;
                    RailPlugin.Log.LogInfo($"locomotive {Train.IdText(loco.m_uid)} owner {Peer(before)} -> {Peer(owner)}, {distance:0} m from the player");
                    if (m_csv != null)
                    {
                        m_ownerChanges++;
                    }
                }
                m_owners[loco.m_uid] = owner;
            }
        }

        private IEnumerator EndOfFrames()
        {
            WaitForEndOfFrame end = new WaitForEndOfFrame();
            while (true)
            {
                yield return end;
                if (m_csv == null)
                {
                    continue;
                }
                Record();
                if (Time.time >= m_until)
                {
                    Finish();
                }
            }
        }

        private void Record()
        {
            Camera camera = Utils.GetMainCamera();
            Player player = Player.m_localPlayer;
            if (camera == null || player == null || ZNetScene.instance == null)
            {
                return;
            }
            Vector3 cam = camera.transform.position;
            Vector3 me = player.transform.position;
            foreach (ZDO loco in Train.Trains())
            {
                if ((loco.GetPosition() - me).sqrMagnitude > Radius * Radius)
                {
                    continue;
                }
                float speed = loco.GetFloat(Train.Speed);
                Row(loco, loco, speed, cam, me, player.IsAttached());
                foreach (ZDOID id in Train.WagonIds(loco))
                {
                    ZDO wagon = ZDOMan.instance.GetZDO(id);
                    if (wagon != null)
                    {
                        Row(wagon, loco, speed, cam, me, player.IsAttached());
                    }
                }
            }
        }

        private void Row(ZDO zdo, ZDO loco, float speed, Vector3 cam, Vector3 me, bool attached)
        {
            ZNetView view = ZNetScene.instance.FindInstance(zdo);
            Vector3 pos = view != null ? view.transform.position : Vector3.zero;
            Renderer renderer = view != null ? view.GetComponentInChildren<MeshRenderer>() : null;
            bool visible = renderer != null && renderer.enabled && renderer.gameObject.activeInHierarchy && renderer.isVisible;
            Vector3 saved = zdo.GetPosition();
            CultureInfo inv = CultureInfo.InvariantCulture;
            m_csv.Append(string.Join(",", new[]
            {
                Time.time.ToString("0.0000", inv), Time.deltaTime.ToString("0.0000", inv), Train.IdText(zdo.m_uid), Kind(zdo),
                Peer(zdo.GetOwner()), zdo.IsOwner() ? "1" : "0", zdo.DataRevision.ToString(inv), speed.ToString("0.000", inv),
                saved.x.ToString("0.000", inv), saved.y.ToString("0.000", inv), saved.z.ToString("0.000", inv),
                pos.x.ToString("0.000", inv), pos.y.ToString("0.000", inv), pos.z.ToString("0.000", inv),
                cam.x.ToString("0.000", inv), cam.y.ToString("0.000", inv), cam.z.ToString("0.000", inv),
                me.x.ToString("0.000", inv), me.y.ToString("0.000", inv), me.z.ToString("0.000", inv),
                attached ? "1" : "0", visible ? "1" : "0", view != null ? view.gameObject.GetInstanceID().ToString(inv) : "0",
            })).Append('\n');

            if (view == null)
            {
                return;
            }
            if (!m_tracks.TryGetValue(zdo.m_uid, out Track track))
            {
                track = new Track();
                m_tracks[zdo.m_uid] = track;
            }
            if (!visible)
            {
                track.hidden++;
            }
            Vector3 rel = pos - cam;
            Vector3 step = pos - track.lastPos;
            if (track.frames > 0)
            {
                track.maxJump = Mathf.Max(track.maxJump, step.magnitude);
                if (Mathf.Abs(speed) > 0.1f)
                {
                    // A smooth train moves speed times frame time each frame.
                    float error = Mathf.Abs(step.magnitude - Mathf.Abs(speed) * Time.deltaTime);
                    track.stepErrorSum += error;
                    track.stepErrorMax = Mathf.Max(track.stepErrorMax, error);
                    track.stepCount++;
                }
                // The train moves one way, a step against the way it faces is a
                // jump back.
                float along = Vector3.Dot(step, view.transform.forward) * Mathf.Sign(speed);
                if (Mathf.Abs(speed) > 0.1f && along < -0.01f)
                {
                    track.backward++;
                }
            }
            if (track.frames > 1)
            {
                // What the eye sees is the train against the camera. Smooth
                // motion changes that by the same step each frame, the second
                // difference shows the shake.
                float shake = ((rel - track.lastRel) - track.lastStep).magnitude;
                track.jitterSum += shake;
                track.jitterMax = Mathf.Max(track.jitterMax, shake);
                track.jitterCount++;
            }
            if (track.frames > 0)
            {
                track.lastStep = rel - track.lastRel;
            }
            track.lastRel = rel;
            track.lastPos = pos;
            track.frames++;
        }

        private void Finish()
        {
            File.WriteAllText(m_path, m_csv.ToString());
            m_csv = null;
            RailPlugin.Log.LogInfo($"rail trace written to {m_path}, {m_ownerChanges} owner changes, {m_recreated} trains made again");
            foreach (KeyValuePair<ZDOID, Track> pair in m_tracks)
            {
                Track t = pair.Value;
                float mean = t.jitterCount > 0 ? t.jitterSum / t.jitterCount : 0f;
                RailPlugin.Log.LogInfo($"  {Train.IdText(pair.Key)}: {t.frames} frames, {t.backward} steps back, {t.hidden} frames hidden, largest step {t.maxJump:0.000} m, shake against the camera mean {mean:0.0000} m max {t.jitterMax:0.000} m, step off from speed mean {(t.stepCount > 0 ? t.stepErrorSum / t.stepCount : 0f):0.0000} m max {t.stepErrorMax:0.000} m");
            }
        }

        [HarmonyPatch(typeof(Terminal), "InitTerminal")]
        private static class Command
        {
            private static void Postfix()
            {
                new Terminal.ConsoleCommand("railtrace", "[seconds] records the trains near you every frame, see docs/rail.md", args =>
                {
                    float seconds = args.Length > 1 && float.TryParse(args[1], NumberStyles.Float, CultureInfo.InvariantCulture, out float value) ? value : 15f;
                    Run(seconds);
                    args.Context?.AddString($"rail trace for {seconds:0} s, then see BepInEx/LogOutput.log");
                });
            }
        }
    }
}
