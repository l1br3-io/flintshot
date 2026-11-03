//! The session capture store — unsaved captures that survive
//! editor navigation until the app-window process exits.
//!
//! Every capture that lands in the app-window editor is tracked
//! here (raw image plus committed annotations) until the user
//! saves it, discards it from Home, or the process exits. The
//! store mirrors each entry to a temp-dir pair —
//! `<dir>/<id>.png` (the source image, written once on arrival,
//! off-thread) and `<dir>/<id>.json` (created-at plus committed
//! annotations, rewritten synchronously on every commit) — so
//! annotations are crash-safe by construction.
//!
//! Persistence is **advisory**, like the recents index: any IO
//! failure logs a warning and degrades to memory-only for that
//! entry; nothing here is ever a user-facing error. And the store
//! never *writes* across a process boundary: startup parks any
//! leftover dir from a crashed run into a quarantine sibling
//! ([`parked_dir`]) instead of deleting it, graceful exit clears
//! the live dir, and adoption of a parked run is read-only — the
//! park is only ever renamed into or swept, never written to.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::editor::Annotation;

/// Stable identity of one session capture within the process
/// lifetime. Ids are monotonic: a larger id arrived later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionId(u64);

impl SessionId {
    /// Wrap a raw id. Production ids come from
    /// [`SessionStore::insert`]; this is for tests, logging, and
    /// serialization seams.
    #[must_use]
    pub const fn from_raw(id: u64) -> Self {
        Self(id)
    }

    /// The raw id (logging / serialization).
    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }
}

/// One resumable capture: the source image (never mutated by
/// annotations) plus committed annotations, in draw order.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct SessionCapture {
    /// The capture's identity in the store.
    pub id: SessionId,
    /// Unix-seconds arrival time (card captions).
    pub created_at: u64,
    /// The source screenshot.
    pub image: image::RgbaImage,
    /// Committed annotations, in draw order.
    pub anns: Vec<Annotation>,
}

/// The listable face of one session capture — identity and
/// metadata without the image payload, so Home can render cards
/// every frame without holding (or decoding) full captures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct SessionSummary {
    /// The capture's identity in the store.
    pub id: SessionId,
    /// Unix-seconds arrival time.
    pub created_at: u64,
    /// Source image width in px.
    pub width: u32,
    /// Source image height in px.
    pub height: u32,
}

/// The `<id>.json` sidecar: everything about an entry except the
/// image. Synchronously rewritten on every committed change.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct SessionSidecar {
    created_at: u64,
    #[serde(default)]
    anns: Vec<Annotation>,
}

/// In-memory index entry: an insertion-order id plus the metadata
/// Home cards need. The image itself lives only on disk (written
/// off-thread on arrival) so a session full of captures costs a
/// few hundred bytes of RAM, not N full images.
#[derive(Debug, Clone)]
struct EntryMeta {
    created_at: u64,
    width: u32,
    height: u32,
}

#[derive(Debug, Default)]
struct Inner {
    next_id: u64,
    entries: Vec<(SessionId, EntryMeta)>,
}

/// The session capture store: a thread-safe in-memory index
/// mirrored to a temp dir. Owned once per app-window process.
///
/// All IO is advisory — failures warn and degrade, they never
/// propagate. A single app window is guaranteed (see
/// `flintshot-app`'s single-instance check), so a fixed dir is
/// safe against concurrent writers.
#[derive(Debug, Clone)]
pub struct SessionStore {
    dir: PathBuf,
    inner: Arc<Mutex<Inner>>,
}

impl SessionStore {
    /// Open the store at the platform location
    /// (`$XDG_RUNTIME_DIR/flintshot/session/`, falling back to
    /// `std::env::temp_dir()/flintshot/session/`), creating the
    /// directory if needed.
    #[must_use]
    pub fn new() -> Self {
        Self::in_dir(session_dir())
    }

