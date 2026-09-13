//! Ambient tokio runtime for window-opening workers.
//!
//! On at-spi desktops, eframe's accessibility bridge (accesskit →
//! zbus 5) requires an ambient tokio reactor on the thread that
//! creates the window. The CLI's window workers (`region` /
//! `fullscreen` → editor or overlay, `open` → editor) own no
//! runtime, so the eframe call panics before any window appears
//! (`TEST_MATRIX` L5, finding 2; full rationale in ADR-0030).
//!
//! [`with_ambient_runtime`] builds a multi-thread runtime, enters it
//! for the duration of the closure, and drops the guard before
//! returning — the narrowest scope that covers the window call
//! without nesting runtimes (e.g. `ipc_client`'s one-shot
//! `block_on` runtimes must run outside it).

use color_eyre::Result;

/// Run `f` with an ambient tokio runtime entered on this thread.
///
/// The runtime's `enter()` guard is held only for `f`; dropping it
/// before return means no reactor context leaks to later code on the
/// thread (which would make nested one-shot runtimes panic).
/// Runtime-construction failure is an `eyre` error — never a panic.
pub(crate) fn with_ambient_runtime<T>(f: impl FnOnce() -> T) -> Result<T> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| color_eyre::eyre::eyre!("building ambient tokio runtime: {e}"))?;
    // Declared after `runtime` so the guard drops first; dropping it
    // un-enters the reactor context before `runtime` itself shuts down.
    let _guard = runtime.enter();
    Ok(f())
}

#[cfg(test)]
mod tests {
    use super::with_ambient_runtime;
    use tokio::runtime::Handle;

    #[test]
    fn try_current_fails_on_a_plain_thread() {
        let no_handle = std::thread::spawn(|| Handle::try_current().is_err())
            .join()
            .expect("probe thread must not panic");
        assert!(no_handle, "a plain thread must have no ambient runtime");
    }

    #[test]
    fn ambient_handle_is_visible_inside_the_helper_but_not_after() {
        assert!(
            Handle::try_current().is_err(),
            "precondition: test thread must not already be inside a runtime"
        );
        let saw_handle =
            with_ambient_runtime(|| Handle::try_current().is_ok()).expect("helper must run");
        assert!(
            saw_handle,
            "Handle::try_current must succeed inside the helper"
        );
        assert!(
            Handle::try_current().is_err(),
            "the ambient runtime must not leak past the helper's return"
        );
    }

    #[test]
    fn helper_propagates_the_closure_return_value() {
        let value = with_ambient_runtime(|| 41 + 1).expect("helper must run");
        assert_eq!(value, 42);
    }
}
