using System;
using System.Collections;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using BepInEx.Logging;
using Newtonsoft.Json;
using UnityEngine;
using UnityEngine.Networking;

namespace Blackforge
{
    // The server half of a competitive server, see docs/competitive.md.
    //
    // The server tells the backend its world and its dead bosses, and learns
    // whether it is competitive and which materials it forbids. After a player
    // spawns, the server asks the join plugin for the whole inventory with the
    // world tag of every item. A forbidden item without the tag of this world
    // kicks the player, so does a forbidden item worn without a tagged copy in
    // the report, and so does no report at all. A clean report clears the
    // player, and only then the join plugin starts to tag new items.
    public static class Competitive
    {
        private const string CheckRpc = "BlackforgeCheck";
        private const string InventoryRpc = "BlackforgeInventory";
        private const string ClearedRpc = "BlackforgeCleared";
        private const string GateRpc = "BlackforgeGate";
        private const string BossPrefix = "defeated_";
        // A change of the flag or the tiers reaches the server this often.
        private const float ProgressInterval = 300f;
        private const float RetryInterval = 30f;
        private const float ReportTimeout = 30f;
        private const int PostTimeout = 10;
        private const string NoReport = "This server checks the items of every player. Update Blackforge and start Valheim from it.";

        private static readonly int[] Worn =
        {
            ZDOVars.s_rightItem, ZDOVars.s_leftItem, ZDOVars.s_chestItem, ZDOVars.s_legItem,
            ZDOVars.s_helmetItem, ZDOVars.s_shoulderItem, ZDOVars.s_utilityItem, ZDOVars.s_trinketItem,
        };

        private class ProgressBody
        {
            public string server;
            public string world;
            public List<string> keys;
        }

        private class RulesAnswer
        {
            public bool competitive { get; set; }
            public List<ForbiddenItem> forbidden { get; set; }
        }

        // One item of the report, the prefab name and the world tag.
        private class Carried
        {
            public string n { get; set; }
            public string t { get; set; }
        }

        private class Check
        {
            public float asked;
            // Prefab names the player carries with the tag of this world.
            public HashSet<string> tagged;
            public bool cleared;
        }

        private static MonoBehaviour host;
        private static ManualLogSource log;
        private static string gateUrl;
        private static string gateSecret;

        private static bool competitive;
        private static Dictionary<string, ForbiddenItem> forbidden = new Dictionary<string, ForbiddenItem>();
        private static string world;
        private static string sentKeys;
        private static float nextPost;
        private static bool posting;
        private static bool failed;
        private static readonly Dictionary<ZNetPeer, Check> checks = new Dictionary<ZNetPeer, Check>();

        public static void Init(MonoBehaviour plugin, ManualLogSource logger, string url, string secret)
        {
            host = plugin;
            log = logger;
            gateUrl = url;
            gateSecret = secret;
        }

        // A postfix of ZNet.OnNewConnection.
        public static void Listen(ZNet __instance, ZNetPeer peer)
        {
            if (__instance.IsServer())
            {
                ZNet net = __instance;
                peer.m_rpc.Register<string>(InventoryRpc, (rpc, json) => OnReport(net, peer, json));
            }
        }

        public static void Tick(ZNet net)
        {
            Progress();
            foreach (ZNetPeer gone in checks.Keys.Where(peer => !net.m_peers.Contains(peer)).ToList())
            {
                checks.Remove(gone);
            }
            if (!competitive)
            {
                return;
            }
            foreach (ZNetPeer peer in net.GetConnectedPeers())
            {
                // A player in character select or on the way in has no body yet.
                if (peer.m_characterID.IsNone() || ZNet.PeersToDisconnectAfterKick.ContainsKey(peer))
                {
                    continue;
                }
                if (!checks.TryGetValue(peer, out Check check))
                {
                    checks[peer] = new Check { asked = Time.time };
                    peer.m_rpc.Invoke(CheckRpc, world ?? "");
                    continue;
                }
                if (check.tagged == null)
                {
                    if (Time.time - check.asked > ReportTimeout)
                    {
                        Kick(net, peer, NoReport, "sent no inventory");
                    }
                    continue;
                }
                CheckWorn(net, peer, check);
            }
        }

        private static void Progress()
        {
            ZoneSystem zones = ZoneSystem.instance;
            if (posting || zones == null || ZNet.World == null)
            {
                return;
            }
            List<string> keys = zones.m_globalKeys
                .Where(key => key.StartsWith(BossPrefix, StringComparison.OrdinalIgnoreCase))
                .OrderBy(key => key)
                .ToList();
            string joined = string.Join(",", keys);
            // A boss death posts at once, unless the last post failed.
            if (Time.time < nextPost && (joined == sentKeys || failed))
            {
                return;
            }
            posting = true;
            host.StartCoroutine(Post(new ProgressBody
            {
                server = ZNet.m_ServerName,
                world = ZNet.World.m_uid.ToString(),
                keys = keys,
            }, joined));
        }

