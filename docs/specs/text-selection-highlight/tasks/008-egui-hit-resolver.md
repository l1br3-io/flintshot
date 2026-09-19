# Task 008: egui — shared pos→char hit resolver

**Description:** New module `crates/flintshot-editor-egui/src/texthit.rs`: a pure, closure-based resolver `char_at_hit(text, style, text_pos, hit, line_height, measure: impl Fn(&str) -> f32) -> Option<usize>` — containment (hit outside the laid-out block → None), line index from `hit.y / line_height`, then binary search over prefix widths on that line to the nearest char boundary (ties snap right). The pumps pass a view-aware `measure` closure that lays out galleys exactly like `EguiCanvas::measure_text` (same FontId family/size path — the drift guard). Design is closure-injected so unit tests use synthetic widths with zero egui context.

**Acceptance criteria:**
- [ ] Hit on char midpoint of "hello" returns that boundary; hits between chars snap to nearest (tie → right)
- [ ] Hit left of the block / above / below the last line → None; hit right of a short line's end snaps to that line's end (industry: click past EOL goes to EOL, not next line)
- [ ] Multi-line: y picks the line; x resolves within it; `TextAlign` centre/right accounted (measure closure receives prefixes; alignment offset applied via full-line width like `caret_coords`)
- [ ] Empty text → Some(0) for any inside hit; synthetic-width unit tests cover every branch above without egui
- [ ] Both pumps can construct the real closure (ctx + view) with ≤ ~10 lines each — the drift guard: same font family const and `view.len(size)` conversion as `EguiCanvas::measure_text`

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui` (closure-based unit tests)
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`

**Dependencies:** Task 006 (API contract it feeds), Task 004 (alignment math precedent)

**Files likely touched:**
- `crates/flintshot-editor-egui/src/texthit.rs` (new)
- `crates/flintshot-editor-egui/src/lib.rs` (module registration)

**Docs:**
- Module doc states the architecture decision (shell owns glyph metrics; core owns state) and the pump contract.

**Estimated scope:** Small: 1–2 files
