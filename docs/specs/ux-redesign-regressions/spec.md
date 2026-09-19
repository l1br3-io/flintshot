---
status: active       # draft → active → shipped (flipped by /build and on merge)
started: 2026-09-18
---

# Spec: UX redesign regressions — home geometry, editor Copy, tofu icons

## Objective

Three regressions surfaced while testing the in-flight `ux-redesign`
effort (branch `ux-redesign`, uncommitted work in tree). This effort
fixes them on top of that work; it does not change the redesign's
design of record.

1. **Home window is tiny.** `route_geometry(&Route::Home)`
   (`crates/flintshot-editor-egui/src/window.rs:159`) still returns
   the pre-redesign compact geometry — 360×240, non-resizable — pinned
   by `home_route_keeps_its_compact_fixed_geometry` (a leftover from
   single-window OQ-4's three-button home). The redesigned home
   (topbar + hero card + three hcards + recents grid) cannot fit: the
   topbar alone needs ~460 px, so on Home the hero buttons overflow
   and clip leftward past the brand.
2. **Copy looks and behaves dead in the editor page.** Two proven
   defects, one open question:
   - The Copy hero's `⧉` icon is **tofu** (see 3) and `self.copied`
     is set on success (`app.rs:1619`) but rendered **nowhere** — the
     old panel `copy_chip`'s "Copied!" morph was retired with the
     five-zone rework and no replacement landed. A successful copy
     produces zero visible change.
   - **Proven by probe**: the click chain works — topbar Copy hero on
     the routed editor page → `handle_topbar_action` → `handle_copy`
     → `copy_to_clipboard` → arboard `Ok`; and with a live process,
     `wl-paste --list-types` serves `image/png`. The page path never
     closes the window, so the in-process wl-clipboard-rs serve
     thread keeps answering.
   - **Open question (user reports paste fails)**: the isolated
     reproduction pastes fine. Residual suspicion concentrates on the
     **worker-window** path: `close_after_copy_window()` returns true
     on Wayland for workers, the process exits after the close, and
     the probe showed COSMIC does **not** take ownership of a
     zwlr_data_control selection from an exited client — the
     clipboard empties. The doc comment's compositor-ownership
     assumption (`app.rs:1608-1614`) is false for arboard's
     thread-serve model. See Open Questions.
3. **Topbar icons are tofu.** The topbar renders icons as Unicode
   text through the Jakarta-first font chain. A cmap scan of the
   whole chain (Plus Jakarta Sans, Ubuntu-Light, NotoEmoji,
   emoji-icon-font) proves these codepoints are **in none of them**:

   | Char  | Codepoint | Where                            |
   | ----- | --------- | -------------------------------- |
   | `⋯`   | U+22EF    | More / overflow chip             |
   | `⧉`   | U+29C9    | Copy hero                        |
   | `▸`   | U+25B8    | breadcrumb separator             |
   | `⤓`   | U+2913    | overflow Save row                |
   | `⇧`   | U+21E7    | overflow Upload row              |
   | `✓`   | U+2713    | checkmarks if any text uses it   |

   The reference mockups (`docs/specs/ux-redesign/reference/`) render
   these fine only because browsers have unlimited fallback fonts.
   The rail / panel / floating icons are painted vector `Glyph` fns
   (`rail::Glyph = fn(&Painter, Rect, Color32)`) and are unaffected —
   the fix follows that established pattern.

**User**: the maintainer dogfooding `flintshot app` on COSMIC
(Wayland) while iterating on the ux-redesign branch.

### Decisions from clarification (binding)

| Decision         | Choice                                                        |
| ---------------- | ------------------------------------------------------------- |
| Icon fix         | **Painted vector glyphs** for all topbar/overflow/breadcrumb icons (rail's `Glyph` fn approach) — no font dependence |
| Home geometry    | **Match the editor routes exactly**: 1024×640, resizable, same transition batch semantics |
| Effort home      | **New effort spec** (`ux-redesign-regressions`), referencing the parent; implementation lands on a branch of the same name cut from `ux-redesign` |
| Copy feedback    | Topbar Copy hero gains a visible **"Copied" state** (check glyph + label morph, ~2 s revert) fed from the editor session's `copied` flag |

## Technologies

Unchanged from the repo: Rust 2021 / MSRV 1.95, egui-eframe 0.35,
arboard 3 (workspace features: `image-data`, `wayland-data-control`).
**No new dependencies.**

## Commands

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test --doc -p flintshot-core -p flintshot-editor-egui
cargo run -- app        # manual: home size, topbar icons, copy feedback
```

Manual clipboard protocol (the copy verification):

```sh
cargo run -- app &      # take/open a capture → editor page → click Copy
wl-paste --list-types   # must include image/png
wl-paste --type image/png | wc -c
```

## Project Structure

```
crates/flintshot-editor-egui/src/
├── window.rs      # route_geometry: Home → stable geometry; dispatch
│                  #   Capture on the Settings route (dead-control fix)
├── topbar.rs      # painted glyphs replace text icons; Copy hero's
│                  #   copied state; overflow row icons
├── widgets.rs     # ghost_button takes a Glyph fn instead of &str icon
│                  #   (+ copied-state variant), if the signature work
│                  #   lands there rather than in topbar
├── rail.rs        # glyph helpers exported/shared for topbar reuse
├── app.rs         # expose copied state to the router-drawn topbar;
│                  #   worker copy-close policy (ask-first item)
└── home.rs        # layout tolerates 1024×640 + resize (ScrollArea if
                    #   content overflows)
docs/specs/ux-redesign-regressions/
└── spec.md        # this file
```

## Code Style

Repo conventions (AGENTS.md) apply. New rule this effort adds,
enforced by test:

```rust
// GOOD — topbar icons are painted vector glyphs, like the rail's
widgets::ghost_button(ui, id, g_copy, "Copy", Ghost::HeroKey, enabled);

// BAD — text-glyph icons through the font chain (tofu risk)
widgets::ghost_button(ui, id, "⧉", "Copy", Ghost::HeroKey, enabled);
```

All `Color32` construction stays in `theme/palette.rs`; glyphs paint
with tokens only.

## Testing Strategy

- **Geometry**: update `home_route_keeps_its_compact_fixed_geometry`
  → `home_matches_the_stable_geometry` (1024×640, resizable, title
  `flintshot`); Home↔Editor transition batches still issue exactly
  `InnerSize` + `Resizable` + `Title`.
- **Tofu ban** (static scan, mirrors the `Color32` token-scan test):
  none of `⋯ ⧉ ▸ ⤓ ⇧ ✓` (and any other char proven absent from the
  embedded chain) may appear in `crates/flintshot-editor-egui/src` —
  icons must be painted glyphs. The scan's char list derives from a
  cmap check of the actual font files, documented in the test.
- **Copy feedback**: routed-window test clicking the topbar Copy hero
  (clipboard seam stubbed or env-guarded) asserts the hero's copied
  state renders (label/glyph swap in painted shapes) and resets on a
  fresh editor route.
- **Worker close policy**: pure test of the revised
  `close_after_copy` decision table (see Open Questions for the
  policy).
- **Regression**: full existing suite stays green (336 tests today).

## Boundaries

- **Always**: run the full gate; keep all ten tools + shortcuts
  working; painted glyphs consume palette tokens only; update
  CHANGELOG under `## [Unreleased]`; fix forward on `ux-redesign`'s
  uncommitted work (never revert it).
- **Ask first**: changing the worker-window close-after-copy
  lifecycle (ADR territory — window/close choreography); touching
  anything outside the three defects + the two named adjacency fixes
  (Settings-route Capture dispatch, `copied` feedback).
- **Never**: add a symbol-font dependency; text-glyph icons in the
  topbar; `unsafe`; `#[allow]` without a justifying comment.

## Success Criteria

1. `flintshot app` opens Home at 1024×640, resizable; Home ↔ Editor
   ↔ Settings transitions swap geometry consistently; the topbar
   never clips or overflows on Home.
2. Every topbar / overflow / breadcrumb icon renders as a painted
   vector glyph — no tofu at any zoom or ppp; the tofu-ban scan test
   is green.
3. Clicking Copy in the editor page shows the "Copied" state (check
   glyph + label) for ~2 s; `wl-paste --list-types` lists `image/png`
   after a copy while the app runs (manual protocol).
4. The overflow's More chip opens its menu (painted dots glyph); all
   overflow rows show painted icons.
5. Settings-route Capture click starts a capture (no dead control).
6. Full gate green; CHANGELOG updated.

## Open Questions

- **Page-mode paste failure (unreproduced)**: the isolated page-path
  copy provably serves the clipboard on COSMIC while the process
  lives. After the feedback fix lands, retest with the manual
  protocol — if paste still fails, capture: paste-target app (X11 or
  Wayland), timing relative to app quit, and `wl-paste` output.
- **Worker close-after-copy on Wayland** (proven defect, ask-first
  policy change): the probe showed the selection dies with the
  process — COSMIC does not adopt an exited client's zwlr selection.
  Recommendation: stop closing worker windows after Copy on Wayland
  (align with page mode: stay open, show "Copied"); alternative is a
  lingering detached serve process (wl-copy-style fork), which is
  heavier. Decide before this effort ships; the `close_after_copy`
  doc comment must be corrected either way.
