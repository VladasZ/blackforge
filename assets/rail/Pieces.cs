using System;
using System.Collections.Generic;
using System.Linq;
using UnityEngine;

namespace Blackforge
{
    // Builds the rail prefabs, registers them with the net scene and puts them
    // on the hammer. The prefabs live under an inactive holder, so none of
    // their game components wake up until the game spawns a copy.
    //
    // Track and stations get a vanilla WearNTear of wood, so they need support
    // like a wood floor and break when they reach too far into the air. Every
    // other damage is off, see RailPiece. Trains get one without support.
    public static class Pieces
    {
        public class Spec
        {
            public string prefab;
            public string model;
            public string name;
            public string description;
            public int wood;
            public int stone;
        }

        public static readonly Spec[] Specs =
        {
            new Spec { prefab = "bf_track_straight", model = "track_straight", name = "Track 2 m", description = "Straight stone track.", wood = 2, stone = 4 },
            new Spec { prefab = "bf_track_straight_1", model = "track_straight_1", name = "Track 1 m", description = "Short straight stone track.", wood = 1, stone = 2 },
            new Spec { prefab = "bf_track_straight_4", model = "track_straight_4", name = "Track 4 m", description = "Long straight stone track.", wood = 4, stone = 8 },
            new Spec { prefab = "bf_track_curve_r8_22", model = "track_curve_r8_22", name = "Curve 8 m, 22.5 degrees", description = "A tight turn. Place it the other way round to turn left.", wood = 3, stone = 6 },
            new Spec { prefab = "bf_track_curve_r8_45", model = "track_curve_r8_45", name = "Curve 8 m, 45 degrees", description = "A tight turn. Place it the other way round to turn left.", wood = 6, stone = 13 },
            new Spec { prefab = "bf_track_curve_r8_90", model = "track_curve_r8_90", name = "Curve 8 m, 90 degrees", description = "A tight turn. Place it the other way round to turn left.", wood = 13, stone = 25 },
            new Spec { prefab = "bf_track_curve_r16_22", model = "track_curve_r16_22", name = "Curve 16 m, 22.5 degrees", description = "A wide turn. Place it the other way round to turn left.", wood = 6, stone = 13 },
            new Spec { prefab = "bf_track_curve_r16_45", model = "track_curve_r16_45", name = "Curve 16 m, 45 degrees", description = "A wide turn. Place it the other way round to turn left.", wood = 13, stone = 25 },
            new Spec { prefab = "bf_track_curve_r32_22", model = "track_curve_r32_22", name = "Curve 32 m, 22.5 degrees", description = "A very wide turn. Place it the other way round to turn left.", wood = 13, stone = 25 },
            new Spec { prefab = "bf_track_slope_gentle", model = "track_slope_gentle", name = "Gentle slope", description = "Rises 0.5 m over 2 m.", wood = 2, stone = 4 },
            new Spec { prefab = "bf_track_slope_steep", model = "track_slope_steep", name = "Steep slope", description = "Rises 1 m over 2 m.", wood = 2, stone = 5 },
            new Spec { prefab = "bf_track_slope_bottom_gentle", model = "track_slope_bottom_gentle", name = "Gentle slope bottom", description = "Bends from flat into a gentle slope over 4 m. Turned round it ends a slope going down.", wood = 4, stone = 8 },
            new Spec { prefab = "bf_track_slope_top_gentle", model = "track_slope_top_gentle", name = "Gentle slope top", description = "Levels a gentle slope out over 4 m. Turned round it starts a slope going down.", wood = 4, stone = 8 },
            new Spec { prefab = "bf_track_slope_bottom_steep", model = "track_slope_bottom_steep", name = "Steep slope bottom", description = "Bends from flat into a steep slope over 4 m. Turned round it ends a slope going down.", wood = 4, stone = 8 },
            new Spec { prefab = "bf_track_slope_top_steep", model = "track_slope_top_steep", name = "Steep slope top", description = "Levels a steep slope out over 4 m. Turned round it starts a slope going down.", wood = 4, stone = 8 },
            new Spec { prefab = "bf_track_switch_right", model = "track_switch_right", name = "Switch right", description = "A straight with a 16 m branch to the right.", wood = 12, stone = 25 },
            new Spec { prefab = "bf_track_switch_left", model = "track_switch_left", name = "Switch left", description = "A straight with a 16 m branch to the left.", wood = 12, stone = 25 },
            new Spec { prefab = "bf_track_crossing", model = "track_crossing", name = "Crossing", description = "Two tracks crossing at a right angle.", wood = 8, stone = 16 },
            new Spec { prefab = "bf_station", model = "station", name = "Train station", description = "A named stop for trains.", wood = 30, stone = 10 },
            new Spec { prefab = "bf_locomotive", model = "locomotive", name = "Locomotive", description = "Burns coal and pulls up to 3 wagons.", wood = 40, stone = 30 },
            new Spec { prefab = "bf_wagon", model = "wagon", name = "Wagon", description = "Carries cargo behind a locomotive.", wood = 20, stone = 8 },
        };