    /// Testable seam: open the store in an explicit directory,
    /// creating it if needed.
    #[must_use]
    pub fn in_dir(dir: PathBuf) -> Self {
        if let Err(e) = std::fs::create_dir_all(&dir) {
            tracing::warn!(dir = %dir.display(), error = %e, "creating the session dir");
        }
        Self {
            dir,
            inner: Arc::new(Mutex::new(Inner::default())),
        }
    }

    /// Track a newly landed capture. The in-memory entry is
    /// listable immediately; the PNG write runs on a short-lived
    /// thread so the UI thread never blocks on image encode (the
    /// tiny sidecar is written synchronously).
    pub fn insert(&self, image: image::RgbaImage, now: u64) -> SessionId {
        let (width, height) = (image.width(), image.height());
        let mut guard = lock(&self.inner);
        let next = guard.next_id;
        guard.next_id += 1;
        let id = SessionId(next);
        guard.entries.push((
            id,
            EntryMeta {
                created_at: now,
                width,
                height,
            },
        ));
        drop(guard);

        let sidecar = SessionSidecar {
            created_at: now,
            anns: Vec::new(),
        };
        if let Err(e) = write_sidecar(&self.dir, id, &sidecar) {
            tracing::warn!(error = %e, "writing the session sidecar; degrading to memory-only");
        }

        let inner = Arc::clone(&self.inner);
        let dir = self.dir.clone();
        std::thread::spawn(move || write_png(&inner, &dir, id, &image));
        id
    }

    /// Rewrite the committed-annotation sidecar for `id`. Small
    /// payload, synchronous by design: crash-safe annotations for
    /// free. Unknown ids are a no-op (the entry was removed).
    pub fn update_anns(&self, id: SessionId, anns: &[Annotation]) {
        let guard = lock(&self.inner);
        let Some((_, meta)) = guard.entries.iter().find(|(eid, _)| *eid == id) else {
            return;
        };
        let sidecar = SessionSidecar {
            created_at: meta.created_at,
            anns: anns.to_vec(),
        };
        if let Err(e) = write_sidecar(&self.dir, id, &sidecar) {
            tracing::warn!(error = %e, "updating the session sidecar; degrading to memory-only");
        }
    }

