# Task 003: Core — bar caret replaces block caret

**Description:** Retire the solid block caret in `draw_text_indicators` (render.rs): replace it with a thin vertical bar — width `bar_caret_width(size) = max(1.5, size / 24.0)` physical px, height `style.size` (unchanged), positioned at the same `caret_coords` x/line. The bounding box extension logic switches from `block_caret_width` to the bar width. Revise (do not delete) the three block-caret contract tests to lock the new geometry exactly; keep `render_committed_text_emits_no_caret` passing as-is.

**Acceptance criteria:**
- [ ] Pending text emits exactly one caret `DrawRectFilled` with `rect.width == bar_caret_width(size)` and `rect.height == size`, at the caret position (prefix width + align offset), in `style.color`
- [ ] Empty draft still emits the bar at `pos.x` (width stays ≥ 1.5 px so it's visible with no text)
- [ ] Bounding box spans `max(metrics.width, caret_dx + bar_w) + 2*pad` — same formula, bar width substituted
- [ ] The three revised contract tests assert exact geometry (position, size, colour), matching the assertiveness of the block versions
- [ ] `block_caret_width` and its helper references are gone (no dead code)

**Verification:**
- [ ] `cargo test -p flintshot-core` (render tests + the two egui-side caret parity tests if they reference caret width — fix them here too)
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Manual: `cargo run -- app`, place text — thin bar visible while typing, no block

**Dependencies:** None (independent of 001/002; lands after them only for ordering)

**Files likely touched:**
- `crates/flintshot-core/src/render.rs`
- possibly `crates/flintshot-editor-egui/src/canvas.rs` (test-canvas caret assertions, if any mirror block width)

**Docs:**
- `draw_text_indicators` doc comment updated (block → bar); gotcha about physical-px width at high `pixels_per_point` → inline comment.

**Estimated scope:** Small: 1–2 files
