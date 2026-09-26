using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using UnityEngine;

namespace Blackforge
{
    // One centerline of one placed track piece, in world space.
    public class Line
    {
        public ZDOID piece;
        public Vector3[] points;
        public float[] along;

        public Vector3 A => points[0];
        public Vector3 B => points[points.Length - 1];
        public float Length => along[along.Length - 1];

        public Line(ZDOID piece, Vector3[] points)
        {
            this.piece = piece;
            this.points = points;
            along = new float[points.Length];
            for (int i = 1; i < points.Length; i++)
            {
                along[i] = along[i - 1] + Vector3.Distance(points[i - 1], points[i]);
            }
        }

        // The point at a distance from A, and the direction from A to B there.
        public void At(float s, out Vector3 position, out Vector3 forward)
        {
            s = Mathf.Clamp(s, 0f, Length);
            int i = 1;
            while (i < points.Length - 1 && along[i] < s)
            {
                i++;
            }
            float span = Mathf.Max(along[i] - along[i - 1], 1e-6f);
            position = Vector3.Lerp(points[i - 1], points[i], (s - along[i - 1]) / span);
            forward = (points[i] - points[i - 1]).normalized;
        }

        // The direction of travel when leaving this line at one end.
        public Vector3 Leaving(bool atB)
        {
            int n = points.Length;
            return atB ? (points[n - 1] - points[n - 2]).normalized : (points[0] - points[1]).normalized;
        }

        public string Key => Graph.Encode(A) + ";" + Graph.Encode(B);

        public bool Matches(Vector3 a, Vector3 b)
        {
            return (A - a).sqrMagnitude < Graph.Join * Graph.Join && (B - b).sqrMagnitude < Graph.Join * Graph.Join;
        }
    }

    // Where the train goes next at the end of a line: onto another line,
    // entering it at A or at B.
    public struct Step
    {
        public Line line;
        public bool fromA;
    }

    // Every placed track piece this machine knows, as lines joined end to end.
    // It is built from the saved data of the pieces, so it also covers pieces
    // that are not loaded. The server knows the whole world, a client the area
    // around its player.
    public static class Graph
    {
        public const float Join = 0.3f;
        private const float Refresh = 1f;

        private class Piece
        {
            public Vector3 position;
            public Quaternion rotation;
            public List<Line> lines;
        }

        private static readonly Dictionary<ZDOID, Piece> pieces = new Dictionary<ZDOID, Piece>();
        private static readonly Dictionary<Vector3Int, List<(Line line, bool atB)>> ends = new Dictionary<Vector3Int, List<(Line, bool)>>();
        private static Dictionary<int, string> trackPrefabs;
        private static float nextRefresh;

        public static IEnumerable<Line> Lines => pieces.Values.SelectMany(p => p.lines);

        public static void Update()
        {
            if (Time.time < nextRefresh || ZDOMan.instance == null)
            {
                return;
            }
            nextRefresh = Time.time + Refresh;
            Rebuild();
        }

        public static void Rebuild()
        {
            if (trackPrefabs == null)
            {
                trackPrefabs = Pieces.Specs.Where(s => s.model.StartsWith("track_")).ToDictionary(s => s.prefab.GetStableHashCode(), s => s.model);
            }
            HashSet<ZDOID> seen = new HashSet<ZDOID>();
            bool changed = false;
            foreach (ZDO zdo in ZDOMan.instance.m_objectsByID.Values)
            {
                if (!trackPrefabs.TryGetValue(zdo.GetPrefab(), out string model))
                {
                    continue;
                }
                seen.Add(zdo.m_uid);
                Vector3 position = zdo.GetPosition();
                Quaternion rotation = zdo.GetRotation();
                if (pieces.TryGetValue(zdo.m_uid, out Piece known) && known.position == position && known.rotation == rotation)
                {
                    continue;
                }
                Matrix4x4 world = Matrix4x4.TRS(position, rotation, Vector3.one);
                pieces[zdo.m_uid] = new Piece
                {
                    position = position,
                    rotation = rotation,
                    lines = Models.Load(model).lines.Select(l => new Line(zdo.m_uid, l.Select(p => world.MultiplyPoint3x4(p)).ToArray())).ToList(),
                };
                changed = true;
            }
            foreach (ZDOID gone in pieces.Keys.Where(id => !seen.Contains(id)).ToList())
            {
                pieces.Remove(gone);
                changed = true;
            }
            if (changed)
            {
                ends.Clear();
                foreach (Line line in Lines)
                {
                    AddEnd(line, false);
                    AddEnd(line, true);
                }
            }
        }

