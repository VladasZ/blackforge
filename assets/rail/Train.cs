using System.Collections.Generic;
using System.Linq;
using UnityEngine;

namespace Blackforge
{
    // The state of a train lives in the saved data of its locomotive, so any
    // machine that owns the locomotive can move it: the client of a player
    // near it, or the server when nobody is near.
    public static class Train
    {
        public const string Line = "bf_line";
        public const string At = "bf_at";
        public const string Dir = "bf_dir";
        public const string Trail = "bf_trail";
        public const string Route = "bf_route";
        public const string Stop = "bf_stop";
        public const string Moving = "bf_moving";
        public const string Want = "bf_want";
        public const string WantBy = "bf_want_by";
        public const string Wagons = "bf_wagons";
        public const string Coal = "bf_coal";
        public const string Odometer = "bf_odo";
        public const string Speed = "bf_speed";
        public const string Driver = "bf_driver";

        public const int MaxWagons = 3;
        public const int MaxCoal = 100;
        public const float MetersPerCoal = 100f;
        public const float AutoSpeed = 6f;
        public const float DriveSpeed = 10f;
        public const float ReverseSpeed = 4f;
        public const float Accel = 2f;
        public const float Brake = 4f;
        // From the center of the locomotive to the center of the first wagon,
        // and between wagon centers, from the models.
        public const float FirstGap = 4.7f;
        public const float WagonGap = 4.1f;
        private const int TrailLength = 24;

        public static readonly int LocomotiveHash = "bf_locomotive".GetStableHashCode();
        public static readonly int WagonHash = "bf_wagon".GetStableHashCode();

        // The throttle of the local driver, set by the seat.
        public static readonly Dictionary<ZDOID, (float throttle, int side)> Controls = new Dictionary<ZDOID, (float, int)>();

        public static bool Load(ZDO zdo, out TrackCursor cursor)
        {
            cursor = default;
            Line line = Graph.Find(zdo.GetString(Line));
            if (line == null)
            {
                return false;
            }
            cursor = new TrackCursor { line = line, s = zdo.GetFloat(At), dir = zdo.GetInt(Dir, 1) };
            return true;
        }

        public static void Save(ZDO zdo, TrackCursor cursor)
        {
            zdo.Set(Line, cursor.line.Key);
            zdo.Set(At, cursor.s);
            zdo.Set(Dir, cursor.dir);
        }

        // A new locomotive or wagon takes the track under it.
        public static bool Place(ZDO zdo, Transform transform)
        {
            if (!Graph.Nearest(transform.position, 1.5f, out Line line, out float s))
            {
                return false;
            }
            line.At(s, out Vector3 _, out Vector3 forward);
            int dir = Vector3.Dot(forward, transform.forward) >= 0f ? 1 : -1;
            Save(zdo, new TrackCursor { line = line, s = s, dir = dir });
            return true;
        }

        public static List<string> ReadList(ZDO zdo, string key)
        {
            string text = zdo.GetString(key);
            return string.IsNullOrEmpty(text) ? new List<string>() : text.Split('|').ToList();
        }

        public static void WriteList(ZDO zdo, string key, IEnumerable<string> items)
        {
            zdo.Set(key, string.Join("|", items));
        }

        public static List<ZDOID> WagonIds(ZDO zdo)
        {
            return ReadList(zdo, Wagons).Select(ParseId).Where(id => id != ZDOID.None).ToList();
        }

        public static string IdText(ZDOID id)
        {
            return id.UserID + ":" + id.ID;
        }

        public static ZDOID ParseId(string text)
        {
            string[] parts = text.Split(':');
            return parts.Length == 2 && long.TryParse(parts[0], out long user) && uint.TryParse(parts[1], out uint id) ? new ZDOID(user, id) : ZDOID.None;
        }

