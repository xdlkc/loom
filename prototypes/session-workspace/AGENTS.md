# Prototype Instructions

## Selected Loom direction

- The user selected the first displayed visual direction, archived at `public/qa/reference.png`.
- Preserve the light neutral palette, forest-green selected state, slim icon rail, session list, conversation/timeline canvas, and persistent step inspector.
- Conversation selection, timeline selection and inspector navigation must share the same selected step. Keep per-session selection when switching sessions.
- Tool calls and results stay paired. Unrecorded tool durations remain unknown.
- This is an isolated frontend prototype using demo data. Do not connect it to real model providers or read credentials without a separate user request.

Run the local server yourself and open the preview in the browser available to this environment. Do not give the user server-start instructions when you can run it.

Before making substantial visual changes, use the Product Design plugin's `get-context` skill when the visual source is unclear or no longer matches the current goal. When the user gives durable prototype-specific design feedback, preferences, or decisions, record them in `AGENTS.md`.

When implementing from a selected generated mock, treat that image as the source of truth for layout, component anatomy, density, spacing, color, typography, visible content, and hierarchy.

Build app UI in `src/`. Keep `.openai/hosting.json`, `worker/index.js`, `scripts/prepare-sites-build.mjs`, and `tests/sites-worker.test.mjs` intact so the same local prototype can be handed to Sites. Before a Sites handoff, run `npm run build` and `npm run test:sites`; the build must leave `dist/client/index.html`, `dist/server/index.js`, and `dist/.openai/hosting.json`.
