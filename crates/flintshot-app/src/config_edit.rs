//! Open the user's `config.toml` in their editor — the shared
//! implementation behind the settings page's *Edit config file* link
//! (settings-improvement). Extracted from the tray helper (its
//! *Open config* click, ADR-0042) so the settings page and the tray
//! share one path; the tray item is gone but the module is the
//! landing spot for every future "open the config" affordance.

use color_eyre::eyre;

/// Open `config.toml` in `$EDITOR` (fallback `$VISUAL`); print the
/// path if neither is set. The file is materialized from the default
/// template first when missing
/// ([`flintshot_core::ensure_user_config`]), so the file on disk
/// always mirrors what Settings shows before the editor opens it.
///
/// GOTCHA (carried over from the tray behavior): `$EDITOR` from a
/// GUI process has no controlling terminal — terminal editors
/// launched here fail or misbehave; the spawn failure is logged and
/// the path is printed as the fallback cue. An `xdg-open`/portal
/// fallback is a possible follow-up, not this path.
///
/// # Errors
/// - No platform config dir (`dirs::config_dir()` is `None`).
/// - The template could not be written (logged, non-fatal — the
///   editor may still open whatever exists).
/// - The editor failed to spawn.
// The tray helper (feature `tray`) is the only caller until the
// settings page's *Edit config file* link wires this
// (settings-improvement Task 009); without that feature the fn is
// intentionally alive-but-unreferenced.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub(crate) fn open_config_in_editor() -> eyre::Result<()> {
    use std::process::Command;

    let path = flintshot_core::config_path()
        .ok_or_else(|| eyre::eyre!("no platform config_dir available"))?;
    if let Err(e) = flintshot_core::ensure_user_config() {
        // Non-fatal by design: a missing template write (permissions,
        // a read-only home) must not block opening an existing file.
        tracing::warn!(error = %e, "could not materialize the missing config template");
    }
    if let Some(editor) = std::env::var_os("EDITOR").or_else(|| std::env::var_os("VISUAL")) {
        let argv = editor_argv(&editor);
        if argv.is_empty() {
            println!("{}", path.display());
            tracing::info!(path = %path.display(), "$EDITOR/$VISUAL is blank; printed config path");
            return Ok(());
        }
        Command::new(&argv[0])
            .args(&argv[1..])
            .arg(&path)
            .spawn()
            .map_err(|e| eyre::eyre!("launching {editor:?} on {}: {e}", path.display()))?;
        tracing::info!(editor = ?editor, path = %path.display(), "opened config");
    } else {
        println!("{}", path.display());
        tracing::info!(path = %path.display(), "no $EDITOR/$VISUAL; printed config path");
    }
    Ok(())
}

/// Split an `$EDITOR`/`$VISUAL` value into program + leading args.
/// Values like `code --wait` are common (and conventionally unquoted —
/// shells do not expand the variable into words, but every editor
/// launcher tolerates whitespace splitting); treating the whole value
/// as one binary name makes `Command::new` look for a file literally
/// named `code --wait` and the launch silently fail.
fn editor_argv(value: &std::ffi::OsStr) -> Vec<std::ffi::OsString> {
    value
        .to_string_lossy()
        .split_whitespace()
        .map(std::ffi::OsString::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── $EDITOR splitting (moved from the tray helper) ───────────

    #[test]
    fn editor_value_with_args_splits_into_program_and_args() {
        use std::ffi::OsStr;
        let argv = editor_argv(OsStr::new("code --wait"));
        assert_eq!(argv, ["code", "--wait"].map(std::ffi::OsString::from));
    }

    #[test]
    fn plain_editor_value_is_a_single_word() {
        use std::ffi::OsStr;
        assert_eq!(
            editor_argv(OsStr::new("vim")),
            ["vim"].map(std::ffi::OsString::from)
        );
    }

    #[test]
    fn blank_editor_value_is_empty() {
        use std::ffi::OsStr;
        assert!(editor_argv(OsStr::new("   ")).is_empty());
    }
}
