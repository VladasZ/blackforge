using System.Collections;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using BepInEx;
using HarmonyLib;
using UnityEngine;

namespace Blackforge
{
    // A test the game runs by itself, to see trains the way a player does
    // with nobody at the keyboard. It runs when BepInEx/rail-test.txt exists:
    // the main menu starts the local world RailTest with the character
    // Railtest, both made when missing. In the world it builds a loop of track
    // in the air, puts a locomotive with 3 wagons on it and takes rail shots,
    // see RailShot, of the train standing, passing by, from the seat, with
    // the material noise off and with the bend off. Then it writes
    // BepInEx/rail-test-done.txt and quits the game. See docs/rail.md.
    public class RailAutotest : MonoBehaviour
    {
        private const string WorldName = "RailTest";
        private const string Character = "railtest";

        // The throttle the test drives with, the seat takes it over the keys.
        public static float? Throttle;

        private static string Trigger => Path.Combine(Paths.BepInExRootPath, "rail-test.txt");
        private static string Done => Path.Combine(Paths.BepInExRootPath, "rail-test-done.txt");

        public static bool Active => File.Exists(Trigger);

        private static readonly List<string> report = new List<string>();

        private static void Note(string line)
        {
            RailPlugin.Log.LogInfo($"rail test: {line}");
            report.Add(line);
        }

        [HarmonyPatch(typeof(FejdStartup), "Start")]
        private static class StartWorld
        {
            private static void Postfix(FejdStartup __instance)
            {
                if (Active && Game.instance == null)
                {
                    __instance.StartCoroutine(Enter(__instance));
                }
            }
        }

        private static IEnumerator Enter(FejdStartup menu)
        {
            yield return new WaitForSeconds(3f);
            if (File.Exists(Done))
            {
                File.Delete(Done);
            }
            PlayerProfile profile = new PlayerProfile(Character, FileHelpers.FileSource.Local);
            if (!PlayerProfile.HaveProfile(Character) || !profile.Load())
            {
                profile.SetName("Railtest");
            }
            // No intro flight with the valkyrie.
            profile.m_firstSpawn = false;
            profile.Save();
            Game.SetProfile(Character, FileHelpers.FileSource.Local);
            World world = World.GetCreateWorld(WorldName, FileHelpers.FileSource.Local);
            ZNet.m_onlineBackend = OnlineBackendType.Steamworks;
            ZNet.SetServer(true, false, false, world.m_name, "", world);
            ZNet.ResetServerHost();
            Note($"starting world {world.m_name}");
            menu.m_startingWorld = true;
            menu.LoadMainScene();
        }

        [HarmonyPatch(typeof(Game), "Start")]
        private static class StartTest
        {
            private static void Postfix(Game __instance)
            {
                if (Active)
                {
                    __instance.gameObject.AddComponent<RailAutotest>();
                }
            }
        }

        private void Start()
        {
            StartCoroutine(Run());
        }

        private IEnumerator Run()
        {
            while (Player.m_localPlayer == null)
            {
                yield return null;
            }
            // At once, the last run may have left the character high in the air,
            // and a fall kills it.
            Player player = Player.m_localPlayer;
            player.SetGodMode(true);
            if (!player.InDebugFlyMode())
            {
                player.ToggleDebugFly();
            }
            while (player.IsTeleporting() || ZNetScene.instance == null || Game.instance.m_inIntro || Valkyrie.m_instance != null)
            {
                yield return new WaitForSeconds(1f);
            }
            // The zones around the spawn load.
            yield return new WaitForSeconds(8f);

            // Rail objects of earlier runs, gone so the shots show only this train.
            HashSet<int> rail = new HashSet<int>(Pieces.Specs.Select(s => s.prefab.GetStableHashCode()));
            int cleared = 0;
            foreach (ZDO old in ZDOMan.instance.m_objectsByID.Values.Where(z => rail.Contains(z.GetPrefab())).ToList())
            {
                old.SetOwner(ZDOMan.GetSessionID());
                ZDOMan.instance.DestroyZDO(old);
                cleared++;
            }
            Note($"cleared {cleared} rail objects of earlier runs");
            yield return new WaitForSeconds(2f);

            // High above the trees, so the sky is behind the train.
            Vector3 origin = player.transform.position;
            float ground = Mathf.Max(ZoneSystem.instance.GetGroundHeight(origin), ZoneSystem.instance.m_waterLevel);
            Vector3 start = new Vector3(origin.x, ground + 80f, origin.z);
            List<(Vector3 point, Vector3 heading)> ends = BuildLoop(start, Vector3.forward);
            Note($"loop of {ends.Count} pieces at {start}, it closes within {Vector3.Distance(ends.Last().point, start):0.000} m");
            yield return new WaitForSeconds(2f);
            Graph.Rebuild();

            // The train on the first straight, the locomotive in front.
            Vector3 forward = Vector3.forward;
            ZDO loco = Spawn("bf_locomotive", start + forward * 14f, forward);
            List<ZDO> wagons = new List<ZDO>();
            float back = 14f - Train.FirstGap;
            for (int i = 0; i < Train.MaxWagons; i++)
            {
                wagons.Add(Spawn("bf_wagon", start + forward * back, forward));
                back -= Train.WagonGap;
            }
            yield return new WaitForSeconds(2f);
            long me = ZDOMan.GetSessionID();
            foreach (ZDO wagon in wagons)
            {
                ZRoutedRpc.instance.InvokeRoutedRPC(me, "bf_couple", loco.m_uid, wagon.m_uid, true);
            }
            loco.Set(Train.Coal, Train.MaxCoal);
            loco.Set(Train.Driver, me);
            Drive(loco, 0f);
            RailShot.Target = loco.m_uid;
            yield return new WaitForSeconds(2f);
            Note($"train {Train.IdText(loco.m_uid)} with {Train.WagonIds(loco).Count} wagons, on the track: {Train.Load(loco, out TrackCursor _)}");

            // A side view of the first straight, from outside the loop.
            Vector3 center = ends.Aggregate(Vector3.zero, (sum, end) => sum + end.point) / ends.Count;
            Vector3 middle = start + forward * 8f;
            Vector3 outward = Vector3.ProjectOnPlane(middle - center, Vector3.up).normalized;
            Vector3 side = middle + outward * 15f + Vector3.up * 2f;

            // The whole screen, and the test character hidden, so nothing but
            // the train and the world is in the pictures.
            RailShot.FullScreen = true;
            foreach (Renderer renderer in player.GetComponentsInChildren<Renderer>())
            {
                renderer.enabled = false;
            }
            Vector3 close = middle + outward * 7f + Vector3.up * 1.5f;
            Drive(loco, 1f);
            // Our own train, then a train that acts like one the server moves,
            // drawn the old way and with the follower.
            (string label, bool remote, bool follow)[] cases =
            {
                ("own", false, true),
                ("server-old", true, false),
                ("server-follow", true, true),
            };
            foreach ((string label, bool remote, bool follow) in cases)
            {
                Simulation.Remote = remote;
                Follow.Enabled = follow;
                Place(player, close, middle);
                yield return PassBy(loco, middle, label);
            }
            Simulation.Remote = false;
            Follow.Enabled = true;
            RailShot.FullScreen = false;
            Simulation.SameFrame = true;
            RailShot.Target = ZDOID.None;

            Drive(loco, 0f);
            Note("done");
            File.WriteAllLines(Done, report);
            yield return new WaitForSeconds(3f);
            Application.Quit();
        }

