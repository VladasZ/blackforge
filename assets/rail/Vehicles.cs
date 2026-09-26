using System.Linq;
using HarmonyLib;
using UnityEngine;

namespace Blackforge
{
    // A locomotive or wagon in the scene. It shows where the saved data says
    // it is, the owner of the train moves that data.
    public class RailBody : MonoBehaviour
    {
        private ZNetView m_nview;

        private void Awake()
        {
            m_nview = GetComponent<ZNetView>();
            Rigidbody body = gameObject.AddComponent<Rigidbody>();
            body.isKinematic = true;
        }

        private void Start()
        {
            // A new piece takes the track it was placed on.
            if (m_nview.IsValid() && m_nview.IsOwner() && string.IsNullOrEmpty(m_nview.GetZDO().GetString(Train.Line)))
            {
                Graph.Rebuild();
                Train.Place(m_nview.GetZDO(), transform);
            }
        }

        private Vector3 m_lastSaved;
        private float m_savedAt;

        private void LateUpdate()
        {
            if (m_nview == null || !m_nview.IsValid())
            {
                return;
            }
            ZDO zdo = m_nview.GetZDO();
            Vector3 saved = zdo.GetPosition();
            Quaternion rotation = zdo.GetRotation();
            if (zdo.IsOwner())
            {
                transform.SetPositionAndRotation(saved, rotation);
                return;
            }
            // Another machine moves this train and its position comes in a
            // few times a second. Between updates it glides on at its speed.
            if (saved != m_lastSaved)
            {
                m_lastSaved = saved;
                m_savedAt = Time.time;
            }
            float speed = LocomotiveSpeed(zdo);
            Vector3 predicted = saved + rotation * Vector3.forward * speed * Mathf.Min(Time.time - m_savedAt, 0.3f);
            float follow = Mathf.Clamp01(Time.deltaTime * 20f);
            if ((transform.position - predicted).sqrMagnitude > 25f)
            {
                follow = 1f;
            }
            transform.position = Vector3.Lerp(transform.position, predicted, follow);
            transform.rotation = Quaternion.Slerp(transform.rotation, rotation, follow);
        }

        // A wagon takes the speed of the locomotive that pulls it.
        private float LocomotiveSpeed(ZDO zdo)
        {
            if (zdo.GetPrefab() == Train.LocomotiveHash)
            {
                return zdo.GetFloat(Train.Speed);
            }
            foreach (ZDO loco in Train.Trains())
            {
                if (Train.WagonIds(loco).Contains(zdo.m_uid))
                {
                    return loco.GetFloat(Train.Speed);
                }
            }
            return 0f;
        }
    }

    // The locomotive: the station menu and the coal.
    public class RailLocomotive : MonoBehaviour, Hoverable, Interactable
    {
        private ZNetView m_nview;

        private void Awake()
        {
            m_nview = GetComponent<ZNetView>();
        }

        public ZDOID Id => m_nview.GetZDO().m_uid;

        private ParticleSystem[] m_smoke;
        private float[] m_smokeRates;

        // The chimney smokes while there is coal to burn, more the faster the
        // train goes.
        private void Update()
        {
            Transform smoke = transform.Find("smoke");
            if (smoke == null || !m_nview.IsValid())
            {
                return;
            }
            ZDO zdo = m_nview.GetZDO();
            bool burning = zdo.GetInt(Train.Coal) > 0;
            if (smoke.gameObject.activeSelf != burning)
            {
                smoke.gameObject.SetActive(burning);
            }
            if (m_smoke == null)
            {
                m_smoke = smoke.GetComponentsInChildren<ParticleSystem>(true);
                m_smokeRates = m_smoke.Select(s => s.emission.rateOverTimeMultiplier).ToArray();
            }
            float boost = 1f + Mathf.Abs(zdo.GetFloat(Train.Speed)) * 0.4f;
            for (int i = 0; i < m_smoke.Length; i++)
            {
                ParticleSystem.EmissionModule emission = m_smoke[i].emission;
                emission.rateOverTimeMultiplier = m_smokeRates[i] * boost;
            }
        }

        public string GetHoverText()
        {
            if (!m_nview.IsValid())
            {
                return "";
            }
            ZDO zdo = m_nview.GetZDO();
            string state = zdo.GetBool(Train.Moving) ? "on its way" : zdo.GetLong(Train.Driver) != 0L ? "driven" : "waiting";
            return Localization.instance.Localize(
                $"Locomotive, {state}\nCoal {zdo.GetInt(Train.Coal)} of {Train.MaxCoal}, wagons {Train.WagonIds(zdo).Count} of {Train.MaxWagons}\n" +
                "[<color=yellow><b>$KEY_Use</b></color>] Stations\n[<color=yellow><b>$KEY_AltPlace + $KEY_Use</b></color>] Add coal");
        }

        public string GetHoverName()
        {
            return "Locomotive";
        }

        public float GetHoverOffset()
        {
            return 0f;
        }

        public bool Interact(Humanoid user, bool hold, bool alt)
        {
            if (hold || !m_nview.IsValid())
            {
                return false;
            }
            if (alt)
            {
                AddCoal(user);
                return true;
            }
            ZRoutedRpc.instance.InvokeRoutedRPC(Network.Server, "bf_list", Id);
            return true;
        }

        public bool UseItem(Humanoid user, ItemDrop.ItemData item)
        {
            if (item?.m_dropPrefab != null && item.m_dropPrefab.name == "Coal")
            {
                AddCoal(user);
                return true;
            }
            return false;
        }

