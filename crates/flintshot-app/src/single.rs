//! Single-instance lock for the daemon role.
//!
//! Only the daemon acquires this lock (see ADR-0020 — workers such as
//! `flintshot capture` never take it, so multiple editors can run at
//! once). A second `flintshot daemon` against the same id fails the
//! acquire and exits.
//!
//! Linux implements the lock directly with a std abstract-socket bind
//! instead of the `single-instance` crate: the crate creates its
//! socket **without `FD_CLOEXEC`**, and the daemon spawns worker
//! children (`region --standalone` editors) that inherit the fd. The
//! abstract name then outlives the daemon — held ghost-alive by any
//! open editor — and every subsequent `flintshot daemon` fails with
//! "already running" while no daemon exists (live L5 finding,
//! 2026-08-30; ADR-0033). std marks every fd it creates
//! close-on-exec, so the lock dies with the daemon and only the
//! daemon. macOS/Windows keep the crate (flock file / named mutex),
//! where child inheritance is not a hazard for the lock primitive.
//!
//! `--replace` (tell the running daemon to quit via IPC and take
//! over) is wired in Task 006; this module only owns the lock
//! primitive.

use color_eyre::eyre;
use std::time::Duration;

/// Lock name for the daemon role. An abstract-socket name on Linux
/// (hashed namespace in the crate on macOS/Windows).
const DAEMON_LOCK_NAME: &str = "flintshot-daemon";

/// Lock name for the app-window role. Distinct from the daemon's so
/// a daemon and an app window coexist (the window is a client, not
/// the daemon).
const APP_LOCK_NAME: &str = "flintshot-app";

/// A held single-instance lock. Dropping it releases the OS resource.
pub struct InstanceLock {
    // `_`-prefixed: held for Drop (the lock release), never read.
    #[cfg(target_os = "linux")]
    _inner: LinuxLock,
    #[cfg(not(target_os = "linux"))]
    _inner: single_instance::SingleInstance,
}

/// The Linux lock: a bound abstract unix-domain socket owned by a std
/// [`std::os::unix::net::UnixListener`]. Binding fails with
/// `AddrInUse` while any other live socket holds the name — the same
/// semantics as the crate, minus the fd-inheritance hazard.
#[cfg(target_os = "linux")]
struct LinuxLock {
    /// Held for the lock's lifetime; never `accept`ed. std creates the
    /// socket with `SOCK_CLOEXEC`, so spawned workers cannot inherit
    /// it and ghost-hold the name after the daemon exits.
    _listener: std::os::unix::net::UnixListener,
}

/// Acquire the daemon single-instance lock.
///
/// Returns the held [`InstanceLock`] on success — **the caller must
/// keep it alive for the daemon's lifetime** (dropping it releases the
/// lock). Errors if another daemon already holds it.
///
/// # Errors
/// - [`eyre::Report`] if the OS lock primitive could not be created.
/// - [`eyre::Report`] (`"already running"`) if another daemon holds the
///   lock.
pub fn acquire_daemon_lock() -> eyre::Result<InstanceLock> {
    acquire_named(DAEMON_LOCK_NAME)
}

/// Try to acquire the app-window single-instance lock.
///
/// Returns `Ok(Some(lock))` for the first app window (held for its
/// lifetime), `Ok(None)` when one is already running — the caller's
/// cue to focus it (delegate `Request::OpenApp`) instead of opening a
/// second window.
///
/// # Errors
/// Only real lock-primitive failures; a held lock is `Ok(None)`, not
/// an error.
pub fn acquire_app_lock() -> eyre::Result<Option<InstanceLock>> {
    try_acquire_named(APP_LOCK_NAME)
}

/// Acquire the daemon lock, retrying until `timeout` elapses. Used by
/// `--replace`: after the old daemon releases its IPC socket there's a
/// short window before its single-instance lock is freed, so a single
/// immediate acquire can race. Retry absorbs it.
///
/// # Errors
/// Propagates the last acquire error if `timeout` expires.
pub fn acquire_daemon_lock_with_retry(timeout: Duration) -> eyre::Result<InstanceLock> {
    acquire_named_with_retry(DAEMON_LOCK_NAME, timeout)
}