    /// Drop one capture (save or discard): the memory entry plus
    /// both files. Missing files are not an error (advisory).
    pub fn remove(&self, id: SessionId) {
        let mut guard = lock(&self.inner);
        guard.entries.retain(|(eid, _)| *eid != id);
        drop(guard);
        for path in [png_path(&self.dir, id), json_path(&self.dir, id)] {
            if let Err(e) = std::fs::remove_file(&path) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    tracing::warn!(path = %path.display(), error = %e, "removing a session file");
                }
            }
        }
    }

    /// The listable entries, most recent first (ids are
    /// monotonic, so id order is arrival order).
    #[must_use]
    pub fn list(&self) -> Vec<SessionSummary> {
        let guard = lock(&self.inner);
        let mut summaries: Vec<SessionSummary> = guard
            .entries
            .iter()
            .map(|(id, meta)| SessionSummary {
                id: *id,
                created_at: meta.created_at,
                width: meta.width,
                height: meta.height,
            })
            .collect();
        drop(guard);
        summaries.sort_by_key(|s| std::cmp::Reverse(s.id));
        summaries
    }

    /// The on-disk PNG path for `id` — the thumbnail source for
    /// Home's session cards. Advisory: the background write may
    /// still be in flight or may have failed (memory-only entry).
    #[must_use]
    pub fn png_path(&self, id: SessionId) -> PathBuf {
        png_path(&self.dir, id)
    }

    /// Rebuild one capture for resume: decode the PNG, parse the
    /// sidecar. A corrupt or missing sidecar degrades to empty
    /// annotations (advisory); a missing or unreadable PNG means
    /// the capture cannot be rebuilt — `None`. Unknown ids are
    /// `None`.
    #[must_use]
    pub fn load(&self, id: SessionId) -> Option<SessionCapture> {
        let created_at = {
            let guard = lock(&self.inner);
            guard
                .entries
                .iter()
                .find(|(eid, _)| *eid == id)?
                .1
                .created_at
        };
        let image = image::open(png_path(&self.dir, id)).ok()?.to_rgba8();
        let anns = read_sidecar(&self.dir, id).map_or_else(Vec::new, |s| s.anns);
        Some(SessionCapture {
            id,
            created_at,
            image,
            anns,
        })
    }

    /// Empty the store and remove the whole session dir — the
    /// graceful-exit path. Best effort: IO failures warn.
    pub fn clear(&self) {
        let mut guard = lock(&self.inner);
        guard.entries.clear();
        drop(guard);
        if let Err(e) = std::fs::remove_dir_all(&self.dir) {
            if e.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!(dir = %self.dir.display(), error = %e, "clearing the session dir");
            }
        }
    }

    /// Delete a leftover session dir from a crashed run. Absence
    /// is fine (the usual case); failures warn, never error.
    pub fn sweep_stale(dir: &Path) {
        if let Err(e) = std::fs::remove_dir_all(dir) {
            if e.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!(dir = %dir.display(), error = %e, "sweeping a stale session dir");
            }
        }
    }

    /// Park a previous run's leftover session dir instead of
    /// sweeping it, so Home can offer recovery. The dir is atomically
    /// renamed to its fixed quarantine sibling ([`parked_dir`]); a new
    /// park replaces any older one — newest-wins, the older park is
    /// swept and logged, keeping disk bounded to one parked run. No
    /// leftover is a no-op. Advisory: on failure this warns and
    /// degrades to the old [`SessionStore::sweep_stale`] behavior.
    pub fn park_previous_run(dir: &Path) {
        if !dir.is_dir() {
            return;
        }
        let park = parked_dir(dir);
        if park.exists() {
            // Newest-wins (the plan's pinned answer to a second crash
            // parking over an unhandled one): the park holds unsaved
            // screenshots, not archives, and bounded disk beats
            // merge complexity.
            tracing::info!(park = %park.display(), "sweeping an older never-handled park");
            Self::sweep_stale(&park);
        }
        if let Err(e) = std::fs::rename(dir, &park) {
            tracing::warn!(
                dir = %dir.display(),
                park = %park.display(),
                error = %e,
                "parking the previous run's session dir; degrading to the old sweep"
            );
            Self::sweep_stale(dir);
        }
    }
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}

/// The platform session dir: `$XDG_RUNTIME_DIR/flintshot/session/`
/// (OS-cleaned tmpfs) with a `std::env::temp_dir()` fallback for
/// non-systemd setups.
#[must_use]
pub fn session_dir() -> PathBuf {
    session_dir_from(std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from))
}

/// Pure seam for [`session_dir`]: the runtime dir when the
/// platform provides one, the temp dir otherwise.
#[must_use]
fn session_dir_from(xdg: Option<PathBuf>) -> PathBuf {
    xdg.unwrap_or_else(std::env::temp_dir)
        .join("flintshot")
        .join("session")
}

/// The fixed quarantine sibling a leftover session dir is parked
/// into (`<…>/session-parked`): [`SessionStore::park_previous_run`]
/// renames there, [`SessionStore::load_parked`] reads back from
/// there. A path with no final component gets a child name — still
/// fixed, still derived.
#[must_use]
pub fn parked_dir(session_dir: &Path) -> PathBuf {
    let Some(name) = session_dir.file_name() else {
        return session_dir.join("parked");
    };
    let mut parked = std::ffi::OsString::from(name);
    parked.push("-parked");
    session_dir.with_file_name(parked)
}

/// A lock that survives poisoning — store state is advisory, so a
/// panicked writer's data is better than a dead store.
fn lock(inner: &Mutex<Inner>) -> std::sync::MutexGuard<'_, Inner> {
    inner
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn png_path(dir: &Path, SessionId(id): SessionId) -> PathBuf {
    dir.join(format!("{id}.png"))
}

