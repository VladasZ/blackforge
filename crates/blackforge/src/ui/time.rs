//! Times as the app shows them: how long ago on screen, the full date in a
//! tooltip.

use std::time::Duration;

use chrono::{DateTime, Local, Utc};
use timeago::{Formatter, TimeUnit};

/// How long ago `at`, seconds since the Unix epoch, was: "just now",
/// "5 minutes ago", "3 weeks ago".
pub fn ago(at: i64) -> String {
    let seconds = u64::try_from(Utc::now().timestamp() - at).unwrap_or(0);
    let mut formatter = Formatter::new();
    formatter.min_unit(TimeUnit::Minutes).too_low("just now");
    formatter.convert(Duration::from_secs(seconds))
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
