using System;
using System.Collections.Generic;
using UnityEngine;

namespace Blackforge
{
    // A place on the track: a line, a distance from its A end, and which way
    // along the line counts as forward, +1 toward B and -1 toward A.
    public struct TrackCursor
    {
        public Line line;
        public float s;
        public int dir;

        public Vector3 Position
        {
            get
            {
                line.At(s, out Vector3 position, out Vector3 _);
                return position;
            }
        }

        public Vector3 Forward
        {
            get
            {
                line.At(s, out Vector3 _, out Vector3 forward);
                return forward * dir;
            }
        }

        // Moves along the track by a distance, backward when it is negative.
        // At the end of a line the chooser picks the next one out of the
        // steps the graph offers. Returns the distance it could not go, more
        // than zero at the end of the track or at a gap.
        public float Walk(float distance, Func<List<Step>, bool, Step?> choose, Action<Line, bool> entered = null)
        {
            bool backward = distance < 0f;
            float left = Mathf.Abs(distance);
            int travel = backward ? -dir : dir;
            for (int guard = 0; guard < 64 && left > 1e-4f; guard++)
            {
                float room = travel > 0 ? line.Length - s : s;
                if (left <= room)
                {
                    s += travel * left;
                    return 0f;
                }
                left -= room;
                s = travel > 0 ? line.Length : 0f;
                List<Step> steps = Graph.Next(line, travel > 0);
                Step? next = steps.Count == 0 ? null : choose(steps, backward);
                if (next == null)
                {
                    return left;
                }
                Line from = line;
                line = next.Value.line;
                s = next.Value.fromA ? 0f : line.Length;
                travel = next.Value.fromA ? 1 : -1;
                dir = backward ? -travel : travel;
                entered?.Invoke(from, backward);
            }
            return left;
        }

        // The step that bends the least, or the one that bends most toward a
        // side, -1 left and +1 right.
        public static Step Straightest(List<Step> steps, Vector3 heading, int side)
        {
            Step best = steps[0];
            float bestScore = float.MinValue;
            foreach (Step step in steps)
            {
                Vector3 onward = step.fromA ? step.line.Leaving(false) * -1f : step.line.Leaving(true) * -1f;
                float turn = Vector3.Cross(heading, onward).y;
                float score = side == 0 ? Vector3.Dot(heading, onward) : side * turn;
                if (score > bestScore)
                {
                    bestScore = score;
                    best = step;
                }
            }
            return best;
        }
    }
}
