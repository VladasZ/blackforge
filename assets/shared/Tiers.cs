using System;
using System.Collections.Generic;
using UnityEngine;

namespace Blackforge
{
    // One material a competitive server forbids, `Forbidden` of blackforge-api.
    public class ForbiddenItem
    {
        public string item;
        public string boss;
        public int tier;
    }

    // Shared by the join plugin and the server plugin, see docs/competitive.md.
    // The backend lists only what a world cannot give before a boss dies, like
    // a boss drop or an ore no pickaxe of the tier can mine. This walks the
    // recipes, the smelters, fermenters and cooking stations of the game, and
    // the build cost of every station they need, and forbids every item that
    // cannot be made without a forbidden material.
    public static class Tiers
    {
        // The id of the world an item was found or made in. The game saves it
        // with the item, so fair loot of a competitive world stays allowed there.
        public const string TagKey = "blackforge.world";
        // A build piece in the walk, next to the items.
        private const string PiecePrefix = "piece:";

        // One way to make an item. Everything in `all` is needed, and one of
        // `anyOne` when a recipe takes any one of its ingredients.
        private class Way
        {
            public List<string> all = new List<string>();
            public List<string> anyOne = new List<string>();
        }

        private class Walk
        {
            public Dictionary<string, ForbiddenItem> listed;
            public HashSet<string> allowed;
            public Dictionary<string, List<Way>> ways = new Dictionary<string, List<Way>>();
            public Dictionary<string, ForbiddenItem> memo = new Dictionary<string, ForbiddenItem>();
            public HashSet<string> visiting = new HashSet<string>();

            public void Add(string product, Way way)
            {
                if (!ways.TryGetValue(product, out List<Way> list))
                {
                    list = new List<Way>();
                    ways[product] = list;
                }
                list.Add(way);
            }
        }

        public static string Tag(ItemDrop.ItemData item)
        {
            return item.m_customData.TryGetValue(TagKey, out string tag) ? tag : null;
        }

        // Every forbidden item by prefab name, with the boss that frees it. An
        // allowed item is never forbidden, it also drops somewhere anybody can
        // reach. A name the game does not know goes to warn, it is a typo in
        // the tiers of the backend.
        public static Dictionary<string, ForbiddenItem> Expand(
            List<ForbiddenItem> materials, List<string> allowed, Action<string> warn)
        {
            Dictionary<string, ForbiddenItem> result = new Dictionary<string, ForbiddenItem>();
            ObjectDB db = ObjectDB.instance;
            if (db == null || materials == null || materials.Count == 0)
            {
                return result;
            }
            Walk walk = new Walk
            {
                listed = new Dictionary<string, ForbiddenItem>(),
                allowed = new HashSet<string>(allowed ?? new List<string>()),
            };
            foreach (string item in walk.allowed)
            {
                if (db.GetItemPrefab(item) == null)
                {
                    warn($"{item} of the allowed items is no item of the game");
                }
            }
            foreach (ForbiddenItem material in materials)
            {
                if (db.GetItemPrefab(material.item) == null)
                {
                    warn($"{material.item} of the tiers is no item of the game");
                }
                walk.listed[material.item] = material;
            }
            Collect(db, walk);
            foreach (GameObject prefab in db.m_items)
            {
                if (prefab == null)
                {
                    continue;
                }
                ForbiddenItem found = Resolve(prefab.name, walk);
                if (found != null)
                {
                    result[prefab.name] = found;
                }
            }
            return result;
        }

        private static void Collect(ObjectDB db, Walk walk)
        {
            HashSet<Piece> added = new HashSet<Piece>();
            foreach (Recipe recipe in db.m_recipes)
            {
                if (recipe == null || recipe.m_item == null || !recipe.m_enabled)
                {
                    continue;
                }
                Way way = new Way();
                foreach (Piece.Requirement requirement in recipe.m_resources)
                {
                    // An ingredient with no amount is only for the upgrades,
                    // the first level of the item does not need it. An idol
                    // is only taken by the upgrade station.
                    if (requirement.m_resItem != null && requirement.m_amount > 0 && !requirement.m_upgraderResource)
                    {
                        (recipe.m_requireOnlyOneIngredient ? way.anyOne : way.all).Add(requirement.m_resItem.name);
                    }
                }
                Piece station = recipe.m_craftingStation != null ? recipe.m_craftingStation.GetComponent<Piece>() : null;
                if (station != null)
                {
                    way.all.Add(AddPiece(station, walk, added));
                }
                walk.Add(recipe.m_item.name, way);
            }
            // The stations are build pieces, found through the tools that
            // build them. The main menu has no ZNetScene, but it has these.
            HashSet<GameObject> pieces = new HashSet<GameObject>();
            foreach (GameObject prefab in db.m_items)
            {
                PieceTable table = prefab?.GetComponent<ItemDrop>()?.m_itemData.m_shared.m_buildPieces;
                if (table == null)
                {
                    continue;
                }
                foreach (GameObject piece in table.m_pieces)
                {
                    if (piece != null)
                    {
                        pieces.Add(piece);
                    }
                }
            }
            foreach (GameObject prefab in pieces)
            {
                Piece piece = prefab.GetComponent<Piece>();
                if (piece == null)
                {
                    continue;
                }
                foreach (Smelter smelter in prefab.GetComponentsInChildren<Smelter>(true))
                {
                    foreach (Smelter.ItemConversion conversion in smelter.m_conversion)
                    {
                        Convert(walk, AddPiece(piece, walk, added), conversion.m_from, conversion.m_to);
                    }
                }
                foreach (Fermenter fermenter in prefab.GetComponentsInChildren<Fermenter>(true))
                {
                    foreach (Fermenter.ItemConversion conversion in fermenter.m_conversion)
                    {
                        Convert(walk, AddPiece(piece, walk, added), conversion.m_from, conversion.m_to);
                    }
                }
                foreach (CookingStation cooking in prefab.GetComponentsInChildren<CookingStation>(true))
                {
                    foreach (CookingStation.ItemConversion conversion in cooking.m_conversion)
                    {
                        Convert(walk, AddPiece(piece, walk, added), conversion.m_from, conversion.m_to);
                    }
                }
            }
        }

