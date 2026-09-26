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

        private void Start()
        {
            ZNetView view = GetComponentInParent<ZNetView>();
            MeshFilter filter = GetComponent<MeshFilter>();
            if (view == null || !view.IsValid() || filter == null || filter.sharedMesh == null)
            {
                return;
            }
            System.Random random = new System.Random(view.GetZDO().m_uid.GetHashCode());
            Vector3 seed = new Vector3((float)random.NextDouble(), (float)random.NextDouble(), (float)random.NextDouble()) * 1000f;
            Mesh mesh = Instantiate(filter.sharedMesh);
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
            filter.mesh = mesh;
        }

        private void OnDestroy()
        {
            MeshFilter filter = GetComponent<MeshFilter>();
            if (filter != null && filter.sharedMesh != null && filter.sharedMesh.name.EndsWith("(Clone)"))
            {
                Destroy(filter.sharedMesh);
            }
        }
    }
}