        private static Vector3Int Cell(Vector3 p)
        {
            return new Vector3Int(Mathf.FloorToInt(p.x), Mathf.FloorToInt(p.y), Mathf.FloorToInt(p.z));
        }

        private static void AddEnd(Line line, bool atB)
        {
            Vector3Int cell = Cell(atB ? line.B : line.A);
            if (!ends.TryGetValue(cell, out List<(Line, bool)> list))
            {
                list = new List<(Line, bool)>();
                ends[cell] = list;
            }
            list.Add((line, atB));
        }

        // The lines a train can go on to when it leaves a line at one end. A
        // line going back the way the train came is not one of them, so the
        // two lines of a switch never lead into each other.
        public static List<Step> Next(Line line, bool atB)
        {
            Vector3 end = atB ? line.B : line.A;
            Vector3 heading = line.Leaving(atB);
            List<Step> steps = new List<Step>();
            Vector3Int cell = Cell(end);
            for (int x = -1; x <= 1; x++)
            {
                for (int y = -1; y <= 1; y++)
                {
                    for (int z = -1; z <= 1; z++)
                    {
                        if (!ends.TryGetValue(cell + new Vector3Int(x, y, z), out List<(Line line, bool atB)> list))
                        {
                            continue;
                        }
                        foreach ((Line other, bool otherAtB) in list)
                        {
                            if (other == line || ((otherAtB ? other.B : other.A) - end).sqrMagnitude > Join * Join)
                            {
                                continue;
                            }
                            // Entering at A runs toward B, entering at B runs toward A.
                            Vector3 onward = otherAtB ? -other.Leaving(true) : -other.Leaving(false);
                            if (Vector3.Dot(heading, onward) > 0.5f)
                            {
                                steps.Add(new Step { line = other, fromA = !otherAtB });
                            }
                        }
                    }
                }
            }
            return steps;
        }

        public static Line Find(Vector3 a, Vector3 b)
        {
            Vector3Int cell = Cell(a);
            for (int x = -1; x <= 1; x++)
            {
                for (int y = -1; y <= 1; y++)
                {
                    for (int z = -1; z <= 1; z++)
                    {
                        if (!ends.TryGetValue(cell + new Vector3Int(x, y, z), out List<(Line line, bool atB)> list))
                        {
                            continue;
                        }
                        foreach ((Line line, bool _) in list)
                        {
                            if (line.Matches(a, b))
                            {
                                return line;
                            }
                        }
                    }
                }
            }
            return null;
        }

        public static Line Find(string key)
        {
            return Decode(key, out Vector3 a, out Vector3 b) ? Find(a, b) : null;
        }

        // The closest point of any line within a distance.
        public static bool Nearest(Vector3 point, float within, out Line best, out float at)
        {
            best = null;
            at = 0f;
            float bestDistance = within * within;
            foreach (Line line in Lines)
            {
                for (int i = 0; i + 1 < line.points.Length; i++)
                {
                    Vector3 a = line.points[i];
                    Vector3 b = line.points[i + 1];
                    float t = Mathf.Clamp01(Vector3.Dot(point - a, b - a) / Mathf.Max((b - a).sqrMagnitude, 1e-6f));
                    float distance = (Vector3.Lerp(a, b, t) - point).sqrMagnitude;
                    if (distance < bestDistance)
                    {
                        bestDistance = distance;
                        best = line;
                        at = line.along[i] + t * (line.along[i + 1] - line.along[i]);
                    }
                }
            }
            return best != null;
        }

        public static string Encode(Vector3 v)
        {
            return string.Format(CultureInfo.InvariantCulture, "{0:0.###},{1:0.###},{2:0.###}", v.x, v.y, v.z);
        }

        public static Vector3 DecodeVector(string text)
        {
            string[] parts = text.Split(',');
            return new Vector3(
                float.Parse(parts[0], CultureInfo.InvariantCulture),
                float.Parse(parts[1], CultureInfo.InvariantCulture),
                float.Parse(parts[2], CultureInfo.InvariantCulture));
        }

        public static bool Decode(string key, out Vector3 a, out Vector3 b)
        {
            a = b = Vector3.zero;
            string[] parts = key.Split(';');
            if (parts.Length != 2)
            {
                return false;
            }
            a = DecodeVector(parts[0]);
            b = DecodeVector(parts[1]);
            return true;
        }
    }
}
