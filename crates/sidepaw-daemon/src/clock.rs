//! Timestamps. Events carry RFC 3339 strings so `sidepaw-core` needs no
//! time library; the daemon is the only place that asks what time it is.

use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

pub fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .replace_nanosecond(OffsetDateTime::now_utc().millisecond() as u32 * 1_000_000)
        .unwrap_or_else(|_| OffsetDateTime::now_utc())
        .format(&Rfc3339)
        .unwrap_or_default()
}

/// Midnight of the current local day as RFC 3339 in UTC, so it compares
/// with stored timestamps. Falls back to UTC midnight when the local
/// offset cannot be determined.
pub fn start_of_today_rfc3339() -> String {
    let now = OffsetDateTime::now_utc();
    let local = time::UtcOffset::current_local_offset()
        .map(|off| now.to_offset(off))
        .unwrap_or(now);
    local
        .replace_time(time::Time::MIDNIGHT)
        .to_offset(time::UtcOffset::UTC)
        .format(&Rfc3339)
        .unwrap_or_default()
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
