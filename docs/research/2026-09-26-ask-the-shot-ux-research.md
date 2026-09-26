# Ask-the-shot UX research — patterns to adapt, from where

Research notes, 2026-09-26. Primary sources only (app makers' own
sites, support/user guides, official manuals, first-party blogs,
official extension directories); every URL cited below was fetched and
re-verified today. Scope: the Ask-the-shot **UX surface** — entry
point, prompt affordance, result presentation, iteration/follow-up,
streaming/loading states, error and unconfigured states,
history/persistence, privacy posture — against what v1 shipped
([`../specs/ai-transforms/spec.md`](../specs/ai-transforms/spec.md))
and what Phase 7 plans
([`../ROADMAP.md`](../ROADMAP.md)). Companion to
[`2026-09-26-phase7-adaptation-research.md`](2026-09-26-phase7-adaptation-research.md),
which settled the *wire/impl* layer (SSE, cancellation, repaint); this
file settles the *interaction* layer. Claims I could not verify from a
primary source are flagged in §7 rather than guessed.

## 1. Baseline: what flintshot ships today

From the shipped code (all `crates/flintshot-editor-egui/src/`):
entry is the rail's spark chip or the `I` key → the transform dialog
(`ai_dialog.rs`: saved-transform radio pills + custom prompt, mutually
exclusive; gold Run; a **Configure AI…** link instead of a dead Run
when unconfigured). Results land in the right panel's AI tab
(`ai_panel.rs`: shot thumbnail, user chips right-aligned, assistant
markdown cards with the arrival hairline, berry error cards, a
one-line follow-up pill that goes **inert while a request is
pending**, a "Thinking…" spinner). Config lives on its own route
(`ai_config.rs`: provider presets, model, base-URL override, masked
Replace-key, Test connection, transforms CRUD). Conversation state is
in-memory only; a new transform replaces the thread; responses are
non-streaming. Constraints that filter every recommendation below:
egui immediate mode, no webview/Electron, BYO key (keyring, never
echoed), single binary, every network call user-initiated
(ADR-0069).

The deltas this research targets: no Stop, no streaming render, no
copy affordance on answers, no regenerate/retry, no starter transforms
(empty store shows invitation copy), the pill locks during flight, no
error classification, no privacy/egress copy anywhere in the AI UI.

## 2. Screenshot tools with AI — the honest state of the market

