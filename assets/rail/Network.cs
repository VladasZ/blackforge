using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using HarmonyLib;
using UnityEngine;

namespace Blackforge
{
    // Requests between players, the owner of a train and the server. The
    // server knows the whole world, so it plans routes, keeps track free for
    // one train at a time and checks station names. The owner of a
    // locomotive is the only one that writes its data.
    public static class Network
    {
        private class Pending
        {
            public ZDOID loco;
            public ZDOID station;
            public long by;
        }

        private static readonly List<Pending> pending = new List<Pending>();
        private static float nextServerTick;

        public static bool IsServer => ZNet.instance != null && ZNet.instance.IsServer();

        public static long Server => ZRoutedRpc.instance.GetServerPeerID();

        public static void Register()
        {
            ZRoutedRpc rpc = ZRoutedRpc.instance;
            rpc.Register<ZDOID, ZDOID>("bf_request", OnRequest);
            rpc.Register<ZDOID, string, float, int>("bf_depart", OnDepart);
            rpc.Register<ZDOID>("bf_list", OnList);
            rpc.Register<ZDOID, string>("bf_list_result", OnListResult);
            rpc.Register<ZDOID, int>("bf_coal", OnCoal);
            rpc.Register<ZDOID, ZDOID, bool>("bf_couple", OnCouple);
            rpc.Register<ZDOID, long, bool>("bf_drive", OnDrive);
            rpc.Register<ZDOID, string>("bf_name", OnName);
            rpc.Register<ZDOID, string>("bf_set_name", OnSetName);
            rpc.Register<string>("bf_message", OnMessage);
            RailPlugin.Log.LogInfo("rail network ready");
        }

        // Sends to the machine that owns an object, which is this one when
        // nobody else does.
        public static void ToOwner(ZDOID id, string method, params object[] args)
        {
            ZDO zdo = ZDOMan.instance.GetZDO(id);
            long owner = zdo != null && zdo.HasOwner() ? zdo.GetOwner() : ZDOMan.GetSessionID();
            ZRoutedRpc.instance.InvokeRoutedRPC(owner, method, args);
        }

        public static void Tell(long peer, string text)
        {
            ZRoutedRpc.instance.InvokeRoutedRPC(peer, "bf_message", text);
        }

        private static void OnMessage(long sender, string text)
        {
            Player.m_localPlayer?.Message(MessageHud.MessageType.Center, text);
        }

        // A player asks to send a train to a station.
        private static void OnRequest(long sender, ZDOID loco, ZDOID station)
        {
            if (!IsServer)
            {
                return;
            }
            pending.RemoveAll(p => p.loco == loco);
            if (TryDepart(loco, station, sender, out string message))
            {
                return;
            }
            if (message != null)
            {
                Tell(sender, message);
                return;
            }
            pending.Add(new Pending { loco = loco, station = station, by = sender });
            Tell(sender, "The train waits until the track is free");
        }

        // Plans a route and sends the train off when its track is free. False
        // with no message means it has to wait.
        private static bool TryDepart(ZDOID locoId, ZDOID stationId, long by, out string message)
        {
            message = null;
            Graph.Rebuild();
            ZDO loco = ZDOMan.instance.GetZDO(locoId);
            ZDO station = ZDOMan.instance.GetZDO(stationId);
            if (loco == null || station == null)
            {
                message = "That train or station is gone";
                return false;
            }
            if (!Plan(loco, station, out List<Line> route, out float stop, out float length))
            {
                message = "No track leads there from the front of this train";
                return false;
            }
            int cost = Mathf.Max(1, Mathf.CeilToInt(length / Train.MetersPerCoal));
            if (loco.GetInt(Train.Coal) < cost)
            {
                message = $"The trip needs {cost} coal, the locomotive has {loco.GetInt(Train.Coal)}";
                return false;
            }
            HashSet<Line> taken = Taken(loco);
            if (route.Any(taken.Contains))
            {
                return false;
            }
            string keys = string.Join("|", route.Select(l => l.Key));
            ToOwner(locoId, "bf_depart", locoId, keys, stop, cost);
            Tell(by, $"The train leaves for {station.GetString(Station.Name)}, {cost} coal");
            return true;
        }

        // Track that other trains stand on or have reserved for their trip.
        private static HashSet<Line> Taken(ZDO self)
        {
            HashSet<Line> taken = new HashSet<Line>();
            foreach (ZDO other in Train.Trains())
            {
                if (other == self)
                {
                    continue;
                }
                foreach (Line line in Train.Covered(other))
                {
                    taken.Add(line);
                }
                if (other.GetBool(Train.Moving))
                {
                    foreach (string key in Train.ReadList(other, Train.Route))
                    {
                        Line line = Graph.Find(key);
                        if (line != null)
                        {
                            taken.Add(line);
                        }
                    }
                }
            }
            return taken;
        }

