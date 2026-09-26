using UnityEngine;

namespace Blackforge
{
    // Placed pieces never look like the ideal model, the piece shader bends
    // each one by where it stands. A train gets its own bend once, from its
    // id, baked into its own copy of the mesh, and keeps that shape while it
    // moves. A vertex bends by its position alone, so faces stay joined.
    public class TrainShape : MonoBehaviour
    {
        // The ripple of a vanilla wall, _RippleDistance of woodwall.
        private const float Amount = 0.03f;
        private const float Frequency = 0.7f;

        // The console command railshape turns the bend off, to compare.
        public static bool Enabled = true;

        private Mesh m_model;
        private Mesh m_bent;

        private void Start()
        {
            Apply();
        }

        public void Apply()
        {
            ZNetView view = GetComponentInParent<ZNetView>();
            MeshFilter filter = GetComponent<MeshFilter>();
            if (view == null || !view.IsValid() || filter == null)
            {
                return;
            }
            if (m_model == null)
            {
                m_model = filter.sharedMesh;
            }
            if (!Enabled)
            {
                filter.sharedMesh = m_model;
                return;
            }
            if (m_bent == null)
            {
                m_bent = Bend(m_model, view.GetZDO().m_uid.GetHashCode());
            }
            filter.sharedMesh = m_bent;
        }

        private static Mesh Bend(Mesh model, int seedFrom)
        {
            System.Random random = new System.Random(seedFrom);
            Vector3 seed = new Vector3((float)random.NextDouble(), (float)random.NextDouble(), (float)random.NextDouble()) * 1000f;
            Mesh mesh = Instantiate(model);
            Vector3[] vertices = mesh.vertices;
            for (int i = 0; i < vertices.Length; i++)
            {
                Vector3 p = vertices[i] * Frequency + seed;
                vertices[i] += new Vector3(
                    Mathf.PerlinNoise(p.y, p.z) - 0.5f,
                    Mathf.PerlinNoise(p.z, p.x) - 0.5f,
                    Mathf.PerlinNoise(p.x, p.y) - 0.5f) * (2f * Amount);
            }
            mesh.vertices = vertices;
            mesh.RecalculateNormals();
            mesh.RecalculateBounds();
            return mesh;
        }

        private void OnDestroy()
        {
            if (m_bent != null)
            {
                Destroy(m_bent);
            }
        }
    }
}