/// Testable seam: [`acquire_daemon_lock_with_retry`] with an explicit
/// lock name (tests pass a unique name per case).
fn acquire_named_with_retry(name: &str, timeout: Duration) -> eyre::Result<InstanceLock> {
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
fn acquire_named(name: &str) -> eyre::Result<InstanceLock> {
    match try_acquire_named(name)? {
        Some(lock) => Ok(lock),
        None => Err(eyre::eyre!(
            "another flintshot daemon is already running (single-instance lock {name:?} held)"
        )),
    }
}

/// Lock-acquire core: `Ok(None)` means the name is held (the caller
/// decides whether that is an error — the daemon treats it as one,
/// the app window treats it as "focus the existing one").
fn try_acquire_named(name: &str) -> eyre::Result<Option<InstanceLock>> {
    #[cfg(target_os = "linux")]
    {
        use std::os::linux::net::SocketAddrExt;
        use std::os::unix::net::{SocketAddr, UnixListener};
        let addr = SocketAddr::from_abstract_name(name.as_bytes())
            .map_err(|e| eyre::eyre!("building abstract lock name {name:?}: {e}"))?;
        match UnixListener::bind_addr(&addr) {
            Ok(listener) => Ok(Some(InstanceLock {
                _inner: LinuxLock {
                    _listener: listener,
                },
            })),
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => Ok(None),
            Err(e) => Err(eyre::eyre!("binding single-instance lock {name:?}: {e}")),
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let instance = single_instance::SingleInstance::new(name)
            .map_err(|e| eyre::eyre!("creating single-instance lock {name:?}: {e}"))?;
        if instance.is_single() {
            Ok(Some(InstanceLock { _inner: instance }))
        } else {
            Ok(None)
        }
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
        let lock = try_acquire_named(&name).expect("first acquire should succeed");
        assert!(lock.is_some(), "an unheld name must acquire");
    }

    #[test]
    fn held_name_reports_none_not_error() {
        let name = unique_name("held");
        let _first = try_acquire_named(&name).expect("first acquire");
        let second =
            try_acquire_named(&name).expect("a held name is a state, not a transport error");
        assert!(second.is_none(), "the second acquire must report None");
    }

    #[test]
    fn daemon_style_acquire_errors_on_held_name() {
        let name = unique_name("daemon-err");
        let _first = acquire_named(&name).expect("first acquire");
        // `InstanceLock` has no `Debug`, so `expect_err` is out; match.
        let err = match acquire_named(&name) {
            Ok(_) => panic!("held name must error"),
            Err(e) => format!("{e:#}"),
        };
        assert!(err.contains("already running"), "message: {err}");
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
        let _reagained = acquire_named(&name).expect("re-acquire after drop");
    }

    #[test]
    fn distinct_names_coexist() {
        let _a = acquire_named(&unique_name("coexist-a")).expect("a");
        let _b = acquire_named(&unique_name("coexist-b")).expect("b");
    }

    /// The regression this module exists for (ADR-0033): a child
    /// spawned while the lock is held must NOT keep the lock alive
    /// after the holder drops it. The `single-instance` crate's
    /// non-CLOEXEC socket made this fail — a leftover editor
    /// ghost-held the daemon lock and no daemon could restart.
    ///
    /// The reacquire retries briefly: a concurrent `Command::spawn`
    /// fork (anywhere in the test process) duplicates the fd table
    /// before exec closes CLOEXEC fds, transiently holding the name
    /// for microseconds. The bug this guards against holds it for
    /// the child's whole lifetime (`sleep 2`), so a bounded 1 s
    /// retry separates the two cleanly.
    #[cfg(target_os = "linux")]
    #[test]
    fn lock_not_inherited_by_spawned_children() {
        let name = unique_name("inherit");
        let mut child = {
            let _lock = acquire_named(&name).expect("holder acquires");
            std::process::Command::new("sleep")
                .arg("2")
                .spawn()
                .expect("spawning sleep child")
        };
        // Lock dropped above (end of block) while the child still runs.
        let deadline = std::time::Instant::now() + Duration::from_secs(1);
        let reacquired = loop {
            match acquire_named(&name) {
                Ok(lock) => break Some(lock),
                Err(e) if std::time::Instant::now() >= deadline => {
                    let holders = holders_of_abstract_name(&name);
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!(
                        "a live spawned child must not ghost-hold the single-instance lock: \
                         {e} (holders: {holders:?})"
                    );
                }
                Err(_) => std::thread::sleep(Duration::from_millis(10)),
            }
        };
        let _ = child.kill();
        let _ = child.wait();
        drop(reacquired);
    }

    /// Diagnostic for the failure path: raw `/proc/net/unix` lines
    /// still referencing `name` (empty = the name was already gone —
    /// a transient fork-dup window, not a real holder). Test-only.
    #[cfg(target_os = "linux")]
    fn holders_of_abstract_name(name: &str) -> Vec<String> {
        std::fs::read_to_string("/proc/net/unix")
            .unwrap_or_default()
            .lines()
            .filter(|l| l.contains(name))
            .map(str::to_string)
            .collect()
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
        let _lock = acquire_named_with_retry(&name, Duration::from_secs(2))
            .expect("retry succeeds while the holder releases");
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
