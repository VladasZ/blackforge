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
    // The backend lists only raw materials, boss drops and trophies. This walks
    // the recipes and the smelters, fermenters and cooking stations of the game
    // and forbids every item made from a forbidden material too.
    public static class Tiers
    {
        // The id of the world an item was found or made in. The game saves it
        // with the item, so fair loot of a competitive world stays allowed there.
        public const string TagKey = "blackforge.world";

        // One way to make an item. A recipe that takes any one of its
        // ingredients forbids the item only when all of them are forbidden.
        private class Way
        {
            public List<string> inputs = new List<string>();
            public bool anyOne;
        }

        public static string Tag(ItemDrop.ItemData item)
        {
            return item.m_customData.TryGetValue(TagKey, out string tag) ? tag : null;
        }

        // Every forbidden item by prefab name, with the boss that frees it. A
        // material the game does not know goes to warn, it is a typo in the
        // tiers of the backend.
        public static Dictionary<string, ForbiddenItem> Expand(List<ForbiddenItem> materials, Action<string> warn)
        {
            Dictionary<string, ForbiddenItem> result = new Dictionary<string, ForbiddenItem>();
            ObjectDB db = ObjectDB.instance;
            if (db == null || materials == null || materials.Count == 0)
            {
                return result;
            }
            Dictionary<string, ForbiddenItem> listed = new Dictionary<string, ForbiddenItem>();
            foreach (ForbiddenItem material in materials)
            {
                if (db.GetItemPrefab(material.item) == null)
                {
                    warn($"{material.item} of the tiers is no item of the game");
                }
                listed[material.item] = material;
            }
            Dictionary<string, List<Way>> ways = Ways(db);
            Dictionary<string, ForbiddenItem> memo = new Dictionary<string, ForbiddenItem>();
            HashSet<string> visiting = new HashSet<string>();
            foreach (GameObject prefab in db.m_items)
            {
                if (prefab == null)
                {
                    continue;
                }
                ForbiddenItem found = Resolve(prefab.name, listed, ways, memo, visiting);
                if (found != null)
                {
                    result[prefab.name] = found;
                }
            }
            return result;
        }

        private static Dictionary<string, List<Way>> Ways(ObjectDB db)
        {
            Dictionary<string, List<Way>> ways = new Dictionary<string, List<Way>>();
            void Add(string product, Way way)
            {
                if (!ways.TryGetValue(product, out List<Way> list))
                {
                    list = new List<Way>();
                    ways[product] = list;
                }
                list.Add(way);
            }
            foreach (Recipe recipe in db.m_recipes)
            {
                if (recipe == null || recipe.m_item == null || !recipe.m_enabled)
                {
                    continue;
                }
                Way way = new Way { anyOne = recipe.m_requireOnlyOneIngredient };
                foreach (Piece.Requirement requirement in recipe.m_resources)
                {
                    // An ingredient with no amount is only for the upgrades,
                    // the first level of the item does not need it. An idol
                    // is only taken by the upgrade station.
                    if (requirement.m_resItem != null && requirement.m_amount > 0 && !requirement.m_upgraderResource)
                    {
                        way.inputs.Add(requirement.m_resItem.name);
                    }
                }
                if (way.inputs.Count > 0)
                {
                    Add(recipe.m_item.name, way);
                }
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
            foreach (GameObject piece in pieces)
            {
                foreach (Smelter smelter in piece.GetComponentsInChildren<Smelter>(true))
                {
                    foreach (Smelter.ItemConversion conversion in smelter.m_conversion)
                    {
                        Convert(Add, conversion.m_from, conversion.m_to);
                    }
                }
                foreach (Fermenter fermenter in piece.GetComponentsInChildren<Fermenter>(true))
                {
                    foreach (Fermenter.ItemConversion conversion in fermenter.m_conversion)
                    {
                        Convert(Add, conversion.m_from, conversion.m_to);
                    }
                }
                foreach (CookingStation station in piece.GetComponentsInChildren<CookingStation>(true))
                {
                    foreach (CookingStation.ItemConversion conversion in station.m_conversion)
                    {
                        Convert(Add, conversion.m_from, conversion.m_to);
                    }
                }
            }
            return ways;
        }

        private static void Convert(Action<string, Way> add, ItemDrop from, ItemDrop to)
        {
            if (from != null && to != null)
            {
                Way way = new Way();
                way.inputs.Add(from.name);
                add(to.name, way);
            }
        }

        // The latest tier an item needs, null for an allowed item. An item with
        // several ways to make it is forbidden only when every way is.
        private static ForbiddenItem Resolve(
            string name,
            Dictionary<string, ForbiddenItem> listed,
            Dictionary<string, List<Way>> ways,
            Dictionary<string, ForbiddenItem> memo,
            HashSet<string> visiting)
        {
            if (memo.TryGetValue(name, out ForbiddenItem known))
            {
                return known;
            }
            if (!visiting.Add(name))
            {
                return null;
            }
            listed.TryGetValue(name, out ForbiddenItem best);
            if (ways.TryGetValue(name, out List<Way> list))
            {
                ForbiddenItem easiest = null;
                bool allForbidden = true;
                foreach (Way way in list)
                {
                    ForbiddenItem needs = Needs(way, listed, ways, memo, visiting);
                    if (needs == null)
                    {
                        allForbidden = false;
                        break;
                    }
                    if (easiest == null || needs.tier < easiest.tier)
                    {
                        easiest = needs;
                    }
                }
                if (allForbidden && easiest != null && (best == null || easiest.tier > best.tier))
                {
                    best = easiest;
                }
            }
            visiting.Remove(name);
            memo[name] = best;
            return best;
        }

        private static ForbiddenItem Needs(
            Way way,
            Dictionary<string, ForbiddenItem> listed,
            Dictionary<string, List<Way>> ways,
            Dictionary<string, ForbiddenItem> memo,
            HashSet<string> visiting)
        {
            ForbiddenItem result = null;
            foreach (string input in way.inputs)
            {
                ForbiddenItem needs = Resolve(input, listed, ways, memo, visiting);
                if (way.anyOne)
                {
                    // Any one ingredient is enough, the cheapest decides.
                    if (needs == null)
                    {
                        return null;
                    }
                    if (result == null || needs.tier < result.tier)
                    {
                        result = needs;
                    }
                }
                else if (needs != null && (result == null || needs.tier > result.tier))
                {
                    result = needs;
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
