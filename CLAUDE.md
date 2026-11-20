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
  and used, respecting their licences. Check a licence before copying or linking code. Ask before
  committing anything from other open-source projects, to verify the licence matches our requirements.

## G-code output and machines
- WHAM_CAM is not tied to one machine or controller.
- v1 outputs standard, widely compatible G-code through one generic post-processor.
- Machine-specific post-processors (e.g. RepRapFirmware, Grbl, LinuxCNC, Mach3), each with its
  own settings panel per machine type, come after v1. Don't add machine-specific G-code before then.

## Safety: G-code is safety-critical
N.B. This program drives a real machine. A bug can crash the spindle, break tools or ruin parts.
- Never change toolpath or post-processor logic without tests that compare output
  against known-good G-code (checked files in `engine/tests/reference_gcode/`).
- Every operation must respect safe Z / clearance height for rapid moves.
- No rapid (G0) moves below the clearance height except the final approach.
- Output must be plain, readable and commented (operation name, tool, depths).
- The user checks new output in a simulator (CAMotics or NC Viewer) and with an air cut
  before cutting material. Remind them when toolpath logic has changed.

## Version 1 scope
1. Import DXF (and SVG)
2. Basic 2D drawing: lines, arcs, circles, rectangles, polylines.
3. Basic dimensioning, transforming, joining and splitting of geometry, aligned to a grid
4. Operations: profile (inside / outside / on line), pocket, drill
5. Tool library: master tool list and tool libraries (feeds and speeds stay on the operation in v1)
6. Toolpath preview
7. G-code output through a post processor
8. Save and open project files
Anything not on this list waits until v1 works end to end. Note such ideas in `docs/roadmap.md`.

## Stack
- Tauri desktop app: Rust core with a web-based (HTML/CSS/TypeScript) interface.
- Front-end framework: React + TypeScript.
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
- Point and arc maths (bulge ↔ arc, arc sweep across 0°, angle normalising, point helpers) lives
  once in `engine/src/geometry.rs` with its own tests (`engine/tests/arc_maths.rs`). Use those
  helpers; don't write another copy (copies drifting apart caused the arc-across-0° display bug).

## Future option: browser version via WebAssembly
Not in v1 scope, but keep the door open. A later goal is a browser build: the same web interface
plus the engine compiled to WebAssembly, served as static files, with all CAM work running on the
user's device (no server-side processing). To keep this possible:
- The engine must stay WebAssembly-compatible. No direct file system access, OS-specific code or native-only threading inside the engine: it takes data in and returns data. File opening/saving and machine communication live in the app layer (Tauri), not the engine.
- The interface talks to the engine through one thin connection layer (`app/src/engine/index.ts`), not calls scattered through the UI, so that layer can be swapped between Tauri and a WebAssembly build.
- Before adding a Rust library to the engine, check it compiles for wasm32-unknown-unknown. Ask before adding one that doesn't.
- CI builds the engine for WebAssembly on every push, so a break is caught straight away.

## Libraries that shape toolpaths (e.g. cavalier_contours for offsetting)
Agreed 2026-11-16 for any engine library whose output affects toolpaths. cavalier_contours does
the offsetting (trialled on the `offsetting` branch, adopted and merged into `main` 2026-11-17).
- **Pin the exact version** in `engine/Cargo.toml` (e.g. `"=0.9.0"`), not just via the lock file.
  New releases are never picked up automatically, not even bug-fix releases.
- **One adapter file.** Only one engine file (e.g. `engine/src/offset.rs`) calls the library. The
  rest of the engine asks it for results in our own types. An update or a library swap touches
  only that file.