        // A piece is its build cost plus the station it is built at.
        private static string AddPiece(Piece piece, Walk walk, HashSet<Piece> added)
        {
            string key = PiecePrefix + piece.name;
            if (!added.Add(piece))
            {
                return key;
            }
            Way way = new Way();
            foreach (Piece.Requirement requirement in piece.m_resources)
            {
                if (requirement.m_resItem != null && requirement.m_amount > 0)
                {
                    way.all.Add(requirement.m_resItem.name);
                }
            }
            Piece station = piece.m_craftingStation != null ? piece.m_craftingStation.GetComponent<Piece>() : null;
            if (station != null && station != piece)
            {
                way.all.Add(AddPiece(station, walk, added));
            }
            walk.Add(key, way);
            return key;
        }

        private static void Convert(Walk walk, string station, ItemDrop from, ItemDrop to)
        {
            if (from != null && to != null)
            {
                Way way = new Way();
                way.all.Add(from.name);
                way.all.Add(station);
                walk.Add(to.name, way);
            }
        }

        // The latest tier an item needs, null for an allowed item. An item with
        // several ways to make it is forbidden only when every way is.
        private static ForbiddenItem Resolve(string name, Walk walk)
        {
            if (walk.allowed.Contains(name))
            {
                return null;
            }
            if (walk.memo.TryGetValue(name, out ForbiddenItem known))
            {
                return known;
            }
            if (!walk.visiting.Add(name))
            {
                return null;
            }
            walk.listed.TryGetValue(name, out ForbiddenItem best);
            // A listed material comes from somewhere a recipe cannot replace,
            // a boss drop or a rock only a later pickaxe mines.
            if (best == null && walk.ways.TryGetValue(name, out List<Way> list))
            {
                ForbiddenItem easiest = null;
                foreach (Way way in list)
                {
                    ForbiddenItem needs = Needs(way, walk);
                    if (needs == null)
                    {
                        easiest = null;
                        break;
                    }
                    if (easiest == null || needs.tier < easiest.tier)
                    {
                        easiest = needs;
                    }
                }
                best = easiest;
            }
            walk.visiting.Remove(name);
            walk.memo[name] = best;
            return best;
        }

        private static ForbiddenItem Needs(Way way, Walk walk)
        {
            ForbiddenItem result = null;
            foreach (string input in way.all)
            {
                ForbiddenItem needs = Resolve(input, walk);
                if (needs != null && (result == null || needs.tier > result.tier))
                {
                    result = needs;
                }
            }
            if (way.anyOne.Count > 0)
            {
                // Any one ingredient is enough, the cheapest decides.
                ForbiddenItem cheapest = null;
                foreach (string input in way.anyOne)
                {
                    ForbiddenItem needs = Resolve(input, walk);
                    if (needs == null)
                    {
                        cheapest = null;
                        break;
                    }
                    if (cheapest == null || needs.tier < cheapest.tier)
                    {
                        cheapest = needs;
                    }
                }
                if (cheapest != null && (result == null || cheapest.tier > result.tier))
                {
                    result = cheapest;
                }
            }
            return result;
        }

        // The name a player reads.
        public static string Title(string prefab)
        {
            GameObject found = ObjectDB.instance?.GetItemPrefab(prefab);
            string name = found?.GetComponent<ItemDrop>()?.m_itemData.m_shared.m_name;
            if (string.IsNullOrEmpty(name))
            {
                return prefab;
            }
            return Localization.instance != null ? Localization.instance.Localize(name) : name;
        }
    }
}