        private static IEnumerator Post(ProgressBody body, string joined)
        {
            // Counts as failed until the answer is read, so a failure of any
            // kind waits for the retry. The old rules stay until then.
            failed = true;
            nextPost = Time.time + RetryInterval;
            try
            {
                using (UnityWebRequest request = new UnityWebRequest(gateUrl + "/api/gate/progress", "POST"))
                {
                    request.uploadHandler = new UploadHandlerRaw(Encoding.UTF8.GetBytes(JsonConvert.SerializeObject(body)));
                    request.downloadHandler = new DownloadHandlerBuffer();
                    request.SetRequestHeader("Content-Type", "application/json");
                    request.SetRequestHeader("Authorization", "Bearer " + gateSecret);
                    request.timeout = PostTimeout;
                    yield return request.SendWebRequest();
                    if (request.result != UnityWebRequest.Result.Success || request.responseCode != 200)
                    {
                        log.LogWarning($"the progress did not reach blackforge: {request.responseCode} {request.error}");
                        yield break;
                    }
                    RulesAnswer rules = JsonConvert.DeserializeObject<RulesAnswer>(request.downloadHandler.text);
                    bool changed = sentKeys != joined || competitive != rules.competitive;
                    sentKeys = joined;
                    world = body.world;
                    competitive = rules.competitive;
                    forbidden = Tiers.Expand(rules.forbidden, log.LogWarning);
                    failed = false;
                    nextPost = Time.time + ProgressInterval;
                    if (changed)
                    {
                        log.LogInfo(competitive
                            ? $"competitive with bosses [{joined}], {forbidden.Count} items forbidden"
                            : $"not competitive, bosses [{joined}]");
                    }
                }
            }
            finally
            {
                // An exception thrown in here still frees the next post.
                posting = false;
            }
        }

        private static void OnReport(ZNet net, ZNetPeer peer, string json)
        {
            if (!competitive || !checks.TryGetValue(peer, out Check check))
            {
                return;
            }
            List<Carried> carried;
            try
            {
                carried = JsonConvert.DeserializeObject<List<Carried>>(json) ?? new List<Carried>();
            }
            catch (Exception error)
            {
                Kick(net, peer, NoReport, $"sent a bad inventory: {error.Message}");
                return;
            }
            List<string> bad = carried
                .Where(item => item.n != null && forbidden.ContainsKey(item.n) && item.t != world)
                .Select(item => item.n)
                .Distinct()
                .ToList();
            if (bad.Count > 0)
            {
                Kick(net, peer, Refusal(bad), "carries " + string.Join(", ", bad));
                return;
            }
            check.tagged = new HashSet<string>(carried.Where(item => item.t == world).Select(item => item.n));
            if (!check.cleared)
            {
                check.cleared = true;
                log.LogInfo($"{peer.m_playerName} carries nothing forbidden");
                peer.m_rpc.Invoke(ClearedRpc, world);
            }
        }

        // The worn items come from the body of the player, which every client
        // sees. A forbidden one needs a tagged copy in the report.
        private static void CheckWorn(ZNet net, ZNetPeer peer, Check check)
        {
            ZDO body = ZDOMan.instance.GetZDO(peer.m_characterID);
            if (body == null)
            {
                return;
            }
            List<string> bad = new List<string>();
            foreach (int slot in Worn)
            {
                int hash = body.GetInt(slot);
                GameObject prefab = hash == 0 ? null : ObjectDB.instance.GetItemPrefab(hash);
                if (prefab != null && forbidden.ContainsKey(prefab.name) && !check.tagged.Contains(prefab.name))
                {
                    bad.Add(prefab.name);
                }
            }
            if (bad.Count > 0)
            {
                Kick(net, peer, Refusal(bad), "wears " + string.Join(", ", bad));
            }
        }

        private static string Refusal(List<string> items)
        {
            IEnumerable<string> named = items.Take(8).Select(item => $"{Tiers.Title(item)} (needs {forbidden[item].boss})");
            string more = items.Count > 8 ? $" and {items.Count - 8} more" : "";
            return $"{ZNet.m_ServerName} is competitive. You carry items this world has not reached: {string.Join(", ", named)}{more}.";
        }

        private static void Kick(ZNet net, ZNetPeer peer, string text, string why)
        {
            log.LogInfo($"{peer.m_playerName} is kicked, {why}");
            checks.Remove(peer);
            peer.m_rpc.Invoke(GateRpc, text);
            net.InternalKick(peer);
        }
    }
}
