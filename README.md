# WHAM_CAM

A simple 2.5D CAM program for CNC routers and mills. Import or draw 2D geometry, apply machining
operations (profile, pocket, drill), preview the toolpaths and output standard G-code.

It keeps the common workflow quick and plain: one main screen with the drawing area, an
object/operation tree and a properties panel, and every setting visible and editable.
WHAM_CAM runs on Windows and Linux, works fully offline, and is not tied to any one machine or controller.

**Status: early development.** WHAM_CAM can import DXF files and organise them into layers.
Drawing tools, toolpaths and G-code output are still being built, so it can't produce G-code yet.
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

## Licence

Apache License 2.0.