        // The lines a train covers from its locomotive back past its last
        // wagon, which no other train may enter.
        public static List<Line> Covered(ZDO zdo)
        {
            List<Line> lines = new List<Line>();
            if (!Load(zdo, out TrackCursor cursor))
            {
                return lines;
            }
            lines.Add(cursor.line);
            float length = 3f + WagonIds(zdo).Count * WagonGap + FirstGap;
            Queue<string> trail = new Queue<string>(ReadList(zdo, Trail));
            cursor.Walk(-length, (steps, backward) => FromTrail(steps, trail, cursor.Forward), (from, backward) => lines.Add(from));
            return lines;
        }

        private static Step? FromTrail(List<Step> steps, Queue<string> trail, Vector3 heading)
        {
            if (trail.Count > 0)
            {
                string key = trail.Dequeue();
                foreach (Step step in steps)
                {
                    if (step.line.Key == key || (Graph.Decode(key, out Vector3 a, out Vector3 b) && step.line.Matches(a, b)))
                    {
                        return step;
                    }
                }
            }
            return TrackCursor.Straightest(steps, -heading, 0);
        }

        // One tick of a train this machine owns.
        public static void Step(ZDO zdo, float dt)
        {
            if (!Load(zdo, out TrackCursor cursor))
            {
                return;
            }
            float speed = zdo.GetFloat(Speed);
            Controls.TryGetValue(zdo.m_uid, out (float throttle, int side) control);
            bool driven = zdo.GetLong(Driver) != 0L;
            int coal = zdo.GetInt(Coal);
            List<string> route = ReadList(zdo, Route);
            bool moving = zdo.GetBool(Moving);
            if (!driven && !moving && speed == 0f)
            {
                return;
            }

            float target = 0f;
            if (driven)
            {
                target = control.throttle > 0f ? DriveSpeed * control.throttle : control.throttle < 0f ? ReverseSpeed * control.throttle : 0f;
                if (coal <= 0)
                {
                    target = 0f;
                }
            }
            else if (moving)
            {
                float left = RemainingDistance(cursor, route, zdo.GetFloat(Stop));
                target = Mathf.Min(AutoSpeed, Mathf.Sqrt(2f * Brake * Mathf.Max(left - 0.2f, 0f)));
                if (left < 0.3f)
                {
                    Arrive(zdo);
                    target = 0f;
                    speed = 0f;
                }
            }
            float rate = Mathf.Abs(target) > Mathf.Abs(speed) && Mathf.Sign(target) == Mathf.Sign(speed) ? Accel : Brake;
            speed = Mathf.MoveTowards(speed, target, rate * dt);

            List<string> trail = ReadList(zdo, Trail);
            HashSet<Line> blocked = Blocked(zdo);
            float ds = speed * dt;
            TrackCursor next = cursor;
            float missing = next.Walk(ds, (steps, backward) =>
            {
                steps = steps.Where(s => !blocked.Contains(s.line)).ToList();
                if (steps.Count == 0)
                {
                    return null;
                }
                if (backward)
                {
                    Step? back = FromTrail(steps, new Queue<string>(trail), -next.Forward);
                    return back;
                }
                if (moving && !driven && route.Count > 0)
                {
                    foreach (Step step in steps)
                    {
                        if (step.line.Key == route[0] || (Graph.Decode(route[0], out Vector3 a, out Vector3 b) && step.line.Matches(a, b)))
                        {
                            return step;
                        }
                    }
                    return null;
                }
                return TrackCursor.Straightest(steps, next.Forward, control.side);
            }, (from, backward) =>
            {
                if (backward)
                {
                    if (trail.Count > 0)
                    {
                        trail.RemoveAt(0);
                    }
                }
                else
                {
                    trail.Insert(0, from.Key);
                    if (trail.Count > TrailLength)
                    {
                        trail.RemoveAt(trail.Count - 1);
                    }
                    if (route.Count > 0)
                    {
                        route.RemoveAt(0);
                    }
                }
            });
            if (missing > 1e-4f)
            {
                // The end of the track, a gap or another train: stop there and
                // wait, the route stays.
                speed = 0f;
            }

            float moved = Mathf.Abs(ds) - missing;
            if (driven && moved > 0f)
            {
                float odometer = zdo.GetFloat(Odometer) + moved;
                while (odometer >= MetersPerCoal && coal > 0)
                {
                    odometer -= MetersPerCoal;
                    coal--;
                }
                zdo.Set(Odometer, odometer);
                zdo.Set(Coal, coal);
            }

            Save(zdo, next);
            WriteList(zdo, Trail, trail);
            WriteList(zdo, Route, route);
            zdo.Set(Speed, speed);
            Pose(zdo, next, trail);
        }

