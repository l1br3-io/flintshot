//! XDG Screenshot Portal backend — `org.freedesktop.portal.Screenshot`
//! over D-Bus (Linux only).
//!
//! Spike B measured this path live on a Wayland/COSMIC session
//! (`docs/specs/native-capture-backends/tasks/002-spike-portal.md`):
//! prompt-free, ~75–120 ms end-to-end — it earns the `auto` slot on
//! Wayland (ahead of `xcap`, which drives a `ScreenCast` session
//! there).
//! Protocol quirks handled here:
//!
//! - **One Response per request**: the method call returns a request
//!   handle immediately; the actual result arrives later as a
//!   `Response` signal on that handle's object path (`0` = success +
//!   `uri`, `1` = cancelled). The pump thread takes exactly one
//!   message and exits — a second would park forever.
//! - **Client lifetime**: the request handle is bound to our unique
//!   bus name. If the client leaves the bus after the call, the
//!   frontend cancels the request and the Response never arrives —
//!   the connection must stay in scope until the signal is consumed
//!   (it does: it lives in [`grab_whole_desktop`]'s frame).
//! - **Malformed URIs**: xdg-desktop-portal-cosmic writes
//!   `format!("file:///{}", path)` with an absolute path, producing
//!   `file:////tmp/...` (four slashes). [`parse_portal_uri`] strips
//!   the scheme and any extra leading slashes. Percent-encoded URIs
//!   are not decoded — no portal impl we know of encodes; revisit if
//!   one does (`TEST_MATRIX`).
//! - **Clipboard delivery**: confirming the interactive UI with
//!   its Copy button makes the portal write the grab to the system
//!   clipboard itself and answer `clipboard:///` — no file exists.
//!   [`parse_portal_uri`] reports that as a marker error
//!   ([`PORTAL_GRAB_ON_CLIPBOARD`]); the app layer reads the
//!   pixels back from the clipboard.
//! - **Coverage**: the returned image composites **all outputs** at
//!   logical coordinates (`HiDPI` multi-monitor frames are Lanczos3-
//!   downscaled to logical size by the portal — unavoidable).
//!   [`crop_composite_to_monitor`] derives the effective scale from
//!   the composite size vs the reported monitor bounding box. It must
//!   **never** use `scale_factor()`: xcap's geometry on COSMIC is
//!   inflated ~3× (7680×4320 reported for a physical 2560×1440
//!   panel), so only the ratio between the two reported spaces is
//!   trustworthy. A single output keeps its physical pixels; the
//!   crop is a passthrough.
//! - **Cursor**: the protocol exposes no cursor option
//!   (`handle_token`, `modal`, `interactive` only) — grabs via this
//!   backend are always cursor-less. [`CaptureOptions::include_cursor`]
//!   cannot be honored here; a permanent, documented gap.
//! - **Interactive mode** ([`Capturer::capture_selection`]): the
//!   compositor's own selection UI does the region picking. Spike C
//!   (`docs/specs/native-capture-ux/tasks/001-spike-interactive.md`)
//!   verified on COSMIC that the Response returns **exactly the
//!   selection** (passthrough — no composite crop), that files land
//!   in `~/Pictures/Screenshots` (read-then-delete), and that
//!   Escape/dismissal is unreliable on the installed portal build:
//!   code 1 (proper `"cancelled"`), spurious code 0 captures, and a
//!   no-Response hang were all observed. Hence the dedicated
//!   [`INTERACTIVE_RESPONSE_TIMEOUT`] and the `"timed out"` substring
//!   in that error for quiet-exit callers.
//!
//! Budgets: every method call is bounded by the connection's
//! `method_timeout` and the Response wait by [`RESPONSE_TIMEOUT`]
//! (non-interactive) or [`INTERACTIVE_RESPONSE_TIMEOUT`]
//! (interactive) — a wedged portal errors out instead of hanging the
//! worker.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use image::RgbaImage;
use zbus::blocking::Connection;
use zbus::zvariant::{ObjectPath, OwnedValue, Value};
use zbus::MatchRule;

use super::xcap_backend::enumerate_monitors;
use super::{CaptureOptions, Capturer, MonitorInfo};
use crate::error::{FlintshotError, Result};

