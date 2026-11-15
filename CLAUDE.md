# CLAUDE.md

## Project
A modern, simple 2.5D CAM program in the spirit of the reference program. Draw or import 2D geometry,
apply machining operations, preview the toolpaths, output G-code.

Project name: WHAM_CAM

## Design principle: simple
The reference program is still worth using because it is not overcomplicated. This project keeps that.
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
- The user is an experienced the reference program user and CNC machinist. Trust their judgement on
  machining behaviour, and ask them when CAM behaviour is unclear rather than guessing.

## Reference material
- The reference program is the behavioural reference. Study how it behaves and what it outputs.
  Do NOT decompile, disassemble or read the reference program's code or binaries (clean room only).
- The reference program's user guide is the main source for features and settings.
- Open-source CAM projects (FreeCAD CAM, Kiri:Moto, FabexCNC, OpenCAMLib) may be studied
  and used,respecting their licences. Check a licence before copying or linking code. Ask before commiting anything from other opensource projects to verify the license matches our requirements.

## Initial target machine (first post-processor)
- WorkBee CNC router running a Duet 2 with RepRapFirmware (RRF).
- G-code must suit RRF. Known issue: G64 is not supported on this setup, so do not emit it.
- Other controllers (Grbl, LinuxCNC, Mach3) are later post-processors, not v1.

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
TBD. Add exact commands to run, test and build once the stack is chosen.

## Workflow
- After initial build, never commit straight to `main`. Use a branch per feature and merge when it works.
- Commit small and often, with clear messages.
- Never commit secrets. Keys and credentials go in `.env`, which is git-ignored.
- Tasks are tracked in GitHub Issues. Use the `gh` CLI to read and update them.
- At the end of a session, update the Status section below and any decisions above.

## Status
- Stack chosen (see Stack). Draft spec written: `docs/spec.md`, which needs user review. Its open
  questions (section 9) cover G64, drilling on the WorkBee, tool change and profile overlap.
- The reference program reference examples (Drill, Pocket, Profile) are in `tests/reference/local/`.
  They are git-ignored, so they live on this machine only.
- Next: agree the spec, then build the skeleton app (engine crate + Tauri/React shell + CI).
