//! Compact ISO-8601 timestamps for default `--out` filenames.
//!
//! `std::time` gives us seconds since `UNIX_EPOCH` but no civil-calendar
//! conversion. Rather than pull in `time` / `chrono` for one filename
//! suffix, we adapt Howard Hinnant's `civil_from_days` algorithm — a
//! well-known ~10-line derivation from the proleptic Gregorian rules.
//! Reference: <https://howardhinnant.github.io/date_algorithms.html>.
//!
//! Output format is `YYYYMMDDTHHMMSSZ` (e.g. `20260721T143022Z`).
//! Separator-free is deliberate: `:` is reserved in Windows paths and
//! awkward to escape in shells; both forms are valid ISO-8601, but the
//! compact form is universally filesystem-safe.

use std::time::{SystemTime, UNIX_EPOCH};

/// Render the current UTC time as a compact ISO-8601 string suitable
/// for use in a filename: `YYYYMMDDTHHMMSSZ`.
///
/// Returns `"19700101T000000Z"` if the system clock is before
/// `UNIX_EPOCH` (only possible on misconfigured hosts); the caller
/// gets a usable filename either way.
pub(crate) fn iso8601_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    format_iso8601(secs)
}

/// Pure conversion: seconds-since-epoch → `YYYYMMDDTHHMMSSZ`.
/// Split out so the calendar math is unit-testable without touching
/// the wall clock.
fn format_iso8601(secs: u64) -> String {
    let days = secs / 86_400;
    let tod = secs % 86_400;
    let hour = tod / 3_600;
    let min = (tod % 3_600) / 60;
    let sec = tod % 60;

    let (year, month, day) = civil_from_days(days);
    format!("{year:04}{month:02}{day:02}T{hour:02}{min:02}{sec:02}Z")
}

/// Hinnant's `civil_from_days` adapted to `u64`: days since
/// 1970-01-01 → `(year, month, day)`.
///
/// The original algorithm is sign-aware to support pre-epoch dates;
/// we don't need that (filename timestamps are always post-1970), so
/// the `era` branch collapses to plain `z / 146_097` and every value
/// stays non-negative. Returns `(year, month 1-12, day 1-31)` as u64
/// to skip truncating casts — `format!` handles them directly.
fn civil_from_days(z: u64) -> (u64, u64, u64) {
    // Shift epoch from 1970-01-01 to 0000-03-01 (start of a 400-year
    // Gregorian cycle). The algorithm below relies on this anchor so
    // the leap-year pattern collapses to plain arithmetic.
    let z = z + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097; // [0, 146_096]
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pinned epoch second → known UTC date. Guards against accidental
    /// arithmetic drift if anyone touches `civil_from_days`.
    #[test]
    fn format_iso8601_known_epoch() {
        // 1_700_000_000 = 2023-11-14T22:13:20 UTC.
        assert_eq!(format_iso8601(1_700_000_000), "20231114T221320Z");
    }

    /// The UNIX epoch itself — boundary condition.
    #[test]
    fn format_iso8601_epoch() {
        assert_eq!(format_iso8601(0), "19700101T000000Z");
    }

    /// One day past the epoch — exercises the day-rollover path.
    #[test]
    fn format_iso8601_one_day() {
        assert_eq!(format_iso8601(86_400), "19700102T000000Z");
    }

    /// Leap day: 2024-02-29 exists. Catches off-by-one in the
    /// leap-year branch.
    #[test]
    fn format_iso8601_leap_day() {
        // 2024-02-29T12:34:56 UTC. Pinned via `date -u -d`.
        assert_eq!(format_iso8601(1_709_210_096), "20240229T123456Z");
    }

    /// Year rollover across the Gregorian boundary (2023-12-31 →
    /// 2024-01-01). Exercises the `m <= 2` year-increment branch.
    #[test]
    fn format_iso8601_year_rollover() {
        // 2023-12-31T23:59:59 UTC.
        assert_eq!(format_iso8601(1_704_067_199), "20231231T235959Z");
        // 2024-01-01T00:00:00 UTC, one second later.
        assert_eq!(format_iso8601(1_704_067_200), "20240101T000000Z");
    }
}
