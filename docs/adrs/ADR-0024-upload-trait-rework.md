# ADR-0024: Upload trait rework — direct `keyring::Entry` credentials, no progress sink

**Status.** Accepted.

**Context.** Phase 0 (ADR-0004) declared the `UploadBackend` trait with
placeholder types: `UploadContext.credentials` was a `CredentialHandle`
carrying only a backend-name string, and `UploadContext.progress` was a
`ProgressSink` wrapping a `&mut f32`. Phase 4's Imgur backend needs real
credentials — the Imgur Client-ID lives in the OS keychain (the
secrets-stay-out-of-config boundary), never in `config.toml`. That forced
two decisions: how `UploadContext` references credentials (a
`CredentialSource` trait object was the spec-draft alternative), and
whether the progress plumbing earns its keep. A third constraint shapes
both: `UploadContext` is a `flintshot-core` public type, and core's
public API must stay free of GUI toolkit types (enforced by
`no_gui_types_in_core.rs`) — whatever represents credentials must not
weaken that boundary.

**Decision.**

- **`credentials` is `Option<&'a keyring::Entry>` — a direct reference,
  not a trait object.** `None` means anonymous mode (no keychain access
  at all); `Some(entry)` means the caller resolved the entry and the
  backend reads it via `get_password()` at upload time. `keyring` is
  small, stable, synchronous, and not a GUI toolkit, and Phase 4 has
  exactly one concrete credential shape (a per-backend Client-ID
  string). The indirection of a `CredentialSource` trait plus a
  registry would buy nothing with one implementation — YAGNI.

- **`keyring` (v3, `crypto-rust` feature) becomes a `flintshot-core`
  dep** because `keyring::Entry` appears as a parameter type on the
  public `UploadContext`. It is a secrets handle, not a GUI toolkit:
  the boundary scanner (which rejects `egui` / `libcosmic` / `iced` /
  `slint` / `interprocess` / `tray_icon` / …) stays green with no
  scanner change.

- **Callers own entry resolution and fallback.** `flintshot upload`
  builds the entry (service `flintshot`, entry name
  `<backend>:client-id`) and passes `Some(&entry)`; a failed keychain
  read inside `ImgurBackend` degrades to anonymous mode with a
  `tracing::info!` line rather than failing the upload — a headless
  box without an unlocked keychain still uploads.

- **`progress` is dropped and `ProgressSink` removed.** No Phase 4
  consumer exists: the event stream carries two discrete signals
  (`UploadStarted` → `Uploaded`, per the spec-confirmation interview's
  pick over percentage progress) and notifications fire on completion.
  Callers that care about timing wrap the future externally. The trait
  is `0.0.0`-versioned; a progress channel can be re-added when a real
  consumer (a progress-bar UI) appears.

- Related, landed in the same effort (Task 004): `upload` / `delete`
  return `Pin<Box<dyn Future>>` so the trait is dyn-compatible — the
  backend registry hands out `Arc<dyn UploadBackend>`.

**Consequences.**

- `UploadContext` construction is cheap and synchronous; anonymous
  uploads never touch the keychain, so zero-config works everywhere.
- Breaking change to a `0.0.0`-versioned public type — allowed by the
  pre-1.0 contract; `flintshot-app` callers updated in the same change.
- Core's dep graph grows by `keyring` only; `reqwest` stays out of
  core (see ADR-0026), so the COSMIC applet links neither.
- A future backend with a richer credential shape (OAuth token pairs,
  SSH keys) would outgrow `Option<&Entry>`. When that happens the
  migration is mechanical — introduce the trait then, with two real
  consumers to model it against, instead of speculatively now.
- Progress reporting, if ever needed, will arrive as a channel-typed
  field designed against a real UI — not a restoration of the
  `&mut f32` stub.
