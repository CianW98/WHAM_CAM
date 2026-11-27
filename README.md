# WHAM_CAM

A simple 2.5D CAM program for CNC routers and mills. Import or draw 2D geometry, apply machining
operations (profile, pocket, drill), preview the toolpaths and output standard G-code.

It keeps the common workflow quick and plain: one main screen with the drawing area, an
object/operation tree and a properties panel, and every setting visible and editable.
WHAM_CAM runs on Windows and Linux, works fully offline, and is not tied to any one machine or controller.

**Status: early development.** WHAM_CAM can:
- import DXF files and organise them into layers;
- draw lines, arcs, circles, rectangles, polylines and points with snapping, with one-key
  shortcuts (L, A, C, R, P and `.`);
- move, rotate, scale, mirror, cut, copy and paste;
- join, explode, break at intersections and close polylines;
- save and open projects (`.wc` files);
- keep a **master tool list** and **tool libraries** on your computer (tool numbers per library,
  e.g. for a tool changer), and give tools to the whole job, a part or a single operation;
- make **profile** (inside, outside, on line), **pocket** (with islands) and **drill**
  (standard, peck, helical) toolpaths with end mills, with every toolpath checked by a safety
  checker;
- preview toolpaths in 2D and 3D, show the cutter width, and play the real tool back along them.

G-code output is next, so it can't produce G-code yet.
See `docs/spec.md` for the v1 plan and `docs/roadmap.md` for ideas beyond it.

## Download

Installers for Windows and Linux are on the [Releases](https://github.com/CianW98/WHAM_CAM/releases) page.

## Building from source

You need Rust and Node.js. On Linux, Tauri also needs these system libraries:
```
sudo apt install libwebkit2gtk-4.1-dev libsoup-3.0-dev librsvg2-dev build-essential libssl-dev libayatana-appindicator3-dev
```
For the engine's WebAssembly check: `rustup target add wasm32-unknown-unknown`

- Setup: `cd app && npm install`
- Run in development mode: `cd app && npm run tauri dev`
- Engine tests: `cargo test -p wham_engine`

- Lint: `cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings`
- Interface type check and build: `cd app && npm run build`

## Built with

- [Tauri](https://tauri.app) (desktop app), [React](https://react.dev) and TypeScript (interface)
- [three.js](https://threejs.org) (drawing area, MIT licence)
- [cavalier_contours](https://github.com/jbuckmccready/cavalier_contours) (offsetting for
  toolpaths, MIT / Apache-2.0)
- [serde](https://serde.rs) and serde_json (project files, MIT / Apache-2.0)

## Licence

Apache License 2.0.
The logo is not covered by this licence: see `NOTICE`.
