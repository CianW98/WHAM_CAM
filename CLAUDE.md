# CLAUDE.md

## Project
A modern, simple 2.5D CAM program in the spirit of the classic simple 2.5D CAM programs. Draw or import 2D geometry,
apply machining operations, preview the toolpaths, output G-code.

Project name: WHAM_CAM

## Design principle: simple
The best simple CAM programs are still worth using because they are not overcomplicated. This project keeps that.
- Geometry first, operations second: select shapes, then apply profile / pocket / drill etc.
- One main screen: drawing area, object/operation tree, properties panel.
- Every setting visible and editable. No hidden wizards, no 3D modelling, no parametric history.
- If a feature makes the common workflow slower or more confusing, it does not go in.
  Push back and ask before adding complexity.

## About the user
- The user is not a programmer. Explain changes in plain English: what changed, why, and
  whether anything else was affected.
- Ask before large or structural changes, new dependencies, or changing the stack.
- Never delete or rewrite working features without asking.
- The user is an experienced CAM user and CNC machinist. Trust their judgement on
  machining behaviour, and ask them when CAM behaviour is unclear rather than guessing.

## Reference material
- An established simple 2.5D CAM program is the behavioural reference; the user supplies example
  output from it. Study how it behaves and what it outputs.
  Do NOT decompile, disassemble or read any other CAM program's code or binaries (clean room only).
- Published user documentation of such programs is the main source for features and settings.
- Open-source CAM projects (FreeCAD CAM, Kiri:Moto, FabexCNC, OpenCAMLib) may be studied
  and used,respecting their licences. Check a licence before copying or linking code. Ask before commiting anything from other opensource projects to verify the license matches our requirements.

## G-code output and machines
- WHAM_CAM is not tied to one machine or controller.
- v1 outputs standard, widely compatible G-code through one generic post-processor.
- Machine-specific post-processors (e.g. RepRapFirmware, Grbl, LinuxCNC, Mach3), each with its
  own settings panel per machine type, come after v1. Don't add machine-specific G-code before then.

## Safety: G-code is safety-critical
N.B. This program drives a real machine. A bug can crash the spindle, break tools or ruin parts.
- Never change toolpath or post-processor logic without tests that compare output
  against known-good G-code (reference files in `tests/reference/`).
- Every operation must respect safe Z / clearance height for rapid moves.
- No rapid (G0) moves below the clearance height except the final approach.
- Output must be plain, readable and commented (operation name, tool, depths).
- The user checks new output in a simulator (CAMotics or NC Viewer) and with an air cut
  before cutting material. Remind them when toolpath logic has changed.

## Version 1 scope
1. Import DXF (and SVG)
2. Basic 2D drawing: lines, arcs, circles, rectangles, polylines.
3. Basic Dimensioning, Transforming, Joining and splitting of geometries etc. aligned to a grid
3. Operations: profile (inside / outside / on line), pocket, drill
4. Tool library: diameter, flutes, feeds and speeds
5. Toolpath preview
6. G-code output through a post processor
7. Save and open project files
Anything not on this list waits until v1 works end to end.

## Stack
- Tauri desktop app: Rust core with a web-based (HTML/CSS/TypeScript) interface.
- Front-end framework: React + TypeScript (decided 2026-11-15).
- Drawing area: WebGL via three.js (MIT), with a 2D top view and a 3D view. Used directly
  from one React component; it only draws what the engine sends.
- Units: mm only in v1. Inches come later, with CAM styles and workspaces.
- Lead-ins/lead-outs: not in v1 (straight plunge at the plunge feed rate).
- Requirements: runs on Windows and Linux desktop, works fully offline, fast enough to regenerate toolpaths interactively.

## Architecture: keep the CAM engine separate from the interface
- The engine (Rust) owns everything that affects the machine: DXF/SVG import, geometry, toolpath generation, tool library, post-processors and G-code output.
- The engine is its own Rust crate/library with its own tests. It must build and pass its tests without the UI.
- The interface only displays things and passes settings to the engine. No toolpath maths, geometry calculations or G-code generation in the front-end code.
- UI changes must never change G-code output. If a UI change needs an engine change, stop and explain it before doing it.

## Future option: browser version via WebAssembly

- Not in v1 scope, but keep the door open. A later goal is a browser build: the same web interface plus the engine compiled to WebAssembly, served as static files, with all CAM work running on the user's device (no server-side processing). To keep this possible:

- The engine must stay WebAssembly-compatible. No direct file system access, OS-specific code or native-only threading inside the engine: it takes data in and returns data. File opening/saving and machine communication live in the app layer (Tauri), not the engine.
- The interface talks to the engine through one thin connection layer, not calls scattered through the UI, so that layer can be swapped between Tauri and a WebAssembly build.
Before adding a Rust library to the engine, check it compiles for wasm32-unknown-unknown. Ask before adding one that doesn't.
- Add a CI check that builds the engine for WebAssembly once the engine exists.


## Commands
Layout: `engine/` (Rust CAM engine, crate `wham_engine`), `app/` (React + TypeScript interface),
`app/src-tauri/` (Tauri app layer, crate `wham_cam_app`). `app/src/engine/index.ts` is the only
interface file that talks to the engine.

- First-time setup: `cd app && npm install`
- Run the app (dev mode, reloads on change): `cd app && npm run tauri dev`
- Engine tests: `cargo test -p wham_engine`
- Engine WebAssembly check: `cargo build -p wham_engine --target wasm32-unknown-unknown`
- Lint/format: `cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings`
- Interface type check + build: `cd app && npm run build`
- Release build (installer): `cd app && npm run tauri build`
- CI (`.github/workflows/ci.yml`) runs the engine tests, lint and WASM build, the interface build,
  and an app check on Ubuntu and Windows.

## Workflow
- Core v1 features are committed straight to `main` (user's decision, 2026-11-15). Use a branch
  for experimental or post-v1 work, and ask if unsure.
- Commit small and often, with clear messages.
- Never commit secrets. Keys and credentials go in `.env`, which is git-ignored.
- Tasks are tracked in GitHub Issues. Use the `gh` CLI to read and update them.
- At the end of a session, update the Status section below and any decisions above.

## Status
- Stack chosen (see Stack). Draft spec written: `docs/spec.md`, revised to be machine-neutral
  (generic G-code; toolbar and pop-up notifications in the layout). Open questions in section 9.
- Reference examples (Drill, Pocket, Profile) are in `tests/reference/local/`.
  They are git-ignored, so they live on this machine only.
- Skeleton app built (2026-11-15): engine crate with a demo document, Tauri + React window with
  toolbar, tree, properties, three.js drawing area (2D pan/zoom, 3D orbit) and pop-up notifications. CI added.
  Toolbar buttons are placeholders. No toolpaths or G-code yet.
- DXF import done (2026-11-15): engine DXF reader (lines, arcs, circles, polylines, points,
  exact splines/ellipses, blocks), inch prompt, selection in the drawing (click/box), delete,
  undo/redo. Demo drawing removed. File dialog via tauri-plugin-dialog.
- Layers (2026-11-15): delete layer, colours (drawn in layer colour, own preset picker),
  rename in tree (double-click) or Properties; click a layer to open/close it.
- Next: drawing tools (Part 2: click + snap, typed coordinates, editable properties), then
  the offsetting library (candidate: cavalier_contours) and toolpaths.