/// Portal frontend bus name.
const PORTAL_NAME: &str = "org.freedesktop.portal.Desktop";
/// Object path of the portal's desktop interfaces.
const PORTAL_DESKTOP_PATH: &str = "/org/freedesktop/portal/desktop";
/// The interface this backend drives.
const SCREENSHOT_IFACE: &str = "org.freedesktop.portal.Screenshot";
/// Signal interface every request handle exports.
const REQUEST_IFACE: &str = "org.freedesktop.portal.Request";

/// Ceiling for the availability probe's bus round-trips (name-owner
/// check + introspection). Auto selection calls this on every grab
/// under Wayland, so it must stay short.
const PROBE_TIMEOUT: Duration = Duration::from_secs(2);
/// Per-method-call ceiling on the grab connection.
const METHOD_TIMEOUT: Duration = Duration::from_secs(5);
/// Ceiling for waiting on the `Response` signal after the method call
/// returns its handle. Spike B measured 62–106 ms on COSMIC; 5 s
/// leaves room for a slow GNOME/KDE impl while never hanging the
/// worker.
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(5);
/// Response ceiling for the **interactive** path: the user is mid-drag
/// in the compositor's selection UI, so this budgets human think-time,
/// not machine latency. Spike C observed interactions up to ~33 s;
/// 5 min keeps the grab alive for a distracted user while still
/// bounding a wedged portal (whose no-Response hang would otherwise
/// park forever). The timeout error carries the `"timed out"`
/// substring so callers can exit quietly.
const INTERACTIVE_RESPONSE_TIMEOUT: Duration = Duration::from_mins(5);

/// Cross-desktop portal screenshot backend
/// (`org.freedesktop.portal.Screenshot`, non-interactive).
///
/// `monitors()` rides `xcap` enumeration (trait default) — spike B
/// confirmed enumeration is prompt-free; the portal round-trips once
/// per grab only.
pub struct PortalScreenshotCapturer;

impl Capturer for PortalScreenshotCapturer {
    fn id(&self) -> &'static str {
        "portal"
    }

    fn name(&self) -> &'static str {
        "XDG Screenshot Portal"
    }

    fn available(&self) -> Result<()> {
        probe_portal()
    }

    fn capture_monitor(&self, monitor: Option<u32>, opts: &CaptureOptions) -> Result<RgbaImage> {
        if opts.include_cursor {
            tracing::debug!(
                "portal screenshot backend: protocol has no cursor option; \
                 grabbing cursor-less (CaptureOptions::include_cursor is a documented gap)"
            );
        }
        let monitors = enumerate_monitors()?;
        let target = resolve_target_monitor(&monitors, monitor)?;
        let composite = grab_whole_desktop()?;
        crop_composite_to_monitor(composite, target, &monitors)
    }

    fn supports_selection(&self) -> bool {
        true
    }

    fn capture_selection(&self, opts: &CaptureOptions) -> Result<RgbaImage> {
        if opts.include_cursor {
            tracing::debug!(
                "portal screenshot backend: protocol has no cursor option; \
                 grabbing cursor-less (CaptureOptions::include_cursor is a documented gap)"
            );
        }
        // GOTCHA: passthrough by design — spike C verified the portal's
        // interactive Response returns exactly the selected region (dims
        // track the drag), never an all-outputs composite, so the
        // crop_composite_to_monitor dance must NOT run on this path.
        grab_portal_screenshot(true)
    }
}

/// Map any D-Bus layer error into the core error type.
fn dbus_err<E: std::fmt::Display>(e: E) -> FlintshotError {
    FlintshotError::Capture {
        detail: e.to_string(),
    }
}

/// Connect to the session bus with a per-call method timeout.
fn connect(method_timeout: Duration) -> Result<Connection> {
    zbus::blocking::connection::Builder::session()
        .and_then(|b| b.method_timeout(method_timeout).build())
        .map_err(|e| FlintshotError::Capture {
            detail: format!("cannot reach session bus: {e}"),
        })
}

