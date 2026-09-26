using HarmonyLib;
using UnityEngine;

namespace Blackforge
{
    // A locomotive or wagon goes only onto track. Its ghost sits on the
    // centerline of the track under the cursor and turns along it. The hammer
    // rotation picks which way it faces. Off the track the ghost is red and
    // cannot be placed.
    public static class Placement
    {
        public static bool IsVehicle(string prefab)
        {
            return prefab == "bf_locomotive" || prefab == "bf_wagon";
        }

        [HarmonyPatch(typeof(Player), "UpdatePlacementGhost")]
        private static class OnTrackOnly
        {
            private static void Postfix(Player __instance)
            {
                GameObject ghost = __instance.m_placementGhost;
                if (ghost == null || !ghost.activeSelf || !IsVehicle(ghost.name))
                {
                    return;
                }
                if (!OnTrack(__instance, ghost))
                {
                    __instance.m_placementStatus = Player.PlacementStatus.Invalid;
                }
                __instance.SetPlacementGhostValid(__instance.m_placementStatus == Player.PlacementStatus.Valid);
            }
        }

        private static bool OnTrack(Player player, GameObject ghost)
        {
            Transform camera = GameCamera.instance.transform;
            if (!Physics.Raycast(camera.position, camera.forward, out RaycastHit hit, 50f, player.m_placeRayMask))
            {
                return false;
            }
            RailTrack track = hit.collider.GetComponentInParent<RailTrack>();
            if (track == null || !track.Closest(hit.point, out Vector3 point, out Vector3 forward))
            {
                return false;
            }
            Vector3 facing = ghost.transform.rotation * Vector3.forward;
            if (Vector3.Dot(facing, forward) < 0f)
            {
                forward = -forward;
            }
            ghost.transform.position = point;
            ghost.transform.rotation = Quaternion.LookRotation(forward, Vector3.up);
            return true;
        }
    }
}