        // The shortest route from the front of a train to the stop of a
        // station, over lines in the direction the train faces.
        public static bool Plan(ZDO loco, ZDO station, out List<Line> route, out float stop, out float length)
        {
            route = new List<Line>();
            stop = 0f;
            length = 0f;
            if (!Train.Load(loco, out TrackCursor start) || !Graph.Nearest(station.GetPosition(), 3f, out Line goal, out stop))
            {
                return false;
            }
            float ahead = start.dir > 0 ? stop - start.s : start.s - stop;
            if (goal == start.line && ahead >= 0f)
            {
                length = ahead;
                return true;
            }
            Dictionary<(Line, bool), float> best = new Dictionary<(Line, bool), float>();
            Dictionary<(Line, bool), (Line, bool)> previous = new Dictionary<(Line, bool), (Line, bool)>();
            List<((Line line, bool fromA) state, float cost)> open = new List<((Line, bool), float)>();
            float first = start.dir > 0 ? start.line.Length - start.s : start.s;
            foreach (Step step in Graph.Next(start.line, start.dir > 0))
            {
                open.Add(((step.line, step.fromA), first));
                best[(step.line, step.fromA)] = first;
            }
            while (open.Count > 0)
            {
                open.Sort((a, b) => a.cost.CompareTo(b.cost));
                ((Line line, bool fromA) state, float cost) = open[0];
                open.RemoveAt(0);
                if (state.line == goal)
                {
                    length = cost + (state.fromA ? stop : state.line.Length - stop);
                    for ((Line, bool) at = state; ; at = previous[at])
                    {
                        route.Insert(0, at.Item1);
                        if (!previous.ContainsKey(at))
                        {
                            break;
                        }
                    }
                    return true;
                }
                float next = cost + state.line.Length;
                foreach (Step step in Graph.Next(state.line, state.fromA))
                {
                    (Line, bool) key = (step.line, step.fromA);
                    if (best.TryGetValue(key, out float known) && known <= next)
                    {
                        continue;
                    }
                    best[key] = next;
                    previous[key] = state;
                    open.Add((key, next));
                }
            }
            return false;
        }

        // On the owner: the server planned the trip.
        private static void OnDepart(long sender, ZDOID locoId, string route, float stop, int cost)
        {
            ZDO loco = ZDOMan.instance.GetZDO(locoId);
            if (loco == null || !loco.IsOwner())
            {
                return;
            }
            loco.Set(Train.Route, route);
            loco.Set(Train.Stop, stop);
            loco.Set(Train.Moving, true);
            loco.Set(Train.Coal, Mathf.Max(0, loco.GetInt(Train.Coal) - cost));
        }

        // Every named station with the coal a trip there costs, or why it is
        // out of reach.
        private static void OnList(long sender, ZDOID locoId)
        {
            if (!IsServer)
            {
                return;
            }
            Graph.Rebuild();
            ZDO loco = ZDOMan.instance.GetZDO(locoId);
            List<string> rows = new List<string>();
            foreach (ZDO station in Station.All())
            {
                string name = station.GetString(Station.Name);
                if (string.IsNullOrEmpty(name))
                {
                    continue;
                }
                string cost = loco != null && Plan(loco, station, out List<Line> _, out float _, out float length)
                    ? Mathf.Max(1, Mathf.CeilToInt(length / Train.MetersPerCoal)).ToString(CultureInfo.InvariantCulture)
                    : "-";
                rows.Add($"{Train.IdText(station.m_uid)}\t{name.Replace("\t", " ")}\t{cost}");
            }
            ZRoutedRpc.instance.InvokeRoutedRPC(sender, "bf_list_result", locoId, string.Join("\n", rows));
        }

        private static void OnListResult(long sender, ZDOID loco, string rows)
        {
            StationMenu.Show(loco, rows);
        }

        private static void OnCoal(long sender, ZDOID locoId, int amount)
        {
            ZDO loco = ZDOMan.instance.GetZDO(locoId);
            if (loco != null && loco.IsOwner())
            {
                loco.Set(Train.Coal, Mathf.Clamp(loco.GetInt(Train.Coal) + amount, 0, Train.MaxCoal));
            }
        }

