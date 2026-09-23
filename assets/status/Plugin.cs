using System;
using System.IO;
using System.Text;
using BepInEx;
using UnityEngine;

namespace Blackforge
{
    // Runs on the dedicated server only. Every 2 seconds it writes the peers the
    // game has connected right now to the file named by BLACKFORGE_STATUS_FILE.
    // That is the one exact and fast answer to who is on the server. The game's
    // own count line comes every 10 minutes, and the join and leave lines
    // disagree with each other. A file older than a few seconds means the server
    // is not running or not ready, and a reader must treat it as unknown.
    [BepInPlugin("xyz.vladas.blackforge.status", "Blackforge Status", "1.0.0")]
    public class StatusPlugin : BaseUnityPlugin
    {
        private const float Interval = 2f;

        private string path;
        private float next;

        private void Awake()
        {
            path = Environment.GetEnvironmentVariable("BLACKFORGE_STATUS_FILE");
            if (string.IsNullOrEmpty(path))
            {
                Logger.LogWarning("BLACKFORGE_STATUS_FILE is not set, no status is written");
                enabled = false;
            }
        }

        private void Update()
        {
            if (Time.unscaledTime < next)
            {
                return;
            }
            next = Time.unscaledTime + Interval;
            ZNet net = ZNet.instance;
            if (net == null || !net.IsServer() || !net.IsDedicated())
            {
                return;
            }
            try
            {
                Write(net);
            }
            catch (Exception error)
            {
                Logger.LogWarning($"{path} was not written: {error.Message}");
            }
        }

        private void Write(ZNet net)
        {
            StringBuilder json = new StringBuilder();
            json.Append("{\"updated\":").Append(DateTimeOffset.UtcNow.ToUnixTimeSeconds());
            json.Append(",\"players\":[");
            bool first = true;
            // Every connected peer counts, also one that is still in the
            // password prompt or character select. For a restart it is a player.
            foreach (ZNetPeer peer in net.GetConnectedPeers())
            {
                if (!first)
                {
                    json.Append(',');
                }
                first = false;
                json.Append("{\"name\":\"").Append(Escape(peer.m_playerName));
                json.Append("\",\"id\":\"").Append(peer.m_uid).Append("\"}");
            }
            json.Append("]}");

            // A reader never sees half a file, the new one replaces the old in one step.
            string temp = path + ".tmp";
            File.WriteAllText(temp, json.ToString());
            if (File.Exists(path))
            {
                File.Replace(temp, path, null);
            }
            else
            {
                File.Move(temp, path);
            }
        }

        private static string Escape(string value)
        {
            StringBuilder escaped = new StringBuilder();
            foreach (char c in value ?? "")
            {
                if (c == '"' || c == '\\')
                {
                    escaped.Append('\\').Append(c);
                }
                else if (c < ' ')
                {
                    escaped.Append(' ');
                }
                else
                {
                    escaped.Append(c);
                }
            }
            return escaped.ToString();
        }
    }
}