/// Availability probe: the portal bus name has an owner and the
/// desktop object exports the Screenshot interface.
fn probe_portal() -> Result<()> {
    let conn = connect(PROBE_TIMEOUT)?;

    let dbus = zbus::blocking::fdo::DBusProxy::new(&conn).map_err(dbus_err)?;
    let name = zbus::names::BusName::try_from(PORTAL_NAME).map_err(dbus_err)?;
    let owned = dbus.name_has_owner(name).map_err(dbus_err)?;
    if !owned {
        return Err(FlintshotError::Capture {
            detail: format!("{PORTAL_NAME} is not on the session bus (no xdg-desktop-portal?)"),
        });
    }

    let xml = zbus::blocking::Proxy::new(
        &conn,
        PORTAL_NAME,
        PORTAL_DESKTOP_PATH,
        "org.freedesktop.DBus.Introspectable",
    )
    .and_then(|p| Ok(p.introspect()?))
    .map_err(dbus_err)?;
    if !xml.contains(SCREENSHOT_IFACE) {
        return Err(FlintshotError::Capture {
            detail: format!("portal desktop object has no {SCREENSHOT_IFACE} interface"),
        });
    }
    Ok(())
}

/// Build the `Screenshot` options map for a grab.
///
/// Pure seam so the `interactive` key is unit-testable headless.
/// GOTCHA (spike C, `xdg-desktop-portal-cosmic 0.1.0pop1~abf258b`):
/// `interactive: true` opens the compositor's own selection UI whose
/// Escape handling is unreliable — code 1 (proper cancel), a spurious
/// code 0 capture of a stale rect, and a no-Response hang were all
/// observed. Code 1 keeps flowing as the `"cancelled"` error; the
/// hang is bounded by [`INTERACTIVE_RESPONSE_TIMEOUT`].
fn screenshot_options(token: &str, interactive: bool) -> HashMap<&str, Value<'_>> {
    let mut options: HashMap<&str, Value> = HashMap::new();
    options.insert("handle_token", token.into());
    if interactive {
        options.insert("interactive", true.into());
    }
    options
}

/// Run the non-interactive Screenshot dance and return the decoded
/// composite of **all outputs** (logical-resolution coordinate space —
/// see the module docs on coverage).
fn grab_whole_desktop() -> Result<RgbaImage> {
    grab_portal_screenshot(false)
}