fn json_path(dir: &Path, SessionId(id): SessionId) -> PathBuf {
    dir.join(format!("{id}.json"))
}

/// Encode and write the arrival PNG. Runs on a short-lived thread
/// (captures are rare); the entry's existence is re-checked under
/// the store lock right before the write so a concurrent `remove`
/// cannot be resurrected by an in-flight write.
fn write_png(inner: &Mutex<Inner>, dir: &Path, id: SessionId, image: &image::RgbaImage) {
    let mut bytes = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut bytes);
    if let Err(e) = image.write_to(&mut cursor, image::ImageFormat::Png) {
        tracing::warn!(error = %e, "encoding a session PNG; degrading to memory-only");
        return;
    }
    let guard = lock(inner);
    if !guard.entries.iter().any(|(eid, _)| *eid == id) {
        return;
    }
    if let Err(e) = std::fs::write(png_path(dir, id), &bytes) {
        tracing::warn!(error = %e, "writing a session PNG; degrading to memory-only");
    }
}

fn write_sidecar(dir: &Path, id: SessionId, sidecar: &SessionSidecar) -> std::io::Result<()> {
    let body = serde_json::to_string(sidecar).map_err(std::io::Error::other)?;
    std::fs::write(json_path(dir, id), body)
}

fn read_sidecar(dir: &Path, id: SessionId) -> Option<SessionSidecar> {
    let body = std::fs::read_to_string(json_path(dir, id)).ok()?;
    match serde_json::from_str(&body) {
        Ok(sidecar) => Some(sidecar),
        Err(e) => {
            tracing::warn!(error = %e, "parsing a session sidecar; degrading to empty annotations");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{Duration, Instant};
    use tempfile::tempdir;

    fn tiny_image(w: u32, h: u32) -> image::RgbaImage {
        image::RgbaImage::from_pixel(w, h, image::Rgba([200, 30, 30, 255]))
    }

    fn test_line_ann() -> Annotation {
        use crate::geom::{Color, Point, Stroke};
        Annotation::Line {
            a: Point::new(1.0, 2.0),
            b: Point::new(3.0, 4.0),
            stroke: Stroke::new(Color::rgb(255, 255, 255), 2.0),
        }
    }

    /// Poll until `f` pans out or the deadline dies — the PNG
    /// write lands on a background thread "shortly after" insert.
    fn soon<T>(mut f: impl FnMut() -> Option<T>) -> Option<T> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(value) = f() {
                return Some(value);
            }
            if Instant::now() >= deadline {
                return None;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn insert_returns_fresh_ids_and_lists_immediately() {
        let dir = tempdir().unwrap();
        let store = SessionStore::in_dir(dir.path().to_path_buf());

        let a = store.insert(tiny_image(3, 5), 100);
        let b = store.insert(tiny_image(4, 6), 200);

        assert_ne!(a, b, "every insert mints a fresh id");
        let list = store.list();
        assert_eq!(list.len(), 2, "entries are listable before any IO lands");
        assert_eq!(list[0].id, b, "most recent first");
        assert_eq!(list[1].id, a);
        assert_eq!((list[0].width, list[0].height), (4, 6));
        assert_eq!(list[0].created_at, 200);
    }

    #[test]
    fn png_lands_on_disk_shortly_after_insert() {
        let dir = tempdir().unwrap();
        let store = SessionStore::in_dir(dir.path().to_path_buf());
        let id = store.insert(tiny_image(8, 8), 1);

        let path = png_path(dir.path(), id);
        let landed = soon(|| path.exists().then_some(()));
        assert!(landed.is_some(), "the background PNG write completes");
    }

    #[test]
    fn update_anns_rewrites_the_sidecar_roundtrip() {
        let dir = tempdir().unwrap();
        let store = SessionStore::in_dir(dir.path().to_path_buf());
        let id = store.insert(tiny_image(8, 8), 42);
        let anns = vec![test_line_ann()];

        store.update_anns(id, &anns);
        assert!(
            json_path(dir.path(), id).exists(),
            "the sidecar is rewritten synchronously"
        );

        let loaded = soon(|| store.load(id));
        let loaded = loaded.expect("the capture round-trips once the PNG lands");
        assert_eq!(loaded.created_at, 42);
        assert_eq!(
            loaded.anns, anns,
            "committed annotations survive the round-trip"
        );
        assert_eq!(loaded.image.dimensions(), (8, 8));
    }

    #[test]
    fn remove_drops_the_entry_and_both_files() {
        let dir = tempdir().unwrap();
        let store = SessionStore::in_dir(dir.path().to_path_buf());
        let id = store.insert(tiny_image(8, 8), 1);
        store.update_anns(id, &[test_line_ann()]);
        soon(|| png_path(dir.path(), id).exists().then_some(()));

        store.remove(id);

        assert!(store.list().is_empty(), "the memory entry is gone");
        assert!(!png_path(dir.path(), id).exists());
        assert!(!json_path(dir.path(), id).exists());
        assert!(store.load(id).is_none(), "unknown ids load nothing");
    }

    #[test]
    fn remove_with_missing_files_is_not_an_error() {
        let dir = tempdir().unwrap();
        let store = SessionStore::in_dir(dir.path().to_path_buf());
        let id = store.insert(tiny_image(8, 8), 1);
        soon(|| png_path(dir.path(), id).exists().then_some(()));
        store.remove(id);
        store.remove(id);
        store.remove(SessionId(999));
    }

    #[test]
    fn list_orders_most_recent_first() {
        let dir = tempdir().unwrap();
        let store = SessionStore::in_dir(dir.path().to_path_buf());
        let ids: Vec<SessionId> = (0..4)
            .map(|i| store.insert(tiny_image(2, 2), 10 + i))
            .collect();

        let listed: Vec<SessionId> = store.list().into_iter().map(|s| s.id).collect();
        let mut expected = ids.clone();
        expected.reverse();
        assert_eq!(listed, expected, "insertion order reversed, newest first");
    }

    #[test]
    fn corrupt_or_missing_sidecar_degrades_to_empty_anns() {
        let dir = tempdir().unwrap();
        let store = SessionStore::in_dir(dir.path().to_path_buf());
        let id = store.insert(tiny_image(8, 8), 7);
        store.update_anns(id, &[test_line_ann()]);
        soon(|| png_path(dir.path(), id).exists().then_some(()));

        fs::write(json_path(dir.path(), id), "{{{ not json").unwrap();
        let loaded = store
            .load(id)
            .expect("the entry survives a corrupt sidecar");
        assert!(loaded.anns.is_empty(), "annotations degrade to empty");

        fs::remove_file(json_path(dir.path(), id)).unwrap();
        let loaded = store
            .load(id)
            .expect("the entry survives a missing sidecar");
        assert!(loaded.anns.is_empty());
    }

    #[test]
    fn missing_png_loads_none() {
        let dir = tempdir().unwrap();
        let store = SessionStore::in_dir(dir.path().to_path_buf());
        let id = store.insert(tiny_image(8, 8), 1);
        soon(|| png_path(dir.path(), id).exists().then_some(()));
        fs::remove_file(png_path(dir.path(), id)).unwrap();

        assert!(
            store.load(id).is_none(),
            "no PNG means the capture cannot be rebuilt"
        );
    }

    #[test]
    fn clear_empties_entries_and_removes_the_dir() {
        let dir = tempdir().unwrap();
        let session = dir.path().join("session");
        let store = SessionStore::in_dir(session.clone());
        store.insert(tiny_image(8, 8), 1);
        store.insert(tiny_image(8, 8), 2);
        store.clear();

        assert!(store.list().is_empty());
        let leftovers = fs::read_dir(&session).map_or(0, |rd| rd.flatten().count());
        assert_eq!(leftovers, 0, "graceful exit leaves nothing behind");
    }

    #[test]
    fn sweep_stale_clears_leftovers_and_tolerates_absence() {
        let dir = tempdir().unwrap();
        let stale = dir.path().join("session");
        fs::create_dir_all(&stale).unwrap();
        fs::write(stale.join("7.png"), b"junk").unwrap();
        fs::write(stale.join("7.json"), b"junk").unwrap();

        SessionStore::sweep_stale(&stale);
        assert!(!stale.exists(), "a crashed run's dir is deleted");

        SessionStore::sweep_stale(&stale);
        SessionStore::sweep_stale(&dir.path().join("never-existed"));
    }

    #[test]
    fn parked_dir_is_a_fixed_sibling_of_the_session_dir() {
        assert_eq!(
            parked_dir(Path::new("/runtime/flintshot/session")),
            PathBuf::from("/runtime/flintshot/session-parked")
        );
        // A path without a final component falls back to a child
        // name — still fixed, still derived.
        assert_eq!(parked_dir(Path::new("/")), PathBuf::from("/parked"));
    }

    #[test]
    fn park_previous_run_renames_a_leftover_dir() {
        let dir = tempdir().unwrap();
        let session = dir.path().join("session");
        fs::create_dir_all(&session).unwrap();
        fs::write(session.join("1.png"), b"junk").unwrap();
        fs::write(session.join("1.json"), b"junk").unwrap();

        SessionStore::park_previous_run(&session);

        assert!(!session.exists(), "the leftover is renamed away");
        let park = parked_dir(&session);
        assert!(park.join("1.png").exists(), "the pair survived the move");
        assert!(park.join("1.json").exists());
    }

    #[test]
    fn park_previous_run_without_a_leftover_is_a_no_op() {
        let dir = tempdir().unwrap();
        let session = dir.path().join("session");

        SessionStore::park_previous_run(&session);
        SessionStore::park_previous_run(&session);

        assert!(
            !parked_dir(&session).exists(),
            "no leftover means no park is created"
        );
    }

    #[test]
    fn park_previous_run_replaces_an_older_park_newest_wins() {
        let dir = tempdir().unwrap();
        let session = dir.path().join("session");
        let park = parked_dir(&session);

        // An older park the user never handled, plus a fresh crash.
        fs::create_dir_all(&park).unwrap();
        fs::write(park.join("1.png"), b"older run").unwrap();
        fs::create_dir_all(&session).unwrap();
        fs::write(session.join("2.png"), b"newer run").unwrap();

        SessionStore::park_previous_run(&session);

        assert!(!session.exists(), "the newer leftover moved into the park");
        assert!(
            park.join("2.png").exists(),
            "the newer run's captures are the park now"
        );
        assert!(
            !park.join("1.png").exists(),
            "the older park was swept — disk stays bounded to one run"
        );
    }

    #[test]
    fn park_io_failure_warns_and_degrades_to_the_sweep() {
        let dir = tempdir().unwrap();
        let session = dir.path().join("session");
        let park = parked_dir(&session);

        // A regular file squatting on the park path: the old park
        // cannot be removed and the rename cannot land onto a file
        // — both fail on every platform, root included, so the
        // degrade path runs deterministically.
        fs::create_dir_all(&session).unwrap();
        fs::write(session.join("3.png"), b"junk").unwrap();
        fs::write(&park, b"a file, not a dir").unwrap();

        SessionStore::park_previous_run(&session);

        assert!(
            !session.exists(),
            "the rename failed, so the old sweep behavior took over"
        );
        assert!(park.is_file(), "the undeletable squatter is untouched");
    }

    #[test]
    fn dir_resolution_honors_xdg_and_falls_back_to_temp() {
        assert_eq!(
            session_dir_from(Some(PathBuf::from("/xdg-runtime"))),
            PathBuf::from("/xdg-runtime/flintshot/session")
        );
        assert_eq!(
            session_dir_from(None),
            std::env::temp_dir().join("flintshot").join("session")
        );
    }
}
