# ADR-0026: `reqwest` for upload HTTP

**Status.** Accepted.

**Context.** Phase 4 adds the workspace's first outbound HTTP: Imgur's
`POST /3/image` with a `multipart/form-data` body and a JSON response.
Candidates:

- `reqwest` — de-facto Rust HTTP client; tokio-native; first-class
  multipart and JSON support; rustls available as the TLS backend
- `ureq` — synchronous, blocking, small dep tree; multipart via a
  side crate
- `attohttpc` — synchronous, small; multipart support thinner
- `surf` / `isahc` — maintenance and fit concerns

Constraints: uploads already execute on a tokio runtime (`run_upload`
builds one and `block_on`s the backend future — the workspace-standard
model since ADR-0018); Imgur needs multipart encoding and JSON decoding;
ADR-0001 commits the stack to pure Rust, so TLS should be rustls rather
than OpenSSL; and the HTTP client must stay out of `flintshot-core` —
the COSMIC applet links core + libcosmic only and must not inherit a
heavy network stack.

**Decision.** `reqwest` 0.12 as a workspace dependency, consumed only by
`flintshot-app`, configured `default-features = false` with features
`["rustls-tls", "multipart", "json"]`. `ImgurBackend` lives in
`flintshot-app::upload::imgur`; core keeps only the toolkit-agnostic
trait (ADR-0004). `ureq` / `attohttpc` were rejected because both are
synchronous: the worker would pay a blocking client inside an async
runtime (or forgo async entirely), and neither ships first-class
multipart. reqwest's cost is its transitive tree (hyper, tower, h2, …),
confined to `flintshot-app`.

**Consequences.**

- Native async fits the existing runtime; no block-on-inside-async
  hazards, and concurrent uploads stay cheap.
- Multipart boundaries and JSON decoding are library features — no
  hand-rolled encoding against Imgur's API shape.
- rustls keeps OpenSSL out of the link, consistent with ADR-0001 and
  the cross-platform build story (same rationale as `keyring`'s
  `crypto-rust` feature).
- `flintshot-core` and `flintshot-cosmic` dep graphs are unchanged;
  the applet stays a thin IPC client with no HTTP client inside.
- `flintshot-app`'s dep tree grows accordingly — accepted for the
  batteries; revisit only if binary size or compile time becomes a
  release blocker.
- Testability: `ImgurBackend::with_base_url` swaps the API origin so
  `mockito` stands in for `api.imgur.com`. Success, 429, 401, 403,
  malformed-body, and transport-failure mappings are tested headless;
  CI never touches the real network.