        private static void OnCouple(long sender, ZDOID locoId, ZDOID wagon, bool add)
        {
            ZDO loco = ZDOMan.instance.GetZDO(locoId);
            if (loco == null || !loco.IsOwner())
            {
                return;
            }
            List<ZDOID> wagons = Train.WagonIds(loco);
            if (add && !wagons.Contains(wagon) && wagons.Count < Train.MaxWagons)
            {
                wagons.Add(wagon);
            }
            else if (!add && wagons.Count > 0 && wagons[wagons.Count - 1] == wagon)
            {
                wagons.RemoveAt(wagons.Count - 1);
            }
            Train.WriteList(loco, Train.Wagons, wagons.Select(Train.IdText));
            if (Train.Load(loco, out TrackCursor cursor))
            {
                Train.Pose(loco, cursor, Train.ReadList(loco, Train.Trail));
            }
        }

        // On the owner: a player sits down to drive, or gets up. The driver's
        // machine takes the train over, so the controls answer at once.
        private static void OnDrive(long sender, ZDOID locoId, long player, bool start)
        {
            ZDO loco = ZDOMan.instance.GetZDO(locoId);
            if (loco == null || !loco.IsOwner())
            {
                return;
            }
            if (start)
            {
                loco.Set(Train.Driver, player);
                loco.Set(Train.Moving, false);
                loco.Set(Train.Route, "");
                loco.SetOwner(sender);
            }
            else if (loco.GetLong(Train.Driver) == player)
            {
                loco.Set(Train.Driver, 0L);
            }
        }

        // On the server: a station name must be unique.
        private static void OnName(long sender, ZDOID stationId, string name)
        {
            if (!IsServer)
            {
                return;
            }
            name = name.Trim();
            if (name.Length > 0 && Station.All().Any(s => s.m_uid != stationId && string.Equals(s.GetString(Station.Name), name, System.StringComparison.OrdinalIgnoreCase)))
            {
                Tell(sender, $"There is already a station called {name}");
                return;
            }
            ToOwner(stationId, "bf_set_name", stationId, name);
        }

        private static void OnSetName(long sender, ZDOID stationId, string name)
        {
            ZDO station = ZDOMan.instance.GetZDO(stationId);
            if (station != null && station.IsOwner())
            {
                station.Set(Station.Name, name);
            }
        }

        // The server takes over every train nobody near it owns, and retries
        // trains that wait for free track.
        public static void ServerTick()
        {
            if (!IsServer || Time.time < nextServerTick)
            {
                return;
            }
            nextServerTick = Time.time + 1f;
            long me = ZDOMan.GetSessionID();
            foreach (ZDO loco in Train.Trains())
            {
                long owner = loco.GetOwner();
                if (owner == 0L || (owner != me && ZNet.instance.GetPeer(owner) == null))
                {
                    loco.SetOwner(me);
                }
            }
            foreach (Pending wait in pending.ToList())
            {
                if (TryDepart(wait.loco, wait.station, wait.by, out string message) || message != null)
                {
                    pending.Remove(wait);
                    if (message != null)
                    {
                        Tell(wait.by, message);
                    }
                }
            }
        }

        [HarmonyPatch(typeof(Game), "Start")]
        private static class RegisterOnStart
        {
            private static void Postfix()
            {
                Register();
                new GameObject("BlackforgeRail").AddComponent<Simulation>();
            }
        }
    }

    // Moves every train this machine owns, each frame.
    public class Simulation : MonoBehaviour
    {
        private const float Near = 80f;
        private float m_nextTake;

        // A client takes over the trains the server moves near its player, so
        // they move every frame for that player and not in network steps. The
        // game gives them back when the player leaves.
        private void TakeNearTrains()
        {
            Player player = Player.m_localPlayer;
            if (Network.IsServer || player == null || Time.time < m_nextTake)
            {
                return;
            }
            m_nextTake = Time.time + 0.5f;
            long server = Network.Server;
            foreach (ZDO loco in Train.Trains())
            {
                if (loco.GetOwner() == server && (loco.GetPosition() - player.transform.position).sqrMagnitude < Near * Near)
                {
                    loco.SetOwner(ZDOMan.GetSessionID());
                }
            }
        }

        private void Update()
        {
            if (ZNet.instance == null || ZDOMan.instance == null)
            {
                return;
            }
            Graph.Update();
            Network.ServerTick();
            TakeNearTrains();
            float dt = Time.deltaTime;
            foreach (ZDO loco in Train.Trains())
            {
                if (loco.IsOwner())
                {
                    Train.Step(loco, dt);
                }
            }
        }
    }
}
