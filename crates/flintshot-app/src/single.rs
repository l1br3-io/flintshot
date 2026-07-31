//! Single-instance lock for the daemon role.
//!
//! Only the daemon acquires this lock (see ADR-0020 — workers such as
//! `flintshot region` never take it, so multiple editors can run at
//! once). On Linux `single-instance` binds an abstract Unix-domain
//! socket; on macOS it `flock`s a file; on Windows it creates a named
//! mutex. A second `flintshot daemon` against the same id fails the
//! `is_single()` check and exits.
//!
//! `--replace` (tell the running daemon to quit via IPC and take over)
//! is wired in Task 006; this module only owns the lock primitive.

use color_eyre::eyre;
use single_instance::SingleInstance;
use std::time::Duration;

/// Lock name for the daemon role. Hashed by `single-instance` into the
/// OS's lock namespace (abstract-socket name on Linux).
const DAEMON_LOCK_NAME: &str = "flintshot-daemon";

/// Acquire the daemon single-instance lock.
///
/// Returns the held [`SingleInstance`] on success — **the caller must
/// keep it alive for the daemon's lifetime** (dropping it releases the
/// lock). Errors if another daemon already holds it.
///
/// # Errors
/// - [`eyre::Report`] if the OS lock primitive could not be created.
/// - [`eyre::Report`] (`"already running"`) if another daemon holds the
///   lock.
pub fn acquire_daemon_lock() -> eyre::Result<SingleInstance> {
    acquire_named(DAEMON_LOCK_NAME)
}

/// Acquire the daemon lock, retrying until `timeout` elapses. Used by
/// `--replace`: after the old daemon releases its IPC socket there's a
/// short window before its single-instance (abstract-socket) lock is
/// freed, so a single immediate acquire can race. Retry absorbs it.
///
/// # Errors
/// Propagates the last acquire error if `timeout` expires.
pub fn acquire_daemon_lock_with_retry(timeout: Duration) -> eyre::Result<SingleInstance> {
    acquire_named_with_retry(DAEMON_LOCK_NAME, timeout)
}

/// Testable seam: [`acquire_daemon_lock_with_retry`] with an explicit
/// lock name (tests pass a unique name per case).
fn acquire_named_with_retry(name: &str, timeout: Duration) -> eyre::Result<SingleInstance> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        match acquire_named(name) {
            Ok(lock) => return Ok(lock),
            Err(e) => {
                if std::time::Instant::now() >= deadline {
                    return Err(e);
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

/// Testable seam: same as [`acquire_daemon_lock`] but with an explicit
/// lock name. Tests pass a unique name per case to avoid cross-test
/// interference under parallel `cargo test`.
fn acquire_named(name: &str) -> eyre::Result<SingleInstance> {
    let instance = SingleInstance::new(name)
        .map_err(|e| eyre::eyre!("creating single-instance lock {name:?}: {e}"))?;
    if instance.is_single() {
        Ok(instance)
    } else {
        Err(eyre::eyre!(
            "another flintshot daemon is already running (single-instance lock {name:?} held)"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Deterministic per-invocation suffix so parallel tests don't
    /// collide on the same abstract-socket name.
    static UNIQUE: AtomicU64 = AtomicU64::new(0);

    fn unique_name(label: &str) -> String {
        let n = UNIQUE.fetch_add(1, Ordering::Relaxed);
        format!("flintshot-test-{label}-{n}")
    }

    #[test]
    fn first_acquire_is_single() {
        let name = unique_name("first");
        let lock = acquire_named(&name).expect("first acquire should succeed");
        assert!(
            lock.is_single(),
            "freshly acquired lock must report is_single"
        );
    }

    #[test]
    fn second_acquire_with_same_name_fails() {
        let name = unique_name("twice");
        let _first = acquire_named(&name).expect("first acquire");
        let second = acquire_named(&name);
        let msg = match second {
            Ok(_) => panic!("a second acquire against the same name must fail while held"),
            Err(report) => format!("{report}"),
        };
        assert!(
            msg.contains("already running"),
            "expected 'already running' in error, got: {msg}"
        );
    }

    #[test]
    fn dropping_releases_the_lock() {
        let name = unique_name("drop");
        {
            let _lock = acquire_named(&name).expect("first acquire");
        }
        // After the first lock dropped, a new acquire must succeed.
        let reagained = acquire_named(&name).expect("re-acquire after drop");
        assert!(reagained.is_single());
    }

    #[test]
    fn distinct_names_coexist() {
        let a = acquire_named(&unique_name("coexist-a")).expect("a");
        let b = acquire_named(&unique_name("coexist-b")).expect("b");
        assert!(
            a.is_single() && b.is_single(),
            "distinct names don't conflict"
        );
    }

    #[test]
    fn retry_succeeds_once_holder_releases() {
        let name = unique_name("retry-release");
        let name_clone = name.clone();
        // Holder taken in a background thread that drops it after 150ms.
        let t = std::thread::spawn(move || {
            let _holder = acquire_named(&name_clone).expect("holder");
            std::thread::sleep(Duration::from_millis(150));
        });
        // Give the holder a moment to grab the lock first.
        std::thread::sleep(Duration::from_millis(50));
        let lock = acquire_named_with_retry(&name, Duration::from_secs(2)).expect("retry succeeds");
        assert!(lock.is_single());
        t.join().expect("holder thread");
    }

    #[test]
    fn retry_times_out_when_held() {
        let name = unique_name("retry-timeout");
        let _holder = acquire_named(&name).expect("holder");
        let result = acquire_named_with_retry(&name, Duration::from_millis(120));
        assert!(
            result.is_err(),
            "retry must time out while the lock is held"
        );
    }
}