        // Vanilla pieces that carry the materials the slots take.
        private static readonly (string slot, string prefab, string material)[] MaterialSources =
        {
            (Models.Wood, "wood_beam", "woodwall"),
            (Models.Stone, "stone_wall_1x1", "stone_mat"),
            (Models.Iron, "woodiron_beam", "Ironbeam_mat"),
        };

        private static GameObject holder;
        private static readonly Dictionary<string, GameObject> prefabs = new Dictionary<string, GameObject>();

        public static IEnumerable<GameObject> All => prefabs.Values;

        // Builds the prefabs once, the vanilla prefabs are the source of the
        // materials and of the cart storage background.
        public static bool Ensure(IEnumerable<GameObject> vanilla)
        {
            if (prefabs.Count == Specs.Length)
            {
                return true;
            }
            Dictionary<string, GameObject> byName = vanilla.Where(go => go != null).GroupBy(go => go.name).ToDictionary(g => g.Key, g => g.First());
            Dictionary<string, Material> materials = new Dictionary<string, Material>();
            foreach ((string slot, string prefab, string material) in MaterialSources)
            {
                Material found = byName.TryGetValue(prefab, out GameObject source)
                    ? source.GetComponentsInChildren<Renderer>(true).SelectMany(r => r.sharedMaterials).FirstOrDefault(m => m != null && m.name == material)
                    : null;
                if (found == null)
                {
                    return false;
                }
                materials[slot] = found;
            }
            byName.TryGetValue("Cart", out GameObject cart);
            LogShader(cart, materials[Models.Wood]);
            // Trains move, so their materials work in their own space: the piece
            // shader bends every vertex by a noise read from its world position,
            // which would make a moving train wobble. The bend a train gets when
            // it is placed is baked into its mesh, see TrainShape.
            Dictionary<string, Material> moving = materials.ToDictionary(p => p.Key, p => Moveable(p.Value));
            // The chimney smoke is the smoke of the vanilla smelter.
            GameObject smoke = byName.TryGetValue("smelter", out GameObject smelter)
                ? smelter.GetComponentsInChildren<ParticleSystem>(true).Select(p => p.gameObject).FirstOrDefault(g => g.name.ToLowerInvariant().Contains("smoke"))
                : null;
            RailPlugin.Log.LogInfo(smoke != null ? $"chimney smoke from smelter {smoke.name}" : "the smelter has no smoke, the chimney stays clear");

            holder = new GameObject("BlackforgeRailPrefabs");
            holder.SetActive(false);
            UnityEngine.Object.DontDestroyOnLoad(holder);
            foreach (Spec spec in Specs)
            {
                prefabs[spec.prefab] = Build(spec, Placement.IsVehicle(spec.prefab) ? moving : materials, cart, smoke);
            }
            RailPlugin.Log.LogInfo($"rail pieces built: {string.Join(", ", prefabs.Keys)}");
            return true;
        }

