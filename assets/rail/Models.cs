using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Text;
using UnityEngine;

namespace Blackforge
{
    // Reads the meshes and icons that models/export.py writes and the build
    // embeds. A mesh has one part per material slot, and each slot gets a
    // vanilla material, so the pieces look exactly like vanilla wood, stone
    // and iron.
    public static class Models
    {
        public const string Wood = "wood";
        public const string Stone = "stone";
        public const string Iron = "iron";

        public class Model
        {
            public Mesh mesh;
            public string[] slots;
            public List<Box> boxes;
            public List<Vector3> snaps;
            public List<Vector3[]> lines;
        }

        public class Box
        {
            public Vector3 center;
            public Vector3 size;
            public Quaternion rotation;
        }

        private static readonly Dictionary<string, Model> cache = new Dictionary<string, Model>();

        public static Model Load(string name)
        {
            if (cache.TryGetValue(name, out Model model))
            {
                return model;
            }
            using (Stream stream = Resource($"meshes.{name}.bfm"))
            using (BinaryReader reader = new BinaryReader(stream))
            {
                if (Encoding.ASCII.GetString(reader.ReadBytes(4)) != "BFM4")
                {
                    throw new InvalidDataException($"{name} is not a mesh file");
                }
                int parts = reader.ReadInt32();
                List<Vector3> positions = new List<Vector3>();
                List<Vector3> normals = new List<Vector3>();
                List<Vector2> uvs = new List<Vector2>();
                List<int[]> triangles = new List<int[]>();
                string[] slots = new string[parts];
                for (int part = 0; part < parts; part++)
                {
                    slots[part] = Encoding.UTF8.GetString(reader.ReadBytes(reader.ReadInt32()));
                    int offset = positions.Count;
                    int count = reader.ReadInt32();
                    for (int i = 0; i < count; i++)
                    {
                        positions.Add(new Vector3(reader.ReadSingle(), reader.ReadSingle(), reader.ReadSingle()));
                        normals.Add(new Vector3(reader.ReadSingle(), reader.ReadSingle(), reader.ReadSingle()));
                        uvs.Add(new Vector2(reader.ReadSingle(), reader.ReadSingle()));
                    }
                    int[] tris = new int[reader.ReadInt32()];
                    for (int i = 0; i < tris.Length; i++)
                    {
                        tris[i] = offset + reader.ReadInt32();
                    }
                    triangles.Add(tris);
                }
                List<Box> boxes = new List<Box>();
                int boxCount = reader.ReadInt32();
                for (int i = 0; i < boxCount; i++)
                {
                    Vector3 center = ReadVector(reader);
                    Vector3 size = ReadVector(reader);
                    Vector3 forward = ReadVector(reader);
                    Vector3 up = ReadVector(reader);
                    boxes.Add(new Box { center = center, size = size, rotation = Quaternion.LookRotation(forward, up) });
                }
                Mesh mesh = new Mesh { name = $"bf_{name}" };
                if (positions.Count > ushort.MaxValue)
                {
                    mesh.indexFormat = UnityEngine.Rendering.IndexFormat.UInt32;
                }
                mesh.SetVertices(positions);
                mesh.SetNormals(normals);
                mesh.SetUVs(0, uvs);
                // Plain white, a vanilla piece shader may tint by vertex color.
                mesh.SetColors(Enumerable.Repeat(Color.white, positions.Count).ToList());
                mesh.subMeshCount = parts;
                for (int part = 0; part < parts; part++)
                {
                    mesh.SetTriangles(triangles[part], part);
                }
                mesh.RecalculateBounds();
                mesh.RecalculateTangents();
                List<Vector3> snaps = new List<Vector3>();
                int snapCount = reader.ReadInt32();
                for (int i = 0; i < snapCount; i++)
                {
                    snaps.Add(ReadVector(reader));
                }
                List<Vector3[]> lines = new List<Vector3[]>();
                int lineCount = reader.ReadInt32();
                for (int i = 0; i < lineCount; i++)
                {
                    Vector3[] line = new Vector3[reader.ReadInt32()];
                    for (int j = 0; j < line.Length; j++)
                    {
                        line[j] = ReadVector(reader);
                    }
                    lines.Add(line);
                }
                model = new Model { mesh = mesh, slots = slots, boxes = boxes, snaps = snaps, lines = lines };
                cache[name] = model;
                return model;
            }
        }

        private static Vector3 ReadVector(BinaryReader reader)
        {
            return new Vector3(reader.ReadSingle(), reader.ReadSingle(), reader.ReadSingle());
        }

        public static Sprite Icon(string name)
        {
            using (Stream stream = Resource($"icons.{name}.png"))
            using (MemoryStream memory = new MemoryStream())
            {
                stream.CopyTo(memory);
                Texture2D texture = new Texture2D(2, 2);
                texture.LoadImage(memory.ToArray());
                return Sprite.Create(texture, new Rect(0, 0, texture.width, texture.height), new Vector2(0.5f, 0.5f));
            }
        }

        private static Stream Resource(string name)
        {
            Stream stream = Assembly.GetExecutingAssembly().GetManifestResourceStream(name);
            if (stream == null)
            {
                throw new FileNotFoundException($"the plugin carries no {name}");
            }
            return stream;
        }
    }
}
