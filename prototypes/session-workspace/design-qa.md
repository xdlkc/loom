# Loom session workspace — design QA

final result: passed

## Comparison target and evidence

- Selected visual truth: `public/qa/reference.png`, the first displayed generated design selected by the user.
- Source pixels: 1487 × 1058. Implementation CSS viewport and screenshot: 1487 × 1058 at 1 CSS pixel per image pixel; screenshot bytes are JPEG despite the `.png` capture filenames.
- State: default `config` demo session, conversation view, first model request selected, request-details inspector active, tool result expanded.
- Initial implementation: `qa/desktop-initial.png`.
- Final implementation: `qa/desktop-iteration-2.png`.
- Full-view comparison: `qa/comparison-2.png`. Both source and implementation are displayed together in the same browser capture at 1190 × 846.7 each, using the same 0.8003 scale; comparison viewport is 2380 × 880. There is no density or frame mismatch.
- Focused comparison: `qa/focused-comparison-2.png`. Both step regions are shown side by side at native pixel density, crop origin (395, 240), 700 × 600 per region, in a 1400 × 635 viewport. This makes the numbered spine, icons, timing bar, file row, code typography and spacing readable in the same comparison input.
- Other browser-rendered states: `qa/timeline.png`, `qa/error-state.png`, `qa/mobile-inspector.png`.

## Comparison history

### Initial review — blocked pending visual fixes

- P1, step header: the number marker overlapped the model/tool icon because the absolutely positioned number did not reserve layout space. Fix: reserve a 60px icon offset in desktop step headers; retain the separate narrow-screen layout.
- P2, vertical rhythm: the user-message header and bubble introduced excessive spacing, moving the selected model call below its reference position. Fix: reduce the initial top padding and message-header gap, adjust bubble padding and the following gap. The selected call now starts at the reference's approximately 247px vertical position.
- P2, reading hierarchy: the first implementation used small timing/metadata text and left the request timing bar aligned to the disclosure icon rather than its label. Fix: increase key desktop text sizes and align the bar to the request label; restore sidebar search/list spacing and inspector column proportions.

### Final review — passed

The full-view and focused comparisons show the fixes applied. The four main tracks, independent panes, selected-step tint, user and assistant messages, paired file result, and right-hand inspector follow the selected visual structure. No actionable P0/P1/P2 mismatch remains for this prototype scope.

## Required fidelity surfaces

1. **Fonts and typography:** locally bundled Inter Variable and JetBrains Mono, with platform CJK fallback. Bold headings, quieter timestamps, monospace values and code follow the reference hierarchy. No clipped primary labels at the reference viewport. Minor differences in the generated reference's CJK weight remain P3.
2. **Spacing and layout:** 66px icon rail, 313px session list, flexible execution pane and 380px inspector match the source's principal grid. Header is 67px; execution tabs are 55px. Main spine, selected container, file row and inspector alignment were checked at the same viewport and state.
3. **Colors and tokens:** deep forest green, pale green selected surfaces, neutral white/gray, muted slate secondary text and blue model icons match the reference's palette. Failure states add a consistent text-and-icon red treatment; status is not communicated by color alone.
4. **Image and asset fidelity:** the visual target has no photos or raster illustrations. All icons, including the Command brand mark, use the supplied Phosphor icon library rather than handcrafted SVG/CSS/glyph substitutes. Standard HTML timing bars encode data, not decorative image assets. Fonts are local and sharp at native density.
5. **Copy and content:** Chinese labels, primary task, file contents and model/agent identifiers follow the visual target. Intentional semantic corrections: the inspector shows per-request usage (774 input, 12 output) rather than duplicating the whole session's totals; timing segment ratios are calculated from the labels; only the selected step has a filled number marker. Sidebar demo scenarios use a common three-step structure. Tool durations remain unknown, with no inferred elapsed span.

## Browser interaction verification

- Session search, empty results and clearing filters.
- Agent/status filters, including a single failed-session result.
- Switching sessions updates the task, model, status, steps and inspector.
- Per-session step selection is restored when returning to a session.
- Selecting a conversation step updates the inspector.
- Switching to the time axis preserves selection; selecting a waterfall row updates the same inspector.
- Returning to conversation preserves the selected step and scrolls it into view.
- Previous/next step navigation, including disabled end controls.
- Tool-result collapse and expansion; paired arguments and result remain linked.
- Request, output and raw-event tabs; selected error details are visible in the failed session.
- Copy of a selected raw event checked against the demo step ID.
- New demo session creation, focus remaining in its dialog during keyboard navigation, and Escape close.
- Arrow-key navigation between view tabs.
- Narrow viewport: 390 × 844, document scrollWidth equals 390. Session list and step inspector open as drawers; step viewing, closing and changing sessions work without horizontal page overflow.
- Browser console: no error-level entries during verification.
- `npm run build` and the four provided Sites packaging tests passed. No external deployment was performed.

## Limits and follow-up polish

- This is a frontend interaction prototype using synthetic data. It does not read credentials, call models or change the running Loom/quota services.
- Full screen-reader and assistive-technology certification is outside this check. Native desktop focus indicators, semantic labels, keyboard tab behavior and reduced-motion preference were considered; a full accessibility audit remains future work.
- P3: refine CJK weight and the smallest secondary labels after the user reviews the working prototype.
- P3: longer real sessions, true streaming updates, backend normalization and production persistence are not part of this demo.

## Implementation checklist

- [x] Selected visual faithfully translated into a working local prototype.
- [x] Main session / conversation / timeline / inspector journey verified in-browser.
- [x] Initial visual issues fixed and captured again.
- [x] Source and implementation compared together in full and focused views.
- [x] Build and packaging checks pass.
- [x] Prototype kept running locally for handoff.