/// Shared `Screenshot` dance for both modes. `interactive = true`
/// opens the compositor's selection UI and (per spike C) returns the
/// exact selection; `false` returns the all-outputs composite.
fn grab_portal_screenshot(interactive: bool) -> Result<RgbaImage> {
    let response_budget = if interactive {
        INTERACTIVE_RESPONSE_TIMEOUT
    } else {
        RESPONSE_TIMEOUT
    };
    let conn = connect(METHOD_TIMEOUT)?;

    // Request handle path derived from our unique name (portal
    // convention: .../request/SENDER/TOKEN, with the sender's ':' and
    // '.' mangled to object-path-safe chars).
    let unique = conn
        .unique_name()
        .ok_or_else(|| FlintshotError::Capture {
            detail: "session bus connection has no unique name".into(),
        })?
        .trim_start_matches(':')
        .replace('.', "_");
    let token = format!("flintshot{unique}");
    let req_path: ObjectPath<'_> =
        format!("/org/freedesktop/portal/desktop/request/{unique}/{token}")
            .try_into()
            .map_err(dbus_err)?;

    // GOTCHA: the match rule must be installed *before* the method
    // call — a fast portal can emit the Response before the call
    // returns, and without the subscription already draining into the
    // iterator's queue the signal is lost (spike B reproduced this
    // with short-lived gdbus clients).
    let rule = MatchRule::builder()
        .sender(PORTAL_NAME)
        .and_then(|b| b.interface(REQUEST_IFACE))
        .and_then(|b| b.path(req_path.clone()))
        .map(zbus::match_rule::Builder::build)
        .map_err(dbus_err)?;
    let mut signals =
        zbus::blocking::MessageIterator::for_match_rule(rule, &conn, Some(64)).map_err(dbus_err)?;

    let options = screenshot_options(&token, interactive);
    let proxy =
        zbus::blocking::Proxy::new(&conn, PORTAL_NAME, PORTAL_DESKTOP_PATH, SCREENSHOT_IFACE)
            .map_err(dbus_err)?;
    let handle: zbus::zvariant::OwnedObjectPath = proxy
        .call("Screenshot", &("", &options))
        .map_err(dbus_err)?;
    if handle.as_str() != req_path.as_str() {
        // The frontend is required to build the handle from our
        // handle_token; anything else means we're subscribed to the
        // wrong path and would just time out. Fail fast instead.
        return Err(FlintshotError::Capture {
            detail: format!(
                "portal returned unexpected request handle {handle} (subscribed to {req_path})"
            ),
        });
    }

    // Exactly one Response per request handle (portal contract): take
    // a single message and exit. If the portal wedges and never
    // responds, this thread parks until the worker process exits —
    // bounded, and only reachable on the pathological path.
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(signals.next());
    });

    let msg = match rx.recv_timeout(response_budget) {
        Ok(Some(Ok(m))) => m,
        Ok(Some(Err(e))) => {
            return Err(FlintshotError::Capture {
                detail: format!("portal Response signal error: {e}"),
            });
        }
        Ok(None) => {
            return Err(FlintshotError::Capture {
                detail: "portal message stream ended without a Response".into(),
            });
        }
        Err(_) => {
            return Err(FlintshotError::Capture {
                detail: format!(
                    "portal screenshot timed out after {} s (no Response signal)",
                    response_budget.as_secs(),
                ),
            });
        }
    };

    if msg.header().member().map(zbus::names::MemberName::as_str) != Some("Response") {
        return Err(FlintshotError::Capture {
            detail: format!(
                "unexpected portal signal: {}",
                msg.header()
                    .member()
                    .map_or("?", zbus::names::MemberName::as_str)
            ),
        });
    }
    let (code, results): (u32, HashMap<String, OwnedValue>) =
        msg.body().deserialize().map_err(dbus_err)?;
    interpret_response(code)?;

    let path = uri_path_from_results(&results)?;
    let bytes = std::fs::read(&path).map_err(|e| FlintshotError::Capture {
        detail: format!("portal screenshot file {} unreadable: {e}", path.display()),
    })?;
    // We own cleanup after decode: non-interactive files are 0600
    // temp files, but GOTCHA interactive (cosmic) responses point into
    // the user-visible ~/Pictures/Screenshots — leaving them would
    // litter the dir on every grab, so both modes read-then-delete.
    if let Err(e) = std::fs::remove_file(&path) {
        tracing::debug!("portal screenshot {} not removed: {e}", path.display());
    }

    let img = image::load_from_memory(&bytes).map_err(|e| FlintshotError::Capture {
        detail: format!("portal screenshot decode failed: {e}"),
    })?;
    Ok(img.to_rgba8())
}

/// Map a portal `Response` code: `0` = success, `1` = cancelled
/// (caller-visible via the `"cancelled"` substring, macOS
/// `"permission"` precedent), anything else is a backend refusal.
fn interpret_response(code: u32) -> Result<()> {
    match code {
        0 => Ok(()),
        1 => Err(FlintshotError::Capture {
            detail: "portal screenshot cancelled (response code 1)".into(),
        }),
        n => Err(FlintshotError::Capture {
            detail: format!("portal screenshot refused: response code {n}"),
        }),
    }
}

/// Pull the screenshot path out of a portal Response's results dict.
fn uri_path_from_results(results: &HashMap<String, OwnedValue>) -> Result<PathBuf> {
    let uri = results.get("uri").ok_or_else(|| FlintshotError::Capture {
        detail: "portal Response carried no uri".into(),
    })?;
    let s = String::try_from(uri.clone()).map_err(|e| FlintshotError::Capture {
        detail: format!("portal uri is not a string: {e}"),
    })?;
    parse_portal_uri(&s)
}

/// Marker in a [`FlintshotError::Capture`] detail when the portal
/// answered `clipboard:///` (COSMIC's Copy-confirmed interactive
/// screenshot): the portal already wrote the grab to the system
/// clipboard and no file exists. The consumer
/// (`native_grab_prepared` in `flintshot-app`) reads the pixels
/// back from the clipboard instead of failing. Producer and
/// consumer both reference this constant so the classification
/// cannot drift.
pub const PORTAL_GRAB_ON_CLIPBOARD: &str = "portal grab is on the system clipboard";

