using System.Collections.Generic;
using System.Linq;
using UnityEngine;

namespace Blackforge
{
    // How a player sees a train another machine moves, the server or another
    // player. Its data comes in a few times a second at uneven times. A guess
    // in a straight line between updates, blended toward each new one, made
    // the train jump a little on every update and cut its curves: the flicker
    // on the server. Here the viewer runs its own copy of the train along the
    // track at the speed the owner reports, and pulls it gently toward where
    // the owner's data says the train is now.
    public static class Follow
    {
        // Off draws trains of other machines the old way, for the rail test.
        public static bool Enabled = true;

        // How fast the copy closes the gap to the owner, per second.
        private const float Pull = 2f;
        // Further off than this, the copy jumps to the owner's place.
        private const float Snap = 6f;
        // The owner's data is old by the time it arrives, older than this is
        // not moved on further.
        private const float MaxAge = 0.5f;

        private class Copy
        {
            public TrackCursor cursor;
            public uint revision;
            public float receivedAt;
        }

        private static readonly Dictionary<ZDOID, Copy> copies = new Dictionary<ZDOID, Copy>();

        public static void Forget(ZDOID loco)
        {
            copies.Remove(loco);
        }

        // Moves the copy of a train one frame and puts its bodies there.
        // False when the train is not on known track, then the caller draws it
        // from its data.
        public static bool Step(ZDO loco, float dt)
        {
            if (!Train.Load(loco, out TrackCursor owner))
            {
                copies.Remove(loco.m_uid);
                return false;
            }
            if (!copies.TryGetValue(loco.m_uid, out Copy copy) || Graph.Find(copy.cursor.line.Key) == null)
            {
                copy = new Copy { cursor = owner, revision = loco.DataRevision, receivedAt = Time.time };
                copies[loco.m_uid] = copy;
            }
            if (loco.DataRevision != copy.revision)
            {
                copy.revision = loco.DataRevision;
                copy.receivedAt = Time.time;
            }

            float speed = loco.GetFloat(Train.Speed);
            List<string> route = Train.ReadList(loco, Train.Route);
            List<string> trail = Train.ReadList(loco, Train.Trail);

            // Where the owner's train is now: its data, moved on by the time
            // since it arrived.
            TrackCursor now = owner;
            float age = Mathf.Min(Time.time - copy.receivedAt, MaxAge);
            now.Walk(speed * age, Chooser(now, route, trail, owner.line));
            Vector3 gap = now.Position - copy.cursor.Position;
            if (gap.sqrMagnitude > Snap * Snap || Vector3.Dot(now.Forward, copy.cursor.Forward) < 0f)
            {
                copy.cursor = now;
            }
            else
            {
                float behind = Vector3.Dot(gap, copy.cursor.Forward);
                float ds = speed * dt + behind * Mathf.Min(1f, Pull * dt);
                TrackCursor next = copy.cursor;
                next.Walk(ds, Chooser(next, route, trail, owner.line));
                copy.cursor = next;
            }

            // The trail of the owner starts behind the owner's line. The copy
            // can be a line ahead of it or still on the line before.
            List<string> behindCopy = new List<string>(trail);
            if (copy.cursor.line != owner.line)
            {
                if (behindCopy.Count > 0 && behindCopy[0] == copy.cursor.line.Key)
                {
                    behindCopy.RemoveAt(0);
                }
                else
                {
                    behindCopy.Insert(0, owner.line.Key);
                }
            }
            foreach (Train.Body body in Train.Layout(loco, copy.cursor, behindCopy))
            {
                ZDO part = body.id == loco.m_uid ? loco : ZDOMan.instance.GetZDO(body.id);
                ZNetView view = part != null ? ZNetScene.instance.FindInstance(part) : null;
                if (view != null)
                {
                    view.transform.SetPositionAndRotation(body.position, body.rotation);
                }
            }
            return true;
        }

        // Forward it follows the route, or the owner's line, or goes straight
        // on. Backward it follows the trail.
        private static System.Func<List<Step>, bool, Step?> Chooser(TrackCursor at, List<string> route, List<string> trail, Line ownerLine)
        {
            return (steps, backward) =>
            {
                if (backward)
                {
                    return Train.FromTrail(steps, new Queue<string>(trail), -at.Forward);
                }
                foreach (Step step in steps)
                {
                    if (step.line == ownerLine || route.Any(key => key == step.line.Key || (Graph.Decode(key, out Vector3 a, out Vector3 b) && step.line.Matches(a, b))))
                    {
                        return step;
                    }
                }
                return TrackCursor.Straightest(steps, at.Forward, 0);
            };
        }
    }
}