        private void AddCoal(Humanoid user)
        {
            Inventory inventory = user.GetInventory();
            ZDO zdo = m_nview.GetZDO();
            int room = Train.MaxCoal - zdo.GetInt(Train.Coal);
            // With free crafting or free building, like on the test server, the
            // locomotive fills up for nothing.
            bool free = ZoneSystem.instance.GetGlobalKey(GlobalKeys.NoCraftCost) || ZoneSystem.instance.GetGlobalKey(GlobalKeys.NoBuildCost);
            int amount = free ? room : Mathf.Min(room, inventory.CountItems("$item_coal"));
            if (amount <= 0)
            {
                user.Message(MessageHud.MessageType.Center, room <= 0 ? "The locomotive is full of coal" : "You have no coal");
                return;
            }
            if (!free)
            {
                inventory.RemoveItem("$item_coal", amount);
            }
            Network.ToOwner(Id, "bf_coal", Id, amount);
            user.Message(MessageHud.MessageType.Center, $"{amount} coal added");
        }
    }

    // The seat of the locomotive. Sitting there drives the train by hand:
    // forward and backward set the speed, left and right pick the branch at
    // the next switch, jump gets up.
    public class RailSeat : MonoBehaviour, Hoverable, Interactable, IDoodadController
    {
        public Transform m_attach;
        private RailLocomotive m_loco;

        private void Awake()
        {
            m_loco = GetComponentInParent<RailLocomotive>();
        }

        public string GetHoverText()
        {
            return Localization.instance.Localize("[<color=yellow><b>$KEY_Use</b></color>] Drive\nForward and back set the speed, left and right pick the branch");
        }

        public string GetHoverName()
        {
            return "Driver seat";
        }

        public float GetHoverOffset()
        {
            return 0f;
        }

        public bool Interact(Humanoid user, bool hold, bool alt)
        {
            Player player = user as Player;
            if (hold || player == null || m_loco == null)
            {
                return false;
            }
            ZDOID id = m_loco.Id;
            Network.ToOwner(id, "bf_drive", id, ZDOMan.GetSessionID(), true);
            player.StartDoodadControl(this);
            player.AttachStart(m_attach, null, hideWeapons: false, isBed: false, onShip: true, "attach_chair", new Vector3(0f, 0.5f, 0f));
            return true;
        }

        public bool UseItem(Humanoid user, ItemDrop.ItemData item)
        {
            return false;
        }

        public void OnUseStop(Player player)
        {
            if (m_loco != null)
            {
                ZDOID id = m_loco.Id;
                Train.Controls.Remove(id);
                Network.ToOwner(id, "bf_drive", id, ZDOMan.GetSessionID(), false);
            }
            player.AttachStop();
        }

        public void ApplyControlls(Vector3 moveDir, Vector3 lookDir, bool run, bool autoRun, bool block)
        {
            if (m_loco != null)
            {
                int side = moveDir.x > 0.3f ? 1 : moveDir.x < -0.3f ? -1 : 0;
                Train.Controls[m_loco.Id] = (Mathf.Clamp(moveDir.z, -1f, 1f), side);
            }
        }

        public Component GetControlledComponent()
        {
            return m_loco;
        }

        public Vector3 GetPosition()
        {
            return transform.position;
        }

        public bool IsValid()
        {
            return this != null && m_loco != null;
        }
    }

    // Wagons couple at the back of a train with the alternative use key, the
    // same key uncouples the last wagon. A wagon also stays a container.
    public static class Coupling
    {
        public const float Reach = 2.5f;

        [HarmonyPatch(typeof(Container), "Interact")]
        private static class CoupleOnAlt
        {
            private static bool Prefix(Container __instance, Humanoid character, bool hold, bool alt, ref bool __result)
            {
                if (!alt || hold || __instance.GetComponent<RailBody>() == null)
                {
                    return true;
                }
                __result = Toggle(__instance.GetComponent<ZNetView>(), character);
                return false;
            }
        }

        [HarmonyPatch(typeof(Container), "GetHoverText")]
        private static class CoupleHover
        {
            private static void Postfix(Container __instance, ref string __result)
            {
                if (__instance.GetComponent<RailBody>() != null)
                {
                    __result += Localization.instance.Localize("\n[<color=yellow><b>$KEY_AltPlace + $KEY_Use</b></color>] Couple or uncouple");
                }
            }
        }

        private static bool Toggle(ZNetView view, Humanoid user)
        {
            if (view == null || !view.IsValid())
            {
                return false;
            }
            ZDOID wagon = view.GetZDO().m_uid;
            foreach (ZDO loco in Train.Trains())
            {
                if (Train.WagonIds(loco).LastOrDefault() == wagon)
                {
                    Network.ToOwner(loco.m_uid, "bf_couple", loco.m_uid, wagon, false);
                    user.Message(MessageHud.MessageType.Center, "Wagon uncoupled");
                    return true;
                }
            }
            Vector3 here = view.transform.position;
            foreach (ZDO loco in Train.Trains())
            {
                int count = Train.WagonIds(loco).Count;
                if (count >= Train.MaxWagons || !Train.Load(loco, out TrackCursor rear))
                {
                    continue;
                }
                float back = Train.FirstGap + count * Train.WagonGap;
                rear.Walk(-back, (steps, backward) => TrackCursor.Straightest(steps, -rear.Forward, 0));
                if ((rear.Position - here).sqrMagnitude < Reach * Reach)
                {
                    Network.ToOwner(loco.m_uid, "bf_couple", loco.m_uid, wagon, true);
                    user.Message(MessageHud.MessageType.Center, "Wagon coupled");
                    return true;
                }
            }
            user.Message(MessageHud.MessageType.Center, "Move the wagon right behind a train to couple it");
            return true;
        }
    }
}