        private static GameObject Build(Spec spec, Dictionary<string, Material> materials, GameObject cart, GameObject smoke)
        {
            int layer = LayerMask.NameToLayer("piece");
            GameObject root = new GameObject(spec.prefab) { layer = layer };
            root.transform.SetParent(holder.transform, false);

            ZNetView view = root.AddComponent<ZNetView>();
            view.m_persistent = true;

            Models.Model model = Models.Load(spec.model);
            GameObject visual = new GameObject("model") { layer = layer };
            visual.transform.SetParent(root.transform, false);
            visual.AddComponent<MeshFilter>().sharedMesh = model.mesh;
            visual.AddComponent<MeshRenderer>().sharedMaterials = model.slots.Select(slot => materials[slot]).ToArray();
            if (Placement.IsVehicle(spec.prefab))
            {
                visual.AddComponent<TrainShape>();
            }
            // Box colliders, one per model part. The game places a piece by its
            // convex colliders and skips a concave mesh collider, so a mesh
            // collider alone puts the ghost far off the cursor.
            foreach (Models.Box box in model.boxes)
            {
                GameObject part = new GameObject("collider") { layer = layer };
                part.transform.SetParent(root.transform, false);
                part.transform.localPosition = box.center;
                part.transform.localRotation = box.rotation;
                part.AddComponent<BoxCollider>().size = box.size;
            }
            // Track ends. The hammer snaps a track end of the ghost onto one of a
            // placed piece.
            foreach (Vector3 point in model.snaps)
            {
                GameObject end = new GameObject("_snappoint") { tag = "snappoint", layer = layer };
                end.transform.SetParent(root.transform, false);
                end.transform.localPosition = point;
            }

            Piece piece = root.AddComponent<Piece>();
            piece.m_name = spec.name;
            piece.m_description = spec.description;
            piece.m_icon = Models.Icon(spec.model);
            piece.m_category = Piece.PieceCategory.Misc;
            // The build menu of Valheim 1.0 groups pieces by these tags, not by the
            // category.
            piece.m_usage = Piece.UsageTagFlags.Transport;
            piece.m_canBeRemoved = true;
            root.AddComponent<RailPiece>();
            if (model.lines.Count > 0)
            {
                root.AddComponent<RailTrack>().m_model = spec.model;
            }

            // Trains get one too, like the vanilla cart, so the hammer highlights
            // and removes them the vanilla way. They need no support and move.
            bool vehicle = Placement.IsVehicle(spec.prefab);
            WearNTear wear = root.AddComponent<WearNTear>();
            wear.m_materialType = WearNTear.MaterialType.Wood;
            wear.m_supports = !vehicle;
            wear.m_noRoofWear = true;
            wear.m_noSupportWear = !vehicle;
            wear.m_staticPosition = !vehicle;
            wear.m_snowDamageImmune = true;
            wear.m_ashDamageImmune = true;
            wear.m_burnable = false;
            wear.m_health = 400f;

            if (spec.prefab == "bf_station")
            {
                root.AddComponent<Station>();
            }
            if (Placement.IsVehicle(spec.prefab))
            {
                root.AddComponent<RailBody>();
            }
            if (spec.prefab == "bf_locomotive")
            {
                root.AddComponent<RailLocomotive>();
                // The bench at the back of the driver platform, see models/locomotive.py.
                GameObject seat = new GameObject("seat") { layer = layer };
                seat.transform.SetParent(root.transform, false);
                seat.transform.localPosition = new Vector3(0f, 1.95f, -1.9f);
                seat.AddComponent<BoxCollider>().size = new Vector3(1.1f, 0.6f, 0.5f);
                GameObject attach = new GameObject("attach");
                attach.transform.SetParent(root.transform, false);
                attach.transform.localPosition = new Vector3(0f, 1.8f, -1.85f);
                seat.AddComponent<RailSeat>().m_attach = attach.transform;
                if (smoke != null)
                {
                    // The top of the chimney, see models/locomotive.py.
                    GameObject chimney = UnityEngine.Object.Instantiate(smoke, root.transform, false);
                    chimney.name = "smoke";
                    chimney.transform.localPosition = new Vector3(0f, 4.45f, 1.5f);
                    chimney.transform.localRotation = Quaternion.identity;
                    // Much more and bigger smoke than a smelter, a locomotive works hard.
                    foreach (ParticleSystem system in chimney.GetComponentsInChildren<ParticleSystem>(true))
                    {
                        ParticleSystem.MainModule main = system.main;
                        main.startSizeMultiplier *= 2.5f;
                        main.startLifetimeMultiplier *= 1.5f;
                        main.maxParticles *= 8;
                        ParticleSystem.EmissionModule emission = system.emission;
                        emission.rateOverTimeMultiplier *= 4f;
                    }
                    chimney.SetActive(false);
                }
            }
            if (spec.prefab == "bf_wagon")
            {
                Container container = root.AddComponent<Container>();
                container.m_name = spec.name;
                container.m_width = 6;
                container.m_height = 3;
                Container vanilla = cart != null ? cart.GetComponentInChildren<Container>(true) : null;
                if (vanilla != null)
                {
                    container.m_bkg = vanilla.m_bkg;
                    container.m_openEffects = vanilla.m_openEffects;
                    container.m_closeEffects = vanilla.m_closeEffects;
                }
            }
            return root;
        }

