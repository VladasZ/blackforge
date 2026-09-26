using System.Collections.Generic;
using UnityEngine;

namespace Blackforge
{
    // A placed track piece. Its centerlines come from the model, at the height
    // of the sleeper bottoms, where the origin of a train sits.
    public class RailTrack : MonoBehaviour
    {
        // Unity copies a string when it clones the prefab, but not a list of
        // arrays, so the lines are looked up by the model name.
        public string m_model;

        public List<Vector3[]> Lines => Models.Load(m_model).lines;

        // The closest point on any centerline to a world point, with the
        // direction of the line there.
        public bool Closest(Vector3 world, out Vector3 point, out Vector3 forward)
        {
            point = Vector3.zero;
            forward = Vector3.forward;
            float best = float.MaxValue;
            foreach (Vector3[] line in Lines)
            {
                for (int i = 0; i + 1 < line.Length; i++)
                {
                    Vector3 a = transform.TransformPoint(line[i]);
                    Vector3 b = transform.TransformPoint(line[i + 1]);
                    Vector3 on = a + Vector3.Project(world - a, b - a);
                    float t = Vector3.Dot(on - a, b - a) / Mathf.Max((b - a).sqrMagnitude, 1e-6f);
                    on = Vector3.Lerp(a, b, Mathf.Clamp01(t));
                    float distance = (on - world).sqrMagnitude;
                    if (distance < best)
                    {
                        best = distance;
                        point = on;
                        forward = (b - a).normalized;
                    }
                }
            }
            return best < float.MaxValue;
        }
    }
}
