//! XDG `ScreenCast` portal — the `Wayland` source.
//!
//! The portal dance (`ashpd`, tokio backend, matching the app's
//! portal client): create session → select sources (monitor,
//! embedded cursor, persist-until-revoked so a restore token is
//! granted) → start → open the `PipeWire` remote. The returned
//! [`PortalStream`] carries the fd `pipewiresrc` connects to and the
//! node id it targets; the fd must outlive the pipeline, so the
//! caller keeps the [`OwnedFd`] alive for the whole recording.
//!
//! Denial (user dismissed the consent dialog) maps to
//! [`RecordError::PortalDenied`] via the portal's structured
//! `Response(Cancelled)` error — no message sniffing.
//!
//! The restore token is *in/out on the config*: pass the previous
//! recording's token in to skip the consent prompt; read the fresh
//! token off the prepared [`crate::Recorder`] and persist it (the
//! caller owns storage — config, per the engine ADR).

use std::os::fd::OwnedFd;

use ashpd::desktop::screencast::{
    CursorMode, OpenPipeWireRemoteOptions, Screencast, SelectSourcesOptions, SourceType,
    StartCastOptions,
};
use ashpd::desktop::{CreateSessionOptions, PersistMode, ResponseError};
use ashpd::enumflags2::BitFlags;

use crate::RecordError;

/// What the portal handed us for one recording.
#[derive(Debug)]
pub struct PortalStream {
    /// The `PipeWire` node the stream lives on — `pipewiresrc`'s
    /// `target-object`.
    pub node_id: u32,
    /// The open `PipeWire` remote fd — `pipewiresrc`'s `fd`. Keep it
    /// alive for as long as the pipeline records.
    pub fd: OwnedFd,
    /// The session's restore token (present when the portal grants
    /// persistence) — feed it back on the next recording to skip
    /// the prompt.
    pub restore_token: Option<String>,
}

/// Run the `ScreenCast` dance and return the ready stream.
///
/// `restore_token` from a previous session skips the source-picker
/// prompt where the portal implementation honors tokens.
///
/// # Errors
/// [`RecordError::PortalDenied`] when the user refuses the consent
/// dialog; [`RecordError::PipelineFailed`] (element `portal`) for
/// every other portal failure, with the stage named.
pub async fn open_stream(restore_token: Option<&str>) -> Result<PortalStream, RecordError> {
    let screencast = Screencast::new()
        .await
        .map_err(|err| portal_error("connect", &err))?;
    let session = screencast
        .create_session(CreateSessionOptions::default())
        .await
        .map_err(|err| portal_error("create session", &err))?;

    let mut sources = SelectSourcesOptions::default()
        .set_cursor_mode(CursorMode::Embedded)
        .set_sources(BitFlags::from(SourceType::Monitor))
        .set_multiple(false)
        .set_persist_mode(PersistMode::ExplicitlyRevoked);
    if let Some(token) = restore_token {
        sources = sources.set_restore_token(token);
    }
    screencast
        .select_sources(&session, sources)
        .await
        .map_err(|err| portal_error("select sources", &err))?
        .response()
        .map_err(|err| portal_error("select sources", &err))?;

    let streams = screencast
        .start(&session, None, StartCastOptions::default())
        .await
        .map_err(|err| portal_error("start", &err))?
        .response()
        .map_err(|err| portal_error("start", &err))?;
    let Some(stream) = streams.streams().first() else {
        return Err(RecordError::PipelineFailed {
            element: "portal".into(),
            message: "the portal offered no streams".into(),
        });
    };

    let fd = screencast
        .open_pipe_wire_remote(&session, OpenPipeWireRemoteOptions::default())
        .await
        .map_err(|err| portal_error("open pipewire remote", &err))?;

    Ok(PortalStream {
        node_id: stream.pipe_wire_node_id(),
        fd,
        restore_token: streams.restore_token().map(str::to_owned),
    })
}

/// Map an ashpd failure at a named stage onto the record contract.
/// The portal's structured `Response(Cancelled)` is the consent-deny
/// signal; everything else is a stage-named pipeline failure.
fn portal_error(stage: &str, err: &ashpd::Error) -> RecordError {
    if matches!(err, ashpd::Error::Response(ResponseError::Cancelled)) {
        RecordError::PortalDenied
    } else {
        RecordError::PipelineFailed {
            element: "portal".into(),
            message: format!("{stage}: {err}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancelled_response_is_portal_denied() {
        let err = ashpd::Error::Response(ResponseError::Cancelled);
        assert_eq!(portal_error("start", &err), RecordError::PortalDenied);
    }

    #[test]
    fn other_failures_carry_the_stage() {
        let err = ashpd::Error::NoResponse;
        assert_eq!(
            portal_error("connect", &err),
            RecordError::PipelineFailed {
                element: "portal".into(),
                message: "connect: Portal request didn't receive a response".into(),
            }
        );
    }
}
