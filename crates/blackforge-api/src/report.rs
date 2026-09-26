//! What the game tells the backend about a join that failed or a connection
//! that dropped, so a bad connection can be debugged after the fact. The join
//! plugin writes it, the app adds its own version and sends it on to
//! `POST /api/reports`. The admin reads them with `GET /api/reports` and
//! `GET /api/reports/{id}`, see `docs/reports.md`.

use serde::{Deserialize, Serialize};

/// At most this many bytes of game logs are kept per report, the newest ones.
pub const LOG_BYTES: usize = 512 * 1024;
/// At most this many bytes of the app log are kept per report, the newest.
pub const APP_LOG_BYTES: usize = 128 * 1024;
/// At most this many trail and traffic lines are kept per report, the newest.
pub const TRAIL_LINES: usize = 200;
/// How many reports `GET /api/reports` lists at most.
pub const LIST_LIMIT: i64 = 200;

/// The body of `POST /api/reports`. The join plugin writes it by hand in C#,
/// so every field has a default and an older plugin still gets through.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConnectionReport {
    /// The id of the server of the last join button, empty for a join the
    /// plugin did not start.
    pub server_id: String,
    /// The server name, empty when unknown.
    pub server: String,
    /// What failed. A `ZNet.ConnectionStatus` like `ErrorConnectFailed`, or
    /// one of the plugin's own, like `NotOnline` or `Refused`.
    pub status: String,
    /// The text the player saw.
    pub message: String,
    /// The player was in the world when the connection dropped.
    pub in_world: bool,
    /// Seconds from the click on the join button to the failure.
    pub seconds: f64,
    pub game_version: String,
    pub plugin_version: String,
    /// Filled by the app, the plugin does not know it.
    pub app_version: String,
    pub os: String,
    /// The character the player joined with, empty when unknown.
    pub character: String,
    /// The local `PlayFab` entity id, the id the game server logs as
    /// `playfab/<id>`.
    pub playfab_id: String,
    /// The platform user id, like `Steam_7656...`.
    pub platform_id: String,
    /// Every plugin `BepInEx` loaded, `guid name version`.
    pub mods: Vec<String>,
    /// What the plugin saw since the click, one line per step with its time.
    pub trail: Vec<String>,
    /// The messages in and out of the `ZPlayFabSocket` of the game from the
    /// connect until the world loads, the newest ones.
    pub traffic: Vec<String>,
    /// The newest lines of `BepInEx/LogOutput.log` and of the Unity
    /// `Player.log`.
    pub log: String,
    /// The newest lines of the app log, filled by the app.
    pub app_log: String,
}

impl ConnectionReport {
    /// Cuts the trail and the log to their limits, keeping the newest part.
    pub fn trim(&mut self) {
        keep_last_lines(&mut self.trail);
        keep_last_lines(&mut self.traffic);
        keep_last_bytes(&mut self.log, LOG_BYTES);
        keep_last_bytes(&mut self.app_log, APP_LOG_BYTES);
    }
}

fn keep_last_lines(lines: &mut Vec<String>) {
    if lines.len() > TRAIL_LINES {
        lines.drain(..lines.len() - TRAIL_LINES);
    }
}

/// Keeps the newest bytes, cut on a letter boundary.
pub fn keep_last_bytes(text: &mut String, bytes: usize) {
    if text.len() > bytes {
        let mut start = text.len() - bytes;
        while !text.is_char_boundary(start) {
            start += 1;
        }
        text.drain(..start);
    }
}

/// One row of `GET /api/reports`, the newest first. The trail and the log are
/// only in `GET /api/reports/{id}`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReportRow {
    pub id: i64,
    /// The account id of the sender.
    pub user_id: String,
    pub username: String,
    /// RFC 3339.
    pub created_at: String,
    pub server: String,
    pub character: String,
    pub status: String,
    pub message: String,
    pub in_world: bool,
    pub seconds: f64,
}

/// `GET /api/reports/{id}`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StoredReport {
    pub id: i64,
    pub user_id: String,
    pub username: String,
    pub created_at: String,
    pub report: ConnectionReport,
}

#[cfg(test)]
mod tests {
    use serde_json::from_str;

    use super::{ConnectionReport, LOG_BYTES, TRAIL_LINES};

    // The join plugin writes this by hand in C#, so the keys are fixed here.
    #[test]
    fn plugin_shape_reads() {
        let report: ConnectionReport = from_str(
            r#"{"server_id":"e3ce","server":"Arkham Asylum","status":"ErrorConnectFailed","message":"Failed to connect","in_world":false,"seconds":12.5,"game_version":"0.221.4","plugin_version":"4.2.0","os":"Windows 11","character":"Huilusha","playfab_id":"7EF33498445FE8C6","platform_id":"Steam_76561198108627367","mods":["a A 1.0.0"],"trail":["0.0 click"],"traffic":["in ack id 3, 5 bytes"],"log":"line"}"#,
        )
        .unwrap();
        assert_eq!(report.server, "Arkham Asylum");
        assert_eq!(report.status, "ErrorConnectFailed");
        assert_eq!(report.trail, ["0.0 click"]);
        assert_eq!(report.character, "Huilusha");
        assert_eq!(report.playfab_id, "7EF33498445FE8C6");
        assert_eq!(report.mods, ["a A 1.0.0"]);
        assert_eq!(report.traffic, ["in ack id 3, 5 bytes"]);
        assert!(report.app_version.is_empty());
    }

    #[test]
    fn missing_fields_read_as_empty() {
        let report: ConnectionReport = from_str(r#"{"status":"NotOnline"}"#).unwrap();
        assert_eq!(report.status, "NotOnline");
        assert!(report.log.is_empty());
    }

    #[test]
    fn trim_keeps_the_newest_part() {
        let mut report = ConnectionReport {
            trail: (0..TRAIL_LINES + 5).map(|line| line.to_string()).collect(),
            // A two byte letter on the cut must not split.
            log: format!("old{}new", "ж".repeat(LOG_BYTES)),
            ..ConnectionReport::default()
        };
        report.trim();
        assert_eq!(report.trail.len(), TRAIL_LINES);
        assert_eq!(report.trail[0], "5");
        assert!(report.log.len() <= LOG_BYTES);
        assert!(report.log.ends_with("new"));
        assert!(!report.log.starts_with("old"));
    }
}
