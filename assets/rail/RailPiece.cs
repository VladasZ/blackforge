using HarmonyLib;
using UnityEngine;

namespace Blackforge
{
    // Marks a rail piece. A rail piece takes no damage from hits, fire or
    // weather. Only a lack of support breaks track or a station, and that
    // goes through ApplyDamage, which stays open.
    public class RailPiece : MonoBehaviour
    {
        [HarmonyPatch(typeof(WearNTear), "RPC_Damage")]
        private static class NoHits
        {
            private static bool Prefix(WearNTear __instance)
            {
                return __instance.GetComponent<RailPiece>() == null;
            }
        }
    }
}