        private static void Drive(ZDO loco, float throttle)
        {
            Throttle = throttle;
            Train.Controls[loco.m_uid] = (throttle, 0);
        }

        private static void SetMaterial(string property, float value)
        {
            foreach (Material material in Pieces.Moving)
            {
                material.SetFloat(property, value);
            }
        }

        // The player hangs in the air in fly mode and looks at a point.
        private static void Place(Player player, Vector3 at, Vector3 look)
        {
            player.transform.position = at;
            player.m_body.position = at;
            player.m_body.linearVelocity = Vector3.zero;
            Vector3 dir = look - at;
            player.SetLookDir(dir);
            player.m_lookPitch = Mathf.Asin(-dir.normalized.y) * Mathf.Rad2Deg;
        }

        // Waits until the train comes toward the point, then shoots it going by.
        private IEnumerator PassBy(ZDO loco, Vector3 point, string label)
        {
            float waited = 0f;
            float last = float.MaxValue;
            while (waited < 60f)
            {
                float distance = Vector3.Distance(loco.GetPosition(), point);
                if (distance < 22f && distance < last)
                {
                    break;
                }
                last = distance;
                waited += 0.05f;
                yield return new WaitForSeconds(0.05f);
            }
            yield return Shot(label, 3f);
        }

        private IEnumerator Shot(string label, float seconds)
        {
            RailTrace.Run(seconds);
            RailShot.Run(seconds, label);
            yield return null;
            while (RailShot.Busy)
            {
                yield return null;
            }
            Note($"shot {label}");
        }

        private static ZDO Spawn(string prefab, Vector3 at, Vector3 forward)
        {
            GameObject made = Instantiate(ZNetScene.instance.GetPrefab(prefab), at, Quaternion.LookRotation(forward, Vector3.up));
            WearNTear wear = made.GetComponent<WearNTear>();
            if (wear != null)
            {
                // The loop hangs in the air, nothing holds it there.
                wear.m_noSupportWear = false;
            }
            return made.GetComponent<ZNetView>().GetZDO();
        }

        // Two straights of 16 m joined by two half circles of 16 m radius,
        // each piece put on the end of the one before.
        private static List<(Vector3 point, Vector3 heading)> BuildLoop(Vector3 start, Vector3 heading)
        {
            string[] half = Enumerable.Repeat("bf_track_straight_4", 4).Concat(Enumerable.Repeat("bf_track_curve_r16_45", 4)).ToArray();
            List<(Vector3, Vector3)> ends = new List<(Vector3, Vector3)>();
            Vector3 point = start;
            foreach (string prefab in half.Concat(half))
            {
                Vector3[] line = Models.Load(prefab.Substring(3)).lines[0];
                Vector3 first = Tangent(line[0], line[1], line.Length > 2 ? line[2] : line[1]);
                Vector3 last = -Tangent(line[line.Length - 1], line[line.Length - 2], line.Length > 2 ? line[line.Length - 3] : line[line.Length - 2]);
                Quaternion rotation = Quaternion.LookRotation(heading, Vector3.up) * Quaternion.Inverse(Quaternion.LookRotation(first, Vector3.up));
                Vector3 position = point - rotation * line[0];
                GameObject made = Instantiate(ZNetScene.instance.GetPrefab(prefab), position, rotation);
                WearNTear wear = made.GetComponent<WearNTear>();
                if (wear != null)
                {
                    wear.m_noSupportWear = false;
                }
                point = position + rotation * line[line.Length - 1];
                heading = rotation * last;
                ends.Add((point, heading));
            }
            return ends;
        }

        // The direction of a line at its end point a. The first segment of a
        // curve points half a segment off the curve, the next one corrects it.
        private static Vector3 Tangent(Vector3 a, Vector3 b, Vector3 c)
        {
            Vector3 one = (b - a).normalized;
            Vector3 two = (c - b).normalized;
            return two == one ? one : (1.5f * one - 0.5f * two).normalized;
        }
    }
}
