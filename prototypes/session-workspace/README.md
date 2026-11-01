# Loom session workspace prototype

An isolated frontend prototype of the user's selected first design direction. It includes linked session navigation, conversation replay, a timing waterfall, and selected-step details. All data is synthetic and stays in browser memory. No model requests or credential reads are made.

The current local preview runs at http://127.0.0.1:4333/.

```sh
npm install
npm run dev -- --host 127.0.0.1 --port 4333 --strictPort
```

Use session search or the filter control to find demo sessions. Select a numbered step in either view; the inspector and other view share that selection. Keyboard: `/` focuses search; arrow keys switch tabs; Alt+Up / Alt+Down move between steps; Escape closes overlays.

`npm run build` creates the packaged client. `npm run test:sites` checks the provided static hosting worker. See `design-qa.md` and `qa/` for the browser verification and visual comparison evidence.
