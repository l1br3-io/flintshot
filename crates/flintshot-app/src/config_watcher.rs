//! Config hot-reload: a `notify` watcher on the config file's parent
//! directory, polled from the daemon's main loop. On a config change the
//! daemon re-reads it and re-registers hotkeys when `[hotkeys]` differs
//! (Task 010). Events are coalesced by draining the channel once per
//! loop iteration, so a burst of editor-save events yields a single
//! reload.

use std::path::{Path, PathBuf};

use color_eyre::eyre;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};

/// Watches the config file for changes. The watcher itself is held alive
/// by this struct; the daemon polls [`ConfigWatcher::poll_changed`] each
/// loop iteration.
pub(crate) struct ConfigWatcher {
    // Field unused by name — keeping the watcher alive is the point; it
    // must not be dropped (that stops watching).
    _watcher: RecommendedWatcher,
    events: std::sync::mpsc::Receiver<()>,
}

impl ConfigWatcher {
    /// Start watching `config_path`'s parent directory (the directory
    /// watch catches editors that save via a temp-file rename, which a
    /// direct file watch would miss). Creates the directory if absent.
    ///
    /// # Errors
    /// - If the config has no parent dir, the directory can't be
    ///   created, or `notify` can't install the watch.
    pub(crate) fn start(config_path: &Path) -> eyre::Result<Self> {
        let dir = config_path
            .parent()
            .ok_or_else(|| eyre::eyre!("config path has no parent directory"))?;
        std::fs::create_dir_all(dir)
            .map_err(|e| eyre::eyre!("creating config dir {}: {e}", dir.display()))?;

        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let target: PathBuf = config_path.to_path_buf();
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(ev) = res {
                // Only react to events touching the config file itself.
                if ev.paths.iter().any(|p| p == &target) {
                    let _ = tx.send(());
                }
            }
        })
        .map_err(|e| eyre::eyre!("creating config watcher: {e}"))?;
        watcher
            .watch(dir, RecursiveMode::NonRecursive)
            .map_err(|e| eyre::eyre!("watching config dir {}: {e}", dir.display()))?;

        tracing::info!(path = %config_path.display(), "watching config for hot-reload");
        Ok(Self {
            _watcher: watcher,
            events: rx,
        })
    }

    /// Coalesce and report whether any config-change events arrived
    /// since the last poll. Drains the channel so a burst collapses to
    /// one reload.
    #[must_use]
    pub(crate) fn poll_changed(&self) -> bool {
        let mut changed = false;
        while self.events.try_recv().is_ok() {
            changed = true;
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    /// `notify` against a real filesystem: a write to the watched config
    /// surfaces via `poll_changed`. `#[ignore]` because it exercises the
    /// real kernel watcher (timing-sensitive). Run with
    /// `cargo test -p flintshot-app -- --ignored config_watcher`.
    #[test]
    #[ignore = "exercises the real kernel file watcher; run explicitly"]
    fn watcher_fires_on_config_write() {
        let mut tmp = NamedTempFile::new().unwrap();
        writeln!(tmp, "[hotkeys]\nregion = \"Ctrl+Shift+PrintScreen\"\n").unwrap();
        tmp.flush().unwrap();

        let watcher = ConfigWatcher::start(tmp.path()).unwrap();

        // Drain any startup noise, then mutate.
        let _ = watcher.poll_changed();
        std::thread::sleep(std::time::Duration::from_millis(100));

        let mut tmp2 = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp2, "[hotkeys]\nregion = \"Ctrl+Alt+A\"\n").unwrap();
        std::fs::copy(tmp2.path(), tmp.path()).unwrap();

        // Give the kernel watcher a moment to deliver.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        let mut saw_change = false;
        while std::time::Instant::now() < deadline {
            if watcher.poll_changed() {
                saw_change = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(saw_change, "config watcher did not fire on write");
    }
}