| Tool | AI surface (per first-party pages) | Where AI runs | BYO key? |
| --- | --- | --- | --- |
| [CleanShot X](https://cleanshot.com/features) | "Text recognition (OCR)" + capture history (≤1 month); **no LLM/chat feature** in the official feature list | On-device (macOS vision) | n/a |
| [Shottr](https://shottr.cc) | Instant OCR with post-processing niceties (remove-linebreaks button, double-space fix in [v1.8.1](https://shottr.cc)); object removal | On-device — ["text recognition happens locally… never transferred over the internet"](https://shottr.cc/kb/privacy) | n/a |
| [Xnapper](https://xnapper.com) | ["Automatic redact sensitive information — Emails, credit cards, IP addresses, API keys… Hide them with a single click"](https://xnapper.com); "fast, on-device text recognition" | On-device (macOS vision) | n/a |
| [ShareX](https://getsharex.com) | OCR as one after-capture task among many: "copy… save… open in the image editor, upload… **or recognize text with OCR**" ([docs](https://getsharex.com/docs/ocr.html)); "No AI version" per [issue #8348](https://github.com/ShareX/ShareX/issues/8348) (phase7 §7) | On-device (Windows.Media.Ocr) | n/a |
| [Zight](https://zight.com/ai) | ["Turns every recording into a summary, chapters and a step-by-step guide with screenshots"](https://zight.com/ai); [Smart Actions](https://zight.com/smart-actions) auto-generate notes/SOPs | Zight's cloud, part of the subscription | No |
| [Snipping Tool](https://blogs.windows.com/windows-insider/2025/04/15/text-extractor-in-snipping-tool-begins-rolling-out-to-windows-insiders/) (Windows) | Text Extractor: region → OCR → clipboard, with "Copy all text", "Remove line breaks", "Automatically copy text"; LLM actions live in [Click to Do](https://support.microsoft.com/en-us/windows/ai/ai-features/click-to-do-do-more-with-what-s-on-your-screen), reachable *from* Snipping Tool (≥11.2411.20.0) | OCR on-device; Click to Do's Phi Silica SLM on-NPU, **local** | n/a (OS feature) |

Three findings matter for us. First, **no capture-and-annotate
incumbent ships an in-app LLM chat** — CleanShot, Shottr, ShareX, and
flameshot (phase7 §7) all stop at OCR; the one tool with serious AI
(Zight) is a cloud service where the AI *is* the product. flintshot's
Ask-the-shot is differentiation, not parity chasing — which matches
the NormCap finding in phase7 §7 (its constituency wants local-only).

Second, the OS vendors solved the same "act on what's on screen"
problem and their answers are instructive. Microsoft's
[Click to Do](https://support.microsoft.com/en-us/windows/ai/ai-features/click-to-do-do-more-with-what-s-on-your-screen)
pairs **preset one-press actions** (Summarize, Create a bulleted list,
Rewrite Casual/Formal/Refine) with an escape hatch — **"Ask Copilot"**,
where "you can choose to add your own prompt to go along with your
highlighted content or select a prompt suggestion." That is *exactly*
our dialog's shape (saved transforms + free-text prompt), which is
reassuring; what we lack is their starter preset set and their
privacy copy: "The analysis of this screenshot is always performed
locally on your device… Content is only shared if you choose to
complete an action," and the rewrite actions "stream… back the
response from the model" from the on-device Phi Silica SLM. The
[Text Extractor](https://blogs.windows.com/windows-insider/2025/04/15/text-extractor-in-snipping-tool-begins-rolling-out-to-windows-insiders/)
post states the step-minimization goal outright: "designed to
**minimize the number of steps** it takes to copy text to your
clipboard without having to take a screenshot," with a "Copy all
text" button, a "Remove line breaks" option, and an "Automatically
copy text" mode that "will dismiss Snipping Tool earlier." Shottr's
changelog shows the same post-processing instinct (remove-linebreaks
button + default setting; double-space fix).

Third, Xnapper's auto-redact (emails, credit cards, IPs, API keys in
one click) is the strongest "transform preset" UX in the category —
deterministic, local, zero-prompt. It is a *local-detection* feature,
not an LLM one; worth remembering when we design redaction (a
regex-local preset would honor the privacy posture better than a
model round-trip).

**Adopt:** ship built-in starter transforms (Raycast ships built-in
AI Commands — §3 — and Click to Do ships its rewrite/summarize set;
our empty store currently shows only invitation copy); treat
step-count as the metric for the re-run path (Text Extractor's
stated goal); steal Shottr/Snipping Tool's "remove line breaks"
style post-processing toggles for extract-flavored transforms.

## 3. Selection → action: entry points worth borrowing

**PopClip** (Pilotmoon, current at [2026.8.1](https://pilotmoon.com/popclip/)):
"PopClip appears when you select text in any app, giving you instant
access to useful actions." The relevant artifact for us is its
[official OpenAI Chat extension](https://www.popclip.app/extensions/x/48f32j)
(maintained by PopClip's author, updated 2026-09-08): selection →
one action → answer, with the key note "Requires pre-paid API credits
on your OpenAI account. API usage is not included in ChatGPT Plus
subscription" — first-party acknowledgment that BYO-key billing must
be stated plainly to users. PopClip proves the minimal viable shape:
no chat surface at all, just select-act-result.

**Raycast** is the deepest first-party-documented reference (official
[manual](https://manual.raycast.com/ai), all pages verified today):

- **[Quick AI](https://manual.raycast.com/ai/quick-ai)** — the
  zero-friction path: type a question in the launcher, press Tab,
  "Quick AI streams the answer back in the same window. On the
  response, ↵ pastes the answer into the previously focused app by
  default" (or copy — a configurable "Primary Action"). Auto-new chat
  after inactivity (5 min…never) so new questions don't glom onto an
  old thread. Regenerate is ⌘R; **regenerate with a different model
  is ⇧⌘R**.
- **[AI Commands](https://manual.raycast.com/ai/ai-commands)** — our
  saved transforms, matured: prompts "you reach for again and again…
  one-press". Ships built-ins (Improve Writing; Fix Spelling and
  Grammar; Explain This in Simple Terms; Change Tone to
  Professional/Friendly; Find Bugs in Code; Summarize Webpage; …).
  Per-command model override; **Output Behavior** chooses where the
  result goes — "Open in Raycast" vs "**Replace Selection**" writes
  the result back in place over the selected text; `{selection}` and
  `{argument}` placeholders; built-ins are duplicable (⌘D) but not
  deletable; tags for organization.
- **Quick Fix** — one hotkey (default double-tap right Shift) runs a
  fix command *in place* in any focused app; a "Prefer Selection"
  option scopes it.
- **[Screen Awareness / Send to AI](https://manual.raycast.com/ai/ai-chat)** —
  "Send Focused Window to AI, Send Selected Text to AI, **Send Screen
  Area to AI**, or Send Screen to AI" push a capture into the
  *active* chat instead of starting a new one; an inactivity timer
  decides reuse vs. new thread.
- **[BYOK](https://manual.raycast.com/ai/bring-your-own-key)** — add
  a key per provider, "click **Verify**. Raycast will check the key
  with the provider"; keys can be toggled disabled without deletion;
  a "Manage in [Provider] Console" button deep-links to the
  provider's key page; a key icon marks models running on your key.
  The fine print is the contrast worth citing: for Anthropic/Google/
  OpenAI keys, "requests are processed through **our servers** in
  order to unify the model APIs… OpenRouter is the exception."

**Adopt:** our Test connection already equals Raycast's Verify (AI
config page) — keep it; add the **provider-console link** and
consider a key-status glyph on the AI surface (Raycast's key icon).
Adopt the **primary-action discipline**: a first-class keyboard
"copy the last answer" (Quick AI's paste-or-copy default) and a
one-press **re-run last transform** (Quick AI's Tab-to-submit is the
step-count benchmark). Steal **Output Behavior** as future config on
a transform: answer-in-panel (today) vs. answer-to-clipboard (the
PopClip end-state). Our dialog already matches Click to Do's
preset+free-prompt shape; Raycast's built-ins-that-can't-be-deleted
is the missing piece.

## 4. Result-presentation models (image AI done in-editor)

**Apple Photos [Clean Up](https://support.apple.com/guide/photos/remove-distractions-and-imperfections-pht5c38b77c5/mac)**
([requirements](https://support.apple.com/en-us/121429)): a toolbar
button *inside the editor*, and — the transferable idea — "After you
click Clean Up, **some items may be highlighted automatically** so you
can quickly click to remove them": the model suggests targets before
the user says anything. Interaction is brush/circle + zoom; Done
commits inside the non-destructive edit session (revert stays
available). **Google Photos** offers both extremes: ["Edit your
photos just by asking… simply describe the edits you want"](https://www.google.com/intl/en_us/photos/editing)
(prompt-first) and [Magic Eraser](https://support.google.com/photos/answer/6128850)
's "tap, draw a circle around, or brush" + Erase/Move (gesture-first).
**Screen Studio** ([screen.studio](https://screen.studio)) is the
polish reference: "an **opinionated** screen recorder" whose good
defaults (auto-zoom, cursor smoothing) are the product, with
shareable presets, and an explicit local-processing claim for its
transcript feature: "All of this happens on your machine, and no
data is sent to our servers."

None of these present as chat. They are modal, in-editor, and
non-destructive — the AI never silently mutates the artifact, and the
commit is an explicit Done/Save. flintshot's v1 posture (answers as
cards in a side panel; canvas never touched) is the right cousin for
text answers; the borrowable bits are the **suggestion-first empty
state** (Clean Up's auto-highlights ≈ our dialog offering likely
transforms) and the **explicit egress/local claim in product copy**
(Screen Studio's one-liner).

**Adopt:** keep answers out of the canvas (validated); add a
one-line egress statement to the AI config page (§6); when the
follow-up pill is empty, surface 1–3 likely next actions (the
suggestion-first pattern) rather than a bare hint string.

## 5. Chat-in-tool mechanics (Raycast AI Chat, Cursor Agent)

[Raycast AI Chat](https://manual.raycast.com/ai/ai-chat) and
[Cursor Agent](https://cursor.com/docs/agent/overview) document the
mechanics table-stakes for a tool-embedded conversation:

| Mechanic | Raycast AI Chat / Quick AI | Cursor Agent | flintshot v1 |
| --- | --- | --- | --- |
| Streaming | "AI Chat streams the response back" | streaming; queue works "while Agent is working" | ✗ (Phase 7) |
| Cancel/stop | empty input cancels a run | Enter steers, Cmd+Enter interrupts | ✗ (Phase 7) |
| Follow-up during flight | **Queue** (default) or **Steer** — "Queue a follow-up to run when it finishes, or Steer to inject your message into the current run without restarting" | "Press Enter to queue your message (it waits until Agent finishes)… Cmd+Enter to send immediately"; steering "delivered at the agent's next tool call instead of cutting off work mid-action" | pill goes inert |
| Regenerate | ⌘R; ⇧⌘R with a different model | — | ✗ |
| Edit a sent message | "Edit any previous message to re-run the conversation from that point" (replaces later turns) | — | ✗ |
| Long messages | Collapsed: "A message over 10 lines… is collapsed" with Show more | — | ✗ (narrow panel!) |
| Code blocks | "code block with the detected language and a **copy button**. Long lines scroll sideways… Click **Enable line wrap**" | inline code/images in chat | markdown, no copy, no wrap toggle |
| Done-attention | Background notification "when streaming finishes and the chat isn't in the foreground… tells you why the chat finished (done, needs confirmation, failed…)" — off by default | — | panel auto-flip (shipped) |
| History | Pin/archive/rename/**Branch Chat**; auto-archive; auto-titles | "Checkpoints… stored locally and separate from Git"; conversation search over a local index | in-memory only, by spec |
| Context overflow | Auto-summarize: "Raycast summarizes the earlier part… in the background" ("Summarizing chat…") | — | n/a (2-turn scale) |
| Images in context | Attachments "scoped to the message you send them with, but the model can keep referring back to them later" | "Read files… supports image files (.png, .jpg, …) and includes them in the conversation context for analysis by vision-capable models" | first turn only (deliberate, v1) |

Cursor's [checkpoints](https://cursor.com/docs/agent/overview) are the
most interesting *idea* for an editor-adjacent tool — snapshot before
a significant change, preview, restore — but they exist to make an
agent that **edits files** safe; our assistant produces read-only
text about an image, so the safety net our users need is undo of
*annotations* (already shipped), not of AI edits.

**Adopt:** while streaming, keep the follow-up pill **enabled** and
make Enter **queue** the message (Raycast's default; Cursor's
Enter-to-queue) — this is a direct amendment to `ai_panel.rs`'s
`inert = is_pending() || is_empty()` and it pairs naturally with
Phase 7's Stop control (Stop must remain reachable while the composer
has focus). Add a copy affordance to assistant cards (Raycast's
per-code-block copy + whole-answer copy) and a line-wrap toggle for
code blocks in the ~236 px panel. Add **regenerate** on the last
assistant/error turn (⌘R where the platform allows, plus a card
glyph). Collapse long cards past ~10 lines (Raycast's threshold)
behind Show more.

## 6. Synthesis — ranked for flintshot's actual UI

### Adopt now (Phase 7 scope or smaller)

1. **Streaming + Stop, with the pill queueing instead of locking.**
   Already planned; the UX amendment is Raycast/Cursor's
   queue-during-flight (§5) and Click to Do's precedent that even a
   *local* SLM "streams back the response" (§2). Vision round-trips
   of 5–30 s with a locked composer (current `inert`) read as
   broken; a queued follow-up reads as fast.
2. **Copy affordances.** Per-card "copy answer" + code-block copy +
   line-wrap toggle (Raycast, both surfaces). Cheap, high-frequency:
   extracted text's destination is almost always elsewhere.
3. **Regenerate / retry from the card.** ⌘R semantics from Raycast;
   on error cards this doubles as the retry button, classified by
   error kind — 401 → "Check your key" + Configure link, 429 → wait
   copy, connection → base-URL hint (Raycast's troubleshooting
   taxonomy: "ran out of credits or hit a provider limit").
4. **Remember last transform + MRU ordering + one-press re-run.**
   Phase 7's remember-transform, extended with the Quick-AI
   step-count lesson (§3) and Snipping Tool's "minimize the number of
   steps" (§2): preselect the last-used transform in the dialog *and*
   bind a direct re-run (e.g. `Shift+I`) that skips the dialog
   entirely.
5. **Built-in starter transforms.** Seed the store with Extract text
   / Summarize / Explain this error / Draft a reply — non-deletable,
   duplicable-to-custom (Raycast AI Commands §3; Click to Do §2).
   The empty-store invitation copy stays for user transforms only.
6. **Collapse long cards** past ~10 lines behind Show more (Raycast
   §5) — our panel is the narrowest chat surface in this study.
7. **Config-page trust copy.** Keep Test connection (= Raycast
   Verify); add the "Manage at provider" console link (Raycast BYOK
   §3) and one egress line — "Requests go directly from flintshot to
   `<base-url>`; nothing in between" — which is *more* direct than
   Raycast's own BYOK (their non-OpenRouter keys proxy through
   Raycast servers, per their manual) and matches Screen Studio's
   local-processing copy (§4) and PopClip's plain billing note (§3).

### Defer (with a trigger)

- **Edit-a-message-and-rerun, Branch Chat** (Raycast §5): needs
  conversation-tree semantics and a persistence story we don't have.
  Trigger: the fuller chatbox spec.
- **Background completion notifications** (Raycast §5): only
  meaningful once a request can outlive the editor window
  (daemon-side AI). We already ship the panel auto-flip, which covers
  the in-window case. Trigger: daemon-hosted requests.
- **Output Behavior per transform** (answer-to-clipboard,
  PopClip-style, §3): trivial once transforms carry per-transform
  config (model override, post-processing toggles). Trigger: prompt
  templating (already on the deferred list).
- **Conversation persistence/search** (Raycast history, Cursor local
  index): currently a deliberate spec boundary — "conversation
  history lives in memory per editor session, not persisted."
  The market normalizes persistence (Raycast, CleanShot's 1-month
  capture history); our NormCap-adjacent constituency (phase7 §7)
  rewards the opposite. Revisit only as explicit opt-in, and only
  worth it paired with the OCR/searchable-history stretch goal.
- **Prompt placeholders** (`{selection}`-style, Raycast §3) and
  **image references in follow-ups** (Raycast/Cursor attachment
  scoping, §5): both already on the roadmap's deferred list; the
  references confirm the eventual shape (attachment scoped to one
  message, visible later in-thread).
- **AI-generated image edits** (Clean Up / Magic Editor, §4):
  roadmap-deferred. When revisited, keep it modal-editor +
  Done-commit + revert (the Apple model), never silent canvas
  mutation.

### Reject (and why, on the record)

- **Cloud AI-as-the-product (Zight, §2).** Requires a server backend
  we don't have, sends screen content to a vendor cloud, and
  conflicts with the BYO-key, no-third-party-traffic boundary
  (ADR-0069, spec "Never" list).
- **A proxy layer for BYO keys (Raycast's model, §3).** flintshot
  has no servers; the direct client-to-provider connection *is* the
  privacy feature. Rejecting this is free.
- **Agent tool-loops inside flintshot** (Cursor Agent's tools, Raycast
  AI Extensions/MCP-client surface, §5). An annotating screenshot
  tool has nothing for an agent to drive; our asymmetry is the
  *server* direction — expose `capture` as an MCP tool (phase7 §5,
  Peekaboo's menu-bar + MCP precedent at
  [github.com/openclaw/Peekaboo](https://github.com/openclaw/Peekaboo),
  whose "Agent runs need a configured model provider" is the same
  BYO-provider stance).
- **Rich web-chat surfaces** (the open-webui/LibreChat class in
  REFERENCES.md): no webview in the vision; egui + markdown cards is
  the ceiling for v1 chat chrome.

## 7. What could not be verified from primary sources

- **CleanShot X full changelog** was not audited entry-by-entry; the
  "no LLM chat" claim is scoped to the official
  [all-features page](https://cleanshot.com/features) as of today.
- **Raycast's exact inline error-card copy** (per HTTP status) is not
  documented publicly — the manual's troubleshooting headings
  ("I ran out of credits or hit a provider limit") confirm the
  *taxonomy* but not the wording. Our error-card copy will be our own
  design either way.
- **Zight key handling** — their AI pages describe a hosted,
  subscription-bundled service; absence of any BYO-key affordance is
  inferred from their own material, not from a statement of policy.
- **Snipping Tool "Text Actions" (2023) quick-redact specifics**
  (auto-redacting emails/phone numbers in a snip): documented in
  Microsoft Q&A answers and secondary coverage, but the original
  Insider announcement post was not retrieved today — §2 therefore
  relies on the 2025 Text Extractor post and the Click to Do page,
  both verified.
- **macOS screenshot-adjacent AI**: Apple ships no AI feature in the
  screenshot path itself; the closest first-party references are
  Photos Clean Up and Writing Tools, so §4 stands in for that
  category.
- **Gemini-image-editing-in-Snipping-Tool style rumors**: no primary
  source found; only Text Actions/Text Extractor (OCR-class) and the
  Click to Do LLM actions are documented by Microsoft.

## What we adapt

| Pattern | Source (first-party) | What exactly we take | Effort |
| --- | --- | --- | --- |
| Queue-don't-lock follow-ups | [Raycast AI Chat](https://manual.raycast.com/ai/ai-chat) · [Cursor Agent](https://cursor.com/docs/agent/overview) | Pill stays enabled while streaming; Enter queues; Stop stays reachable | S (with Phase 7) |
| Copy affordances | [Raycast](https://manual.raycast.com/ai/quick-ai) | Card-level copy-answer; code-block copy + wrap toggle; paste-vs-copy as the primary action | S |
| Regenerate / classified retry | [Raycast](https://manual.raycast.com/ai/quick-ai) (⌘R / ⇧⌘R) | Regenerate glyph on last turn; error cards retry, with 401/429/network-specific copy + Configure deep-link | S |
| Starter transforms, non-deletable + duplicable | [Raycast AI Commands](https://manual.raycast.com/ai/ai-commands) · [Click to Do](https://support.microsoft.com/en-us/windows/ai/ai-features/click-to-do-do-more-with-what-s-on-your-screen) | Seed Extract text / Summarize / Explain error / Draft reply; MRU-first ordering | S |
| Step-minimized re-run | [Quick AI](https://manual.raycast.com/ai/quick-ai) · [Text Extractor](https://blogs.windows.com/windows-insider/2025/04/15/text-extractor-in-snipping-tool-begins-rolling-out-to-windows-insiders/) | Remember last transform (Phase 7) + preselect + a one-press re-run that skips the dialog | S |
| Collapsed long cards | [Raycast](https://manual.raycast.com/ai/ai-chat) | >~10-line assistant cards fold behind Show more | S |
| Config-page trust copy | [Raycast BYOK](https://manual.raycast.com/ai/bring-your-own-key) · [Screen Studio](https://screen.studio) · [Shottr privacy](https://shottr.cc/kb/privacy) | "Manage at provider" console link; egress one-liner (direct-to-endpoint); keep masked Replace-key + Test connection | S |
| Suggestion-first affordances | [Apple Clean Up](https://support.apple.com/guide/photos/remove-distractions-and-imperfections-pht5c38b77c5/mac) · [Click to Do "Ask Copilot"](https://support.microsoft.com/en-us/windows/ai/ai-features/click-to-do-do-more-with-what-s-on-your-screen) | Empty-state offers likely transforms; dialog keeps preset+free-prompt shape (validated) | S |
| Output Behavior / post-processing toggles | [Raycast AI Commands](https://manual.raycast.com/ai/ai-commands) · [Shottr](https://shottr.cc) | Deferred: per-transform answer-to-clipboard; remove-linebreaks-class toggles | M (deferred) |
| History / edit-and-rerun / background notify | [Raycast AI Chat](https://manual.raycast.com/ai/ai-chat) | Deferred with triggers (§6); persistence stays opt-in-at-earliest | L (deferred) |