/// Parse the URI the portal returns into a filesystem path.
///
/// Robust to the observed 3-slash (`file:///tmp/x`) and cosmic's
/// 4-slash (`file:////tmp/x`) forms; anything but a `file://` scheme
/// is an error. The one exception: `clipboard:///` means the grab
/// was delivered to the system clipboard by the portal (no file) —
/// surfaced as the [`PORTAL_GRAB_ON_CLIPBOARD`] marker error for
/// read-back by the caller.
fn parse_portal_uri(uri: &str) -> Result<PathBuf> {
    if uri == "clipboard:///" {
        return Err(FlintshotError::Capture {
            detail: PORTAL_GRAB_ON_CLIPBOARD.into(),
        });
    }
    let Some(rest) = uri.strip_prefix("file://") else {
        return Err(FlintshotError::Capture {
            detail: format!("portal returned a non-file uri: {uri}"),
        });
    };
    let trimmed = rest.trim_start_matches('/');
    if trimmed.is_empty() {
        return Err(FlintshotError::Capture {
            detail: format!("portal returned a uri with an empty path: {uri}"),
        });
    }
    Ok(PathBuf::from(format!("/{trimmed}")))
}

/// Resolve the monitor a grab targets: `None` → primary (falling back
/// to the first, mirroring `xcap_backend`'s rule), `Some(n)` → the
/// n-th enumerated monitor.
fn resolve_target_monitor(monitors: &[MonitorInfo], idx: Option<u32>) -> Result<&MonitorInfo> {
    let found = match idx {
        None => monitors
            .iter()
            .find(|m| m.is_primary)
            .or_else(|| monitors.first()),
        Some(n) => monitors.get(n as usize),
    };
    found.ok_or_else(|| FlintshotError::Capture {
        detail: match idx {
            Some(n) => format!(
                "monitor index {n} out of range; {} monitor(s) available",
                monitors.len()
            ),
            None => "no monitors available".into(),
        },
    })
}

/// Crop the all-outputs composite down to the requested monitor.
///
/// Single output: passthrough — the portal preserves that output's
/// physical pixels (spike B). Multiple outputs: the composite is laid
/// out at logical coordinates, so the target's logical rect is
/// derived by scaling the reported monitor bounding box onto the
/// composite (see the module docs on why `scale_factor()` is
/// poison here).
fn crop_composite_to_monitor(
    composite: RgbaImage,
    target: &MonitorInfo,
    all: &[MonitorInfo],
) -> Result<RgbaImage> {
    if all.len() == 1 {
        return Ok(composite);
    }
    let (x, y, w, h) = monitor_rect_in_composite(&composite, target, all)?;
    Ok(image::imageops::crop_imm(&composite, x, y, w, h).to_image())
}