        private static readonly string[] ShaderProperties = { "_MoveableObject", "_TriplanarLocalPos", "_TriplanarMap", "_ValueNoise", "_ValueNoiseVertex", "_RippleDistance", "_RippleFreq" };

        private static Material Moveable(Material source)
        {
            Material copy = new Material(source) { name = source.name + " moving" };
            // Like the vanilla cart: a moving object, with no vertex bend from the
            // shader. The color noise stays, the cart keeps it too.
            Set(copy, "_MoveableObject", 1f);
            Set(copy, "_TriplanarLocalPos", 1f);
            Set(copy, "_ValueNoiseVertex", 0f);
            Set(copy, "_RippleDistance", 0f);
            return copy;
        }

        private static void Set(Material material, string property, float value)
        {
            if (material.HasProperty(property))
            {
                material.SetFloat(property, value);
            }
        }

        // What the vanilla cart and a vanilla wall set in the piece shader.
        private static void LogShader(GameObject cart, Material wall)
        {
            IEnumerable<Material> cartMaterials = cart != null ? cart.GetComponentsInChildren<Renderer>(true).SelectMany(r => r.sharedMaterials).Where(m => m != null).Distinct() : Enumerable.Empty<Material>();
            foreach (Material material in cartMaterials.Append(wall))
            {
                string values = string.Join(", ", ShaderProperties.Select(p => material.HasProperty(p) ? $"{p} {material.GetFloat(p):0.###}" : $"{p} none"));
                RailPlugin.Log.LogInfo($"shader of {material.name} ({material.shader.name}): {values}");
            }
        }

        // The costs need the item database, so they are set when the pieces
        // go on the hammer.
        public static void AddToHammer(ObjectDB db)
        {
            // In the main menu the hammer has no piece table yet. Only a world, which
            // has a net scene, must have one, the game scene also reaches this
            // through CopyOtherDB.
            GameObject hammer = db.GetItemPrefab("Hammer");
            PieceTable table = hammer != null ? hammer.GetComponent<ItemDrop>()?.m_itemData.m_shared.m_buildPieces : null;
            if (table == null)
            {
                if (ZNetScene.instance != null)
                {
                    RailPlugin.Log.LogError("the hammer has no piece table, the rail pieces are not buildable");
                }
                return;
            }
            if (!Ensure(table.m_pieces))
            {
                RailPlugin.Log.LogError("the vanilla materials were not found, the rail pieces are not buildable");
                return;
            }
            ItemDrop wood = db.GetItemPrefab("Wood")?.GetComponent<ItemDrop>();
            ItemDrop stone = db.GetItemPrefab("Stone")?.GetComponent<ItemDrop>();
            CraftingStation workbench = table.m_pieces.Select(p => p.GetComponent<CraftingStation>()).FirstOrDefault(s => s != null && s.name == "piece_workbench");
            foreach (Spec spec in Specs)
            {
                GameObject prefab = prefabs[spec.prefab];
                Piece piece = prefab.GetComponent<Piece>();
                piece.m_craftingStation = workbench;
                piece.m_resources = new[]
                {
                    new Piece.Requirement { m_resItem = wood, m_amount = spec.wood, m_recover = true },
                    new Piece.Requirement { m_resItem = stone, m_amount = spec.stone, m_recover = true },
                };
                if (!table.m_pieces.Contains(prefab))
                {
                    table.m_pieces.Add(prefab);
                }
            }
        }
    }
}
