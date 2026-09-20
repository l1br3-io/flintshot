//! The persistent recents index — which images the user recently
//! had open in the editor.
//!
//! Lives in core (not a shell) because every surface wants the same
//! answer bridge-agnostically: the home welcome grid renders it,
//! every routing point records into it, and the pure-path logic is
//! unit-testable without a GUI. Thumbnails are **never** persisted
//! — the index stores paths (+ mtime) only; the welcome grid
//! decodes and downscales on demand, in memory, so a huge history
//! costs a small TOML file, not a thumbnail cache.
//!
//! Stored at `<config_dir>/flintshot/recents.toml` (beside
//! `config.toml`), serde kebab-case per repo convention. Recents
//! are advisory: a missing, unreadable, or corrupt index starts
//! empty — never an error the user has to see.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{FlintshotError, Result};

/// How many entries the index keeps by default (`ui.recent-captures-
/// limit` overrides per call; clamped 1–200 by the config accessor).
/// Most recent first; anything past the cap drops off the back.
pub const DEFAULT_RECENTS_LIMIT: usize = 50;

/// One recents entry: an image path plus its last-recorded
/// modification time (unix seconds).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub struct RecentEntry {
    /// The image's filesystem path.
    pub path: PathBuf,
    /// Last-seen modification time, unix seconds (0 when
    /// unavailable at record time).
    pub mtime: u64,
}

/// The recents index: an ordered list, front = most recent.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub struct Recents {
    /// Entries, most recent first.
    #[serde(default)]
    pub entries: Vec<RecentEntry>,
}

impl Recents {
    /// An empty index.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Load from the platform location
    /// (`dirs::config_dir() / "flintshot" / "recents.toml"`).
    /// Missing / corrupt / unreadable → empty; dead paths pruned.
    #[must_use]
    pub fn load() -> Self {
        if let Some(path) = recents_path() {
            Self::load_from(&path)
        } else {
            tracing::warn!("no platform config_dir available; recents start empty");
            Self::default()
        }
    }

    /// Testable seam: load from an explicit index path. Missing,
    /// unreadable, or corrupt file → an empty index (advisory
    /// data, never fatal); entries whose paths no longer exist are
    /// pruned.
    #[must_use]
    pub fn load_from(index_path: &Path) -> Self {
        let Ok(contents) = std::fs::read_to_string(index_path) else {
            return Self::default();
        };
        match toml::from_str::<Self>(&contents) {
            Ok(mut recents) => {
                recents.prune_dead();
                recents
            }
            Err(e) => {
                tracing::warn!(
                    index = %index_path.display(),
                    error = %e,
                    "recents index unreadable; starting empty"
                );
                Self::default()
            }
        }
    }

    /// Record `path`: dedupe (an existing entry moves to the
    /// front), refresh its mtime, cap at `limit` (a zero limit is
    /// defensively treated as 1 — an empty index is never the
    /// intent). Load does **not** truncate, so a legacy index
    /// shorter than the new limit grows on the next record. Pure
    /// except for the mtime metadata read.
    pub fn record(&mut self, path: &Path, limit: usize) {
        self.entries.retain(|e| e.path != path);
        self.entries.insert(
            0,
            RecentEntry {
                path: path.to_path_buf(),
                mtime: mtime_secs(path),
            },
        );
        self.entries.truncate(limit.max(1));
    }

    /// Drop `path`'s entry (the card Delete's index half).
    /// Returns whether an entry existed. Order of the rest is
    /// preserved.
    pub fn remove(&mut self, path: &Path) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| e.path != path);
        self.entries.len() != before
    }

    /// The recorded paths, most recent first.
    #[must_use]
    pub fn paths(&self) -> Vec<&Path> {
        self.entries.iter().map(|e| e.path.as_path()).collect()
    }

    /// Save to the platform location, creating the parent
    /// directory if needed.
    ///
    /// # Errors
    /// [`FlintshotError::Io`] on write failure or when no platform
    /// config dir exists; [`FlintshotError::Config`] on
    /// serialization failure.
    pub fn save(&self) -> Result<()> {
        match recents_path() {
            Some(path) => self.save_to(&path),
            None => Err(FlintshotError::Config {
                path: PathBuf::new(),
                detail: "no platform config_dir available".into(),
            }),
        }
    }

    /// Testable seam: save to an explicit index path, creating the
    /// parent directory if needed.
    ///
    /// # Errors
    /// See [`Recents::save`].
    pub fn save_to(&self, index_path: &Path) -> Result<()> {
        if let Some(parent) = index_path.parent() {
            std::fs::create_dir_all(parent).map_err(FlintshotError::Io)?;
        }
        let body = toml::to_string_pretty(self).map_err(|e| FlintshotError::Config {
            path: index_path.to_path_buf(),
            detail: format!("serialization failure: {e}"),
        })?;
        std::fs::write(index_path, body).map_err(FlintshotError::Io)
    }

    /// Drop entries whose paths no longer exist.
    fn prune_dead(&mut self) {
        self.entries.retain(|e| e.path.exists());
    }
}