/// Compute the target monitor's rect in composite pixels, clamped to
/// the composite bounds. Returns `(x, y, w, h)`.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    reason = "values clamped to composite bounds before truncation; ratios only ever scale proportionally"
)]
fn monitor_rect_in_composite(
    composite: &RgbaImage,
    target: &MonitorInfo,
    all: &[MonitorInfo],
) -> Result<(u32, u32, u32, u32)> {
    let min_x = all.iter().map(|m| m.x).min().unwrap_or(0);
    let min_y = all.iter().map(|m| m.y).min().unwrap_or(0);
    let span_x: f64 = f64::from(
        all.iter()
            .map(|m| m.x.saturating_add(m.width.cast_signed()))
            .max()
            .unwrap_or(0)
            - min_x,
    );
    let span_y: f64 = f64::from(
        all.iter()
            .map(|m| m.y.saturating_add(m.height.cast_signed()))
            .max()
            .unwrap_or(0)
            - min_y,
    );
    if span_x <= 0.0 || span_y <= 0.0 {
        return Err(FlintshotError::Capture {
            detail: "monitor enumeration reported a degenerate bounding box".into(),
        });
    }

    let scale_x = f64::from(composite.width()) / span_x;
    let scale_y = f64::from(composite.height()) / span_y;
    let rel_x = f64::from(target.x - min_x) * scale_x;
    let rel_y = f64::from(target.y - min_y) * scale_y;
    let w = f64::from(target.width) * scale_x;
    let h = f64::from(target.height) * scale_y;

    // floor/ceil the edges so rounding never shaves coverage; clamp
    // to the composite so crop_imm can't go out of bounds.
    let cw = f64::from(composite.width());
    let ch = f64::from(composite.height());
    let x0 = (rel_x.floor().clamp(0.0, cw)) as u32;
    let y0 = (rel_y.floor().clamp(0.0, ch)) as u32;
    let x1 = ((rel_x + w).ceil().clamp(0.0, cw)) as u32;
    let y1 = ((rel_y + h).ceil().clamp(0.0, ch)) as u32;
    let (w, hgt) = (x1.saturating_sub(x0), y1.saturating_sub(y0));
    if w == 0 || hgt == 0 {
        return Err(FlintshotError::Capture {
            detail: format!(
                "portal composite {}x{} does not cover monitor {} ({}x{} at {},{})",
                composite.width(),
                composite.height(),
                target.index,
                target.width,
                target.height,
                target.x,
                target.y
            ),
        });
    }
    Ok((x0, y0, w, hgt))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mon(index: u32, x: i32, y: i32, width: u32, height: u32, primary: bool) -> MonitorInfo {
        MonitorInfo {
            index,
            name: format!("m{index}"),
            is_primary: primary,
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn portal_advertises_selection_capability() {
        assert!(PortalScreenshotCapturer.supports_selection());
    }

    #[test]
    fn options_map_plain_carries_token_only() {
        let options = screenshot_options("tok", false);
        assert_eq!(
            options.get("handle_token"),
            Some(&Value::from("tok")),
            "handle_token must always ride the options map"
        );
        assert!(
            !options.contains_key("interactive"),
            "non-interactive grabs must not send the interactive key: {options:?}"
        );
    }

    #[test]
    fn options_map_interactive_carries_true() {
        let options = screenshot_options("tok", true);
        assert_eq!(options.get("handle_token"), Some(&Value::from("tok")));
        match options.get("interactive") {
            Some(Value::Bool(true)) => {}
            other => panic!("interactive must be Bool(true), got {other:?}"),
        }
    }

    #[test]
    fn parse_uri_handles_three_and_four_slash_forms() {
        assert_eq!(
            parse_portal_uri("file:////tmp/screenshot-a1.png").unwrap(),
            PathBuf::from("/tmp/screenshot-a1.png")
        );
        assert_eq!(
            parse_portal_uri("file:///tmp/screenshot-a1.png").unwrap(),
            PathBuf::from("/tmp/screenshot-a1.png")
        );
    }

    #[test]
    fn parse_uri_rejects_non_file_and_empty() {
        assert!(parse_portal_uri("http://example.com/x.png").is_err());
        assert!(parse_portal_uri("file:///").is_err());
    }

    #[test]
    fn parse_uri_marks_clipboard_delivery() {
        let err = parse_portal_uri("clipboard:///").unwrap_err();
        match err {
            FlintshotError::Capture { detail } => {
                assert!(
                    detail.contains(PORTAL_GRAB_ON_CLIPBOARD),
                    "detail must carry the on-clipboard marker: {detail}"
                );
            }
            other => panic!("clipboard:/// must be a Capture error, got {other:?}"),
        }
    }

    #[test]
    fn response_code_zero_is_success() {
        interpret_response(0).expect("code 0 is success");
    }

    #[test]
    fn response_code_one_maps_to_cancelled_substring() {
        let err = interpret_response(1).unwrap_err();
        let FlintshotError::Capture { detail } = err else {
            panic!("expected Capture, got {err:?}");
        };
        assert!(
            detail.contains("cancelled"),
            "callers distinguish dismissal via the substring: {detail}"
        );
    }

    #[test]
    fn other_response_codes_are_refusals() {
        let err = interpret_response(7).unwrap_err();
        let FlintshotError::Capture { detail } = err else {
            panic!("expected Capture, got {err:?}");
        };
        assert!(detail.contains('7'), "code number survives: {detail}");
    }

    #[test]
    fn uri_extraction_from_results() {
        let mut results = HashMap::new();
        results.insert(
            "uri".to_string(),
            OwnedValue::from(zbus::zvariant::Str::from("file:////tmp/shot.png")),
        );
        assert_eq!(
            uri_path_from_results(&results).unwrap(),
            PathBuf::from("/tmp/shot.png")
        );

        assert!(
            uri_path_from_results(&HashMap::new()).is_err(),
            "missing uri is an error"
        );

        let mut wrong_type = HashMap::new();
        wrong_type.insert("uri".to_string(), OwnedValue::from(7_u32));
        assert!(
            uri_path_from_results(&wrong_type).is_err(),
            "non-string uri is an error"
        );
    }

    #[test]
    fn resolve_target_prefers_primary_then_first() {
        let monitors = vec![
            mon(0, 0, 0, 100, 100, false),
            mon(1, 100, 0, 100, 100, true),
        ];
        assert_eq!(resolve_target_monitor(&monitors, None).unwrap().index, 1);
        assert_eq!(resolve_target_monitor(&monitors, Some(0)).unwrap().index, 0);

        let no_primary = vec![mon(0, 0, 0, 100, 100, false)];
        assert_eq!(resolve_target_monitor(&no_primary, None).unwrap().index, 0);

        let err = resolve_target_monitor(&monitors, Some(2)).unwrap_err();
        let FlintshotError::Capture { detail } = err else {
            panic!("expected Capture, got {err:?}");
        };
        assert!(
            detail.contains("out of range"),
            "out-of-range names the problem: {detail}"
        );

        assert!(resolve_target_monitor(&[], None).is_err());
    }

    #[test]
    fn crop_single_monitor_is_passthrough() {
        // Reported geometry inflated 3x vs the composite — irrelevant
        // with one output: physical px preserved, no crop at all.
        let inflated = vec![mon(0, 0, 0, 7680, 4320, true)];
        let mut composite = RgbaImage::new(2560, 1440);
        composite.put_pixel(5, 5, image::Rgba([9, 9, 9, 255]));

        let out = crop_composite_to_monitor(composite.clone(), &inflated[0], &inflated)
            .expect("single monitor passthrough");
        assert_eq!(out.dimensions(), (2560, 1440));
        assert_eq!(out.get_pixel(5, 5), &image::Rgba([9, 9, 9, 255]));
    }

    #[test]
    fn crop_two_monitors_picks_the_requested_half() {
        let monitors = vec![
            mon(0, 0, 0, 100, 100, false),
            mon(1, 100, 0, 100, 100, true),
        ];
        let mut composite = RgbaImage::new(200, 100);
        for x in 0..200 {
            for y in 0..100 {
                let color = if x < 100 {
                    [255, 0, 0, 255]
                } else {
                    [0, 0, 255, 255]
                };
                composite.put_pixel(x, y, image::Rgba(color));
            }
        }

        let right = crop_composite_to_monitor(composite.clone(), &monitors[1], &monitors).unwrap();
        assert_eq!(right.dimensions(), (100, 100));
        assert_eq!(right.get_pixel(50, 50), &image::Rgba([0, 0, 255, 255]));

        let left = crop_composite_to_monitor(composite, &monitors[0], &monitors).unwrap();
        assert_eq!(left.get_pixel(50, 50), &image::Rgba([255, 0, 0, 255]));
    }

    #[test]
    fn crop_derives_scale_from_bounding_box_not_scale_factor() {
        // The COSMIC quirk: xcap reports 7680x4320 per monitor for a
        // physical 2560x1440 panel. Two such panels side by side; the
        // portal composite spans both at logical resolution: 5120x1440.
        // The crop must map monitor 1's reported rect (7680..15360)
        // through the bbox ratio (5120/15360 = 1/3), yielding
        // (2560, 0, 2560, 1440) — scale_factor() would give nonsense.
        let monitors = vec![
            mon(0, 0, 0, 7680, 4320, true),
            mon(1, 7680, 0, 7680, 4320, false),
        ];
        let rect = monitor_rect_in_composite(&RgbaImage::new(5120, 1440), &monitors[1], &monitors)
            .expect("inflated geometry still crops");
        assert_eq!(rect, (2560, 0, 2560, 1440));
    }

    #[test]
    fn crop_zero_extent_monitor_is_an_error() {
        let monitors = vec![mon(0, 0, 0, 100, 100, true), mon(1, 100, 0, 0, 0, false)];
        assert!(
            monitor_rect_in_composite(&RgbaImage::new(200, 100), &monitors[1], &monitors).is_err(),
            "a zero-coverage rect must not reach crop_imm"
        );
    }

    #[test]
    fn crop_degenerate_bounding_box_is_an_error() {
        let monitors = vec![mon(0, 0, 0, 0, 100, true), mon(1, 0, 0, 0, 100, false)];
        assert!(
            monitor_rect_in_composite(&RgbaImage::new(10, 10), &monitors[0], &monitors).is_err()
        );
    }
}
