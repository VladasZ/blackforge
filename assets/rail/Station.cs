using System.Collections.Generic;
using System.Linq;
using UnityEngine;

namespace Blackforge
{
    // A station is named like a portal. The name goes through the server,
    // which refuses a name another station has. A station without a name is
    // not in the locomotive menu. Its stop is the track under its origin.
    public class Station : MonoBehaviour, Hoverable, Interactable, TextReceiver
    {
        public const string Name = "bf_name";
        public static readonly int Hash = "bf_station".GetStableHashCode();

        private ZNetView m_nview;

        private void Awake()
        {
            m_nview = GetComponent<ZNetView>();
        }

        public static IEnumerable<ZDO> All()
        {
            return ZDOMan.instance.m_objectsByID.Values.Where(z => z.GetPrefab() == Hash).ToList();
        }

        public string GetText()
        {
            return m_nview != null && m_nview.IsValid() ? m_nview.GetZDO().GetString(Name) : "";
        }

        public void SetText(string text)
        {
            if (m_nview != null && m_nview.IsValid())
            {
                ZRoutedRpc.instance.InvokeRoutedRPC(Network.Server, "bf_name", m_nview.GetZDO().m_uid, text);
            }
        }

        public string GetHoverText()
        {
            string name = GetText();
            string title = string.IsNullOrEmpty(name) ? "Train station, no name yet" : $"Station {name}";
            return Localization.instance.Localize($"{title}\n[<color=yellow><b>$KEY_Use</b></color>] Set name");
        }

        public string GetHoverName()
        {
            return "Train station";
        }

        public float GetHoverOffset()
        {
            return 0f;
        }

        public bool Interact(Humanoid user, bool hold, bool alt)
        {
            if (hold)
            {
                return false;
            }
            TextInput.instance.RequestText(this, "Station name", 20);
            return true;
        }

        public bool UseItem(Humanoid user, ItemDrop.ItemData item)
        {
            return false;
        }
    }
}