        // Puts the locomotive and its wagons where the cursor says, the
        // wagons walked back along the trail.
        public static void Pose(ZDO zdo, TrackCursor head, List<string> trail)
        {
            zdo.SetPosition(head.Position);
            zdo.SetRotation(Quaternion.LookRotation(head.Forward, Vector3.up));
            List<ZDOID> wagons = WagonIds(zdo);
            TrackCursor back = head;
            Queue<string> queue = new Queue<string>(trail);
            for (int i = 0; i < wagons.Count; i++)
            {
                back.Walk(-(i == 0 ? FirstGap : WagonGap), (steps, backward) => FromTrail(steps, queue, -back.Forward));
                ZDO wagon = ZDOMan.instance.GetZDO(wagons[i]);
                if (wagon == null)
                {
                    continue;
                }
                if (!wagon.IsOwner())
                {
                    wagon.SetOwner(ZDOMan.GetSessionID());
                }
                wagon.SetPosition(back.Position);
                wagon.SetRotation(Quaternion.LookRotation(back.Forward, Vector3.up));
                Save(wagon, back);
            }
        }

        // The track left to the stop, which is a distance from the A end of the
        // last line of the route.
        private static float RemainingDistance(TrackCursor cursor, List<string> route, float stop)
        {
            if (route.Count == 0)
            {
                return cursor.dir > 0 ? stop - cursor.s : cursor.s - stop;
            }
            float left = cursor.dir > 0 ? cursor.line.Length - cursor.s : cursor.s;
            Vector3 end = cursor.dir > 0 ? cursor.line.B : cursor.line.A;
            for (int i = 0; i < route.Count; i++)
            {
                Line line = Graph.Find(route[i]);
                if (line == null)
                {
                    return left + 50f;
                }
                bool fromA = (line.A - end).sqrMagnitude < (line.B - end).sqrMagnitude;
                if (i == route.Count - 1)
                {
                    return left + (fromA ? stop : line.Length - stop);
                }
                left += line.Length;
                end = fromA ? line.B : line.A;
            }
            return left;
        }

        private static void Arrive(ZDO zdo)
        {
            zdo.Set(Moving, false);
            zdo.Set(Route, "");
        }

        // Lines covered by other trains near this one, which it must not
        // enter.
        private static HashSet<Line> Blocked(ZDO self)
        {
            HashSet<Line> blocked = new HashSet<Line>();
            foreach (ZDO other in Trains())
            {
                if (other == self || (other.GetPosition() - self.GetPosition()).sqrMagnitude > 200f * 200f)
                {
                    continue;
                }
                foreach (Line line in Covered(other))
                {
                    blocked.Add(line);
                }
            }
            return blocked;
        }

        private static List<ZDO> trains = new List<ZDO>();
        private static float nextScan;

        // Every locomotive this machine knows.
        public static List<ZDO> Trains()
        {
            if (Time.time >= nextScan && ZDOMan.instance != null)
            {
                nextScan = Time.time + 1f;
                trains = ZDOMan.instance.m_objectsByID.Values.Where(z => z.GetPrefab() == LocomotiveHash).ToList();
            }
            trains.RemoveAll(z => !z.IsValid());
            return trains;
        }
    }
}