/// A path's modification time in unix seconds (0 when metadata is
/// unavailable — the entry stays, staleness display falls back).
fn mtime_secs(path: &Path) -> u64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs())
}

/// Record `path` into the index stored at `index_path`
/// (load → record → save). Failures warn and vanish — recents are
/// advisory; a routing point must never fail over them.
pub fn record_into(index_path: &Path, path: &Path, limit: usize) {
    let mut recents = Recents::load_from(index_path);
    recents.record(path, limit);
    if let Err(e) = recents.save_to(index_path) {
        tracing::warn!(error = %e, "updating the recents index; ignoring");
    }
}

/// Record `path` into the index at the platform location.
pub fn record(path: &Path, limit: usize) {
    if let Some(index_path) = recents_path() {
        record_into(&index_path, path, limit);
    } else {
        tracing::warn!("no platform config_dir available; recents not recorded");
    }
}

/// Drop `path`'s entry from the index stored at `index_path`
/// (load → remove → save). Returns whether an entry existed; save
/// failures warn (advisory data, same rule as [`record_into`]).
pub fn remove_from(index_path: &Path, path: &Path) -> bool {
    let mut recents = Recents::load_from(index_path);
    let removed = recents.remove(path);
    if removed {
        if let Err(e) = recents.save_to(index_path) {
            tracing::warn!(error = %e, "updating the recents index; ignoring");
        }
    }
    removed
}

/// Drop `path`'s entry from the index at the platform location.
#[must_use]
pub fn remove(path: &Path) -> bool {
    recents_path().is_some_and(|index_path| remove_from(&index_path, path))
}

/// Resolve `<config_dir>/flintshot/recents.toml`, or `None` where
/// `dirs::config_dir()` has no answer.
#[must_use]
pub fn recents_path() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("flintshot").join("recents.toml"))
}

/// Which day-group a capture's mtime lands in, against `now` —
/// the recents grid's Today / Yesterday / Earlier eyebrows.
///
/// Days compare as UTC civil days (unix seconds ÷ 86 400, the same
/// convention [`crate::output::chrono_date`] bakes into filename
/// templates); a far-from-UTC midnight skew is the accepted,
/// documented simplification — no timezone dependency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DayBucket {
    /// The same UTC civil day as `now`.
    Today,
    /// The UTC civil day before `now`'s.
    Yesterday,
    /// Anything older — or a clock-skewed future mtime.
    Earlier,
}