- **Every workaround lives in the adapter**, with a comment naming the library issue (e.g. arcs
  over 180° split in two: cavalier_contours issue #15).
- **Each workaround has two tests:**
  - A **canary** that checks the library directly and passes while its problem still exists,
    named after the issue.
  - **Our own test** that checks the right answer through the adapter.
  - When an update fixes the problem, the canary fails: the workaround can then be removed (or kept
    if harmless).
- **Updating** (only when the user asks):
  1. Read the library's changelog.
  2. Work on a branch.
  3. Bump the pin and run everything: engine tests, canaries, the reference toolpath comparisons
     and the safety checker.
  4. Remove workarounds whose canaries now fail.
  5. If any toolpath output changed, it counts as a toolpath logic change: the release notes get a
     "G-code output changed" section, and the user re-checks in a simulator and with an air cut.
  6. Merge when the user is happy.
- **If the library stops being maintained:** its licence lets us keep our own patched copy and
  point the engine at it (a Cargo `[patch]`). Ask before doing this.
- **Library errors and crashes never crash WHAM_CAM.** The adapter turns a library panic into an
  error message, and every resulting toolpath still goes through the safety checker.

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
- CI (`.github/workflows/ci.yml`) runs the engine tests on Ubuntu and Windows, lint and WASM build,
  the interface build, and an app check on Ubuntu and Windows.
- Release workflow (`.github/workflows/release.yml`) runs when a `vX.Y.Z` tag is pushed.

## Workflow
- Core v1 features are committed straight to `main` (user's decision, 2026-11-15). Use a branch
  for experimental or post-v1 work, and ask if unsure.
- Trials of new libraries go on a branch (user's decision, 2026-11-16), e.g. `offsetting` for the
  offsetting library: if the library doesn't work out, `main` never carried it. When the user is
  happy, merge into `main` locally and push (no pull request needed). CI runs on every branch.
- Commit small and often, with clear messages.
- Never commit secrets. Keys and credentials go in `.env`, which is git-ignored.
- Tasks are tracked in GitHub Issues. Use the `gh` CLI to read and update them.
- Ideas beyond v1 go in `docs/roadmap.md` (with reasoning and any decisions), not the spec.
- At the end of a session, update the Status section below, any decisions above, and
  `docs/roadmap.md` if new ideas came up.

## Releases
- **Version numbers** use semantic versioning, `MAJOR.MINOR.PATCH`. While building towards v1,
  MAJOR stays 0: a release that adds features bumps MINOR (0.2.0), a release with only fixes
  bumps PATCH (0.2.1). 1.0.0 = the v1 scope works end to end. After that, MAJOR goes up only for
  breaking changes (e.g. old project files no longer open).
- **One version everywhere:** `engine/Cargo.toml`, `app/src-tauri/Cargo.toml`, `app/package.json`
  and `app/src-tauri/tauri.conf.json` must always carry the same version. A release bumps all
  four in one commit ("Release vX.Y.Z").
- **Releasing** (only when the user asks):
  1. All CI checks pass on `main`.
  2. Bump the version in the four files.
  3. Write the release notes from the commit history since the last tag, in plain English,
     grouped as New / Changed / Fixed.
  4. Tag the commit `vX.Y.Z` and push the tag. The release workflow checks the tag matches the
     four version numbers, runs the engine tests on Linux and Windows, builds the installers
     (Windows `.msi`/`.exe`, Linux `.deb`/`.rpm`/`.AppImage`) and creates a draft GitHub Release
     with them attached.
  5. Add the release notes to the draft, let the user look, then publish it, making sure it
     stays on the `vX.Y.Z` tag.
     Releases are normal releases, not pre-releases (0.x already says unfinished).
     Installers are unsigned for now (Windows warns "unknown publisher").
- **Safety in releases:** if a release changes toolpath or G-code output, the notes say so in a
  separate "G-code output changed" section, saying what changed and reminding the user to
  re-check in a simulator and with an air cut. Every release is checked against the
  checked reference G-code (`engine/tests/reference_gcode/`) before tagging.
- **Traceability:** once G-code output exists, every G-code file's header comment includes the
  WHAM_CAM version that produced it, e.g. `( WHAM_CAM 0.4.0 )`.

## Status
Done:
- Spec: `docs/spec.md` (machine-neutral; open questions in section 9).
- Skeleton: engine crate, Tauri + React window with toolbar, tree, properties, three.js drawing
  area (2D pan/zoom, 3D orbit), pop-up notifications, CI on Linux and Windows.
- DXF import: engine DXF reader (lines, arcs, circles, polylines, points, blocks), unit prompt for
  inch DXFs, click/box selection, delete, undo/redo. Splines/ellipses become polylines of arcs
  and lines on import (`curve.rs`, biarcs; tolerance dialog, 0.01 mm default; decided 2026-11-20).
- Layers: delete, colours (own preset picker), rename in the tree or Properties, open/close in the tree.
- Reference examples (Drill, Pocket, Profile) are in `tests/reference/local/`. The whole top-level
  `tests/` folder is git-ignored for now (this machine only), so the expected moves from the
  references are written into the engine tests (`engine/tests/pocket.rs`, `profile.rs`, `drill.rs`).
- Release 0.1.0 published (2026-11-15): DXF import, layers, selection, undo/redo. No G-code yet.
- Release 0.2.0 published (2026-11-16): drawing tools, snapping, right-click menu, transforms,
  clipboard, PolyRectangle, points pop-up. No G-code yet.
- Release workflow: pushing a `vX.Y.Z` tag checks versions, runs engine tests (Linux + Windows) and
  builds Windows/Linux installers into a draft release. CI also runs engine tests on Windows.
- Roadmap for post-v1 ideas: `docs/roadmap.md`. Build setup moved from the spec to the README.
- Drawing tools (spec 5.2): line, arc (start, end, centre; CCW), circle, rectangle, polyline with
  tangent arcs (A/L), point list; snapping (grid, end, mid, centre) in the engine (`draw.rs`);
  current layer; finished shape is selected; editable shape values and per-project grid spacing.
  (Typed-coordinate box removed at the user's request: unintuitive.)
- Right-click menu (View, Draw, Transform, Edit, Select All / On Layer, Cut/Copy/Paste,
  Undo/Redo). Transforms (move, rotate, scale, mirror; move and rotate also by clicking) around
  the selection centre in the engine (`transform.rs`); app clipboard with click-to-place paste.
- PolyRectangle shape (lower left, width, height, rotation). Properties: `x,y` point boxes,
  points pop-up for polylines/point lists, direction (CW/CCW). The user wants our own look,
  not a copy of the reference program's (2026-11-16).

- Menu bar (File, View, Edit, Draw, Machining) sharing its commands with the right-click menu.
- Edit tools (spec 5.3) in the engine (`edit.rs`): Join (tolerance dialog), Explode, Break at
  intersections, Close polyline.
- Arrow keys step the selection; layer right-click menu (New layer, Rename, Delete layer);
  zoom to fit only on request or after a DXF import. User checked the app (2026-11-16): good.
- Project files (spec 5.8): `.whamcam` JSON with a format version, in the engine
  (`project_file.rs`; serde_json is now an engine dependency). New/Open/Save/Save As, unsaved
  changes prompt, title with `*`, safe write via temp file + rename in the app layer.
- Cleanup before toolpaths: the duplicated point/arc maths gathered into shared helpers in
  `geometry.rs` (no behaviour change; all tests pass).

- Toolpaths (merged from the `offsetting` branch into `main`, released in 0.3.0, 2026-11-17):
  cavalier_contours =0.9.0 adopted, only in `engine/src/offset.rs` (arcs over 180° split, canary
  test for issue #15). Operations model (`operation.rs`), toolpath moves (`toolpath.rs`), safety
  checker (`safety.rs`; rule 5 is "arcs never go up"), shared move building (`path.rs`).
  - Pocket (`pocket.rs`): matches the reference Pocket example move for move. Stepovers over 0.5
    can leave slivers in narrow parts (documented; fix on the roadmap).
  - Profile (`profile.rs`): Inside/Outside/On line, open shapes On line only, no overlap,
    nearest-next shape order, straight plunge; matches the reference Profile loop move for move
    (its spiral lead-in is not in v1).
  - Drill (`drill.rs`): Standard, Peck, Spiral (helix; circles cut to their own diameter; holes
    over 2× the tool exit straight up from the wall), point lists and circle/arc centres,
    nearest next; matches the reference Drill move for move.
  - Generate toolpaths on the button with out-of-date markers; preview in 2D/3D; Show toolpaths
    and Show cut width (shaded cutter band from the engine) toggles; tool playback (cutter in
    2D/3D, times at the feed rates, rapids assumed 5000 mm/min).
  - Operations renamed by double-click in the tree; drawing tool keys L, A, C, R, P and `.`.
- Release 0.3.0 published (2026-11-17): Profile, Pocket and Drill toolpaths with preview and playback. No
  G-code yet.

- Tooling (spec 5.5, decided 2026-11-19), released in 0.4.0:
  - Master tool list and tool libraries belong to the computer (`engine/src/tooling.rs`; file
    `tooling.json` in the app's settings folder, read/written by the app layer). Tools: end type,
    flute type + helix angle (not drills), diameters, lengths, automatic names, live side view.
    Tools hold no feeds/speeds (material-dependent: materials library later). Only end mills
    (flat end) can cut in v1.
  - Left panel tabs Project | Setup. **The Setup tab is global**: it configures computer-wide
    things only, never changes a project and shows no project information.
  - Tools follow the Machining tree: Machining (library + tool) → part (tool override) →
    operation (Custom tool, or a library tool), resolved by the engine before generating
    (`Project::resolve`). Every picked tool is copied into the project.
  - One tool number = one cutter: custom tools avoid library numbers and other tools' numbers;
    Generate refuses clashes and compares each tool with the library on this computer (warns if
    the project's library isn't on this computer).
  - 3D playback cutter modelled from the real tool outline (`Tool::profile`).
- Checked reference G-code: `engine/tests/reference_gcode/` (on GitHub, with a README: test data,
  not for cutting; decided 2026-11-20), compared by `engine/tests/reference_gcode.rs`.
- Release 0.4.0 (2026-11-19): tooling (master tool list, libraries, Setup tab, tools through the
  Machining tree, tool number checks, real 3D cutter). No G-code yet.

- G-code output (spec 5.7, 7), merged into `main` 2026-11-20, not released yet: generic
  post-processor (`post.rs`), read-back check (`gcode_read.rs`), drill holes as G81/G83 cycles
  (Peck now follows G83), machine list in the Setup tab, Produce G-code. Arc test shape (square
  corners, convex, concave) checks G2/G3. CAMotics and NC Viewer don't simulate `G83`.

- Dimensions (spec 5.3, 2026-11-20): aligned, radius, diameter, angle (`dimension.rs`), measure
  only; one smart Dimension tool (key D) picks the kind from what is clicked; format 2.

Next, towards 1.0.0: DXF dimensions, then SVG import and DXF text (each needs a library choice
first) (spec 5.1), then a full end-to-end check by the user.
No 0.x releases before 1.0.0 (user's decision, 2026-11-20; no 0.5.0). The 1.0.0 notes need a
"G-code output changed" section: G-code is new and Peck toolpaths changed (Peck checked by the
user, 2026-11-20).
