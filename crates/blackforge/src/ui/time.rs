//! Times as the app shows them: how long ago on screen, the full date in a
//! tooltip.

use chrono::{DateTime, Local, Utc};

const MINUTE: i64 = 60;
const HOUR: i64 = 60 * MINUTE;
const DAY: i64 = 24 * HOUR;
const WEEK: i64 = 7 * DAY;
const MONTH: i64 = 30 * DAY;
const YEAR: i64 = 365 * DAY;

/// How long ago `at`, seconds since the Unix epoch, was: "just now",
/// "5 minutes ago", "yesterday", "3 weeks ago".
pub fn ago(at: i64) -> String {
    let seconds = (Utc::now().timestamp() - at).max(0);
    if seconds < MINUTE {
        return "just now".to_owned();
    }
    if (DAY..2 * DAY).contains(&seconds) {
        return "yesterday".to_owned();
    }
    let (count, unit) = if seconds < HOUR {
        (seconds / MINUTE, "minute")
    } else if seconds < DAY {
        (seconds / HOUR, "hour")
    } else if seconds < WEEK {
        (seconds / DAY, "day")
    } else if seconds < MONTH {
        (seconds / WEEK, "week")
    } else if seconds < YEAR {
        (seconds / MONTH, "month")
    } else {
        (seconds / YEAR, "year")
    };
    if count == 1 {
        format!("1 {unit} ago")
    } else {
        format!("{count} {unit}s ago")
    }
}

/// The full date and time of `at` in the zone of this machine, for a
/// tooltip.
pub fn full(at: i64) -> String {
    DateTime::from_timestamp(at, 0).map_or_else(
        || "An unknown time".to_owned(),
        |time| {
            time.with_timezone(&Local)
                .format("%a %d %b %Y %H:%M")
                .to_string()
        },
    )
}

/// An RFC 3339 time as Thunderstore writes it, in seconds since the epoch.
pub fn parse(text: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(text)
        .ok()
        .map(|time| time.timestamp())
}