/// Bucket `mtime` (unix seconds) against `now`. A future mtime
/// (clock skew) lands [`DayBucket::Earlier`] rather than panicking.
#[must_use]
pub fn day_bucket(mtime: u64, now: u64) -> DayBucket {
    if mtime > now {
        return DayBucket::Earlier;
    }
    match now / 86_400 - mtime / 86_400 {
        0 => DayBucket::Today,
        1 => DayBucket::Yesterday,
        _ => DayBucket::Earlier,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn existing_png(dir: &Path, name: &str) -> PathBuf {
        let p = dir.join(name);
        fs::write(&p, b"png").unwrap();
        p
    }

    #[test]
    fn missing_index_loads_empty() {
        let dir = tempdir().unwrap();
        let r = Recents::load_from(&dir.path().join("nope.toml"));
        assert!(r.paths().is_empty());
    }

    #[test]
    fn save_load_roundtrip_preserves_order() {
        let dir = tempdir().unwrap();
        let index = dir.path().join("recents.toml");
        let a = existing_png(dir.path(), "a.png");
        let b = existing_png(dir.path(), "b.png");

        let mut r = Recents::new();
        r.record(&a, DEFAULT_RECENTS_LIMIT);
        r.record(&b, DEFAULT_RECENTS_LIMIT);
        r.save_to(&index).unwrap();

        let back = Recents::load_from(&index);
        assert_eq!(back.paths(), vec![b.as_path(), a.as_path()]);
    }

    #[test]
    fn record_dedupes_and_moves_to_front() {
        let dir = tempdir().unwrap();
        let a = existing_png(dir.path(), "a.png");
        let b = existing_png(dir.path(), "b.png");

        let mut r = Recents::new();
        r.record(&a, DEFAULT_RECENTS_LIMIT);
        r.record(&b, DEFAULT_RECENTS_LIMIT);
        r.record(&a, DEFAULT_RECENTS_LIMIT);

        assert_eq!(r.paths(), vec![a.as_path(), b.as_path()]);
        assert_eq!(r.entries.len(), 2, "the re-record must not duplicate");
    }

    #[test]
    fn record_truncates_at_the_configured_limit() {
        let dir = tempdir().unwrap();
        let mut r = Recents::new();
        for i in 0..5 {
            let p = existing_png(dir.path(), &format!("shot-{i}.png"));
            r.record(&p, 3);
        }
        assert_eq!(r.entries.len(), 3, "the configured limit caps the index");
        let newest = existing_png(dir.path(), "shot-4.png");
        assert_eq!(r.paths().first(), Some(&newest.as_path()));
        let oldest = existing_png(dir.path(), "shot-0.png");
        assert!(
            !r.paths().contains(&oldest.as_path()),
            "the oldest entries fall off the back"
        );
    }

    #[test]
    fn record_at_the_default_limit_caps_at_fifty() {
        let dir = tempdir().unwrap();
        let mut r = Recents::new();
        for i in 0..55 {
            let p = existing_png(dir.path(), &format!("shot-{i}.png"));
            r.record(&p, DEFAULT_RECENTS_LIMIT);
        }
        assert_eq!(r.entries.len(), DEFAULT_RECENTS_LIMIT);
        assert_eq!(DEFAULT_RECENTS_LIMIT, 50, "the shipped default");
    }

    /// Acceptance 5: a legacy 20-entry index loads unchanged and
    /// grows to the new limit on the next record.
    #[test]
    fn legacy_twenty_index_grows_to_the_new_limit_on_record() {
        let dir = tempdir().unwrap();
        let index = dir.path().join("recents.toml");
        let mut legacy = Recents::new();
        for i in 0..20 {
            let p = existing_png(dir.path(), &format!("old-{i}.png"));
            legacy.record(&p, 20);
        }
        legacy.save_to(&index).unwrap();

        let loaded = Recents::load_from(&index);
        assert_eq!(loaded.entries.len(), 20, "loads unchanged, no truncation");

        let mut grown = loaded;
        let fresh = existing_png(dir.path(), "new.png");
        grown.record(&fresh, DEFAULT_RECENTS_LIMIT);
        assert_eq!(
            grown.entries.len(),
            21,
            "the next record grows past the legacy cap"
        );
    }

    /// Acceptance 6: an index written by any prior version (path +
    /// mtime only, this schema forever) loads with zero migration.
    #[test]
    fn prior_version_index_fixture_loads_unchanged() {
        let dir = tempdir().unwrap();
        let alive = existing_png(dir.path(), "shot-a.png");
        let index = dir.path().join("recents.toml");
        let fixture = format!(
            "[[entries]]\npath = \"{}\"\nmtime = 1758000000\n",
            alive.display()
        );
        fs::write(&index, fixture).unwrap();
        let loaded = Recents::load_from(&index);
        assert_eq!(loaded.paths(), vec![alive.as_path()]);
        assert_eq!(loaded.entries[0].mtime, 1_758_000_000);
    }

    #[test]
    fn remove_drops_exactly_the_entry_and_keeps_order() {
        let dir = tempdir().unwrap();
        let a = existing_png(dir.path(), "a.png");
        let b = existing_png(dir.path(), "b.png");
        let c = existing_png(dir.path(), "c.png");

        let mut r = Recents::new();
        r.record(&a, DEFAULT_RECENTS_LIMIT);
        r.record(&b, DEFAULT_RECENTS_LIMIT);
        r.record(&c, DEFAULT_RECENTS_LIMIT);

        assert!(r.remove(&b));
        assert_eq!(r.paths(), vec![c.as_path(), a.as_path()]);
        assert!(!r.remove(&b), "a second remove is a no-op");
    }

    #[test]
    fn remove_from_updates_the_index_file() {
        let dir = tempdir().unwrap();
        let index = dir.path().join("recents.toml");
        let a = existing_png(dir.path(), "a.png");
        let b = existing_png(dir.path(), "b.png");
        record_into(&index, &a, DEFAULT_RECENTS_LIMIT);
        record_into(&index, &b, DEFAULT_RECENTS_LIMIT);

        assert!(remove_from(&index, &b));
        let back = Recents::load_from(&index);
        assert_eq!(back.paths(), vec![a.as_path()], "b is gone, a stays");
    }

    #[test]
    fn load_prunes_dead_paths() {
        let dir = tempdir().unwrap();
        let index = dir.path().join("recents.toml");
        let alive = existing_png(dir.path(), "alive.png");
        let dead = dir.path().join("moved-away.png");

        let mut r = Recents::new();
        r.record(&alive, DEFAULT_RECENTS_LIMIT);
        r.record(&dead, DEFAULT_RECENTS_LIMIT);
        fs::remove_file(&dead).ok();
        r.save_to(&index).unwrap();

        let back = Recents::load_from(&index);
        assert_eq!(back.paths(), vec![alive.as_path()]);
    }

    #[test]
    fn corrupt_index_loads_empty() {
        let dir = tempdir().unwrap();
        let index = dir.path().join("recents.toml");
        fs::write(&index, "not [ valid toml").unwrap();
        assert!(Recents::load_from(&index).paths().is_empty());
    }

    #[test]
    fn record_captures_an_mtime() {
        let dir = tempdir().unwrap();
        let a = existing_png(dir.path(), "a.png");
        let mut r = Recents::new();
        r.record(&a, DEFAULT_RECENTS_LIMIT);
        let mtime = r.entries.first().unwrap().mtime;
        assert!(mtime > 0, "a freshly written file has a real mtime");
    }

    #[test]
    fn record_into_loads_records_and_saves() {
        let dir = tempdir().unwrap();
        let index = dir.path().join("recents.toml");
        let a = existing_png(dir.path(), "a.png");
        let b = existing_png(dir.path(), "b.png");

        record_into(&index, &a, DEFAULT_RECENTS_LIMIT);
        record_into(&index, &b, DEFAULT_RECENTS_LIMIT);

        let back = Recents::load_from(&index);
        assert_eq!(back.paths(), vec![b.as_path(), a.as_path()]);
    }

    #[test]
    fn save_to_creates_parent_directories() {
        let dir = tempdir().unwrap();
        let nested = dir.path().join("flintshot").join("recents.toml");
        Recents::new().save_to(&nested).unwrap();
        assert!(nested.exists());
    }

    #[test]
    fn recents_path_is_beside_config() {
        if let (Some(config), Some(recents)) = (crate::config::config_path(), recents_path()) {
            assert_eq!(
                config.parent(),
                recents.parent(),
                "recents.toml lives beside config.toml"
            );
            assert_eq!(
                recents.file_name().map(|f| f.to_string_lossy()),
                Some("recents.toml".into())
            );
        }
    }

    // --- day_bucket (home-capture-grids Task 001) ---

    /// `now` pinned to 23:30:00 UTC on day `20_000` so hour-based
    /// cases land deterministically around the midnight boundary.
    const NOW: u64 = 20_000 * 86_400 + 23 * 3_600 + 1_800;

    #[test]
    fn day_bucket_same_instant_is_today() {
        assert_eq!(day_bucket(NOW, NOW), DayBucket::Today);
    }

    #[test]
    fn day_bucket_twenty_three_hours_ago_is_today() {
        // Exactly midnight today: the same civil day, one hour and
        // a half before `now`.
        assert_eq!(day_bucket(NOW - 84_600, NOW), DayBucket::Today);
    }

    #[test]
    fn day_bucket_one_second_past_the_boundary_is_yesterday() {
        assert_eq!(day_bucket(NOW - 84_601, NOW), DayBucket::Yesterday);
    }

    #[test]
    fn day_bucket_twenty_five_hours_ago_is_yesterday() {
        assert_eq!(day_bucket(NOW - 90_000, NOW), DayBucket::Yesterday);
    }

    #[test]
    fn day_bucket_a_week_ago_is_earlier() {
        assert_eq!(day_bucket(NOW - 604_800, NOW), DayBucket::Earlier);
    }

    #[test]
    fn day_bucket_clock_skew_lands_earlier_without_panicking() {
        assert_eq!(day_bucket(NOW + 1, NOW), DayBucket::Earlier);
    }
}
