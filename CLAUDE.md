# CLAUDE.md

## Project
A modern, simple 2.5D CAM program with its own look and feel. Draw or import 2D geometry,
apply machining operations, preview the toolpaths, output G-code.

Project name: WHAM_CAM

## Design principle: simple
WHAM_CAM stays simple and is not overcomplicated.
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
- The user supplies reference examples (`tests/reference/local/`) to check toolpaths against.
- Published documentation (controllers, G-code, CAM programs) may be studied.
  Do NOT decompile, disassemble or read any other CAM program's code or binaries.
- Open-source CAM projects (FreeCAD CAM, Kiri:Moto, FabexCNC, OpenCAMLib) may be studied
  and used, respecting their licences. Check a licence before copying or linking code. Ask before
  committing anything from other open-source projects, to verify the licence matches our requirements.

## Safety: G-code is safety-critical
N.B. This program drives a real machine. A bug can crash the spindle, break tools or ruin parts.
- Never change toolpath or post-processor logic without tests that compare output
  against known-good G-code (checked files in `engine/tests/reference_gcode/`).
- Every operation must respect safe Z / clearance height for rapid moves.
- No rapid (G0) moves below the clearance height except the final approach.
- Output must be plain, readable and commented (operation name, tool, depths).
- No machine-specific G-code outside a post-processor.
- The user checks new output in a simulator and with an air cut
  before cutting material. Remind them when toolpath logic has changed.

## Scope
What WHAM_CAM does is in `docs/spec.md`; what is planned is in `docs/roadmap.md`. Don't add a
feature that is in neither. Note new ideas in `docs/roadmap.md` and ask first.

## Stack
- Tauri desktop app: Rust core with a web-based (HTML/CSS/TypeScript) interface.
- Front-end framework: React + TypeScript.
- Drawing area: WebGL via three.js (MIT), with a 2D top view and a 3D view. Used directly
  from one React component; it only draws what the engine sends.
- Units: mm only.
- Requirements: runs on Windows and Linux desktop, works fully offline, fast enough to regenerate toolpaths interactively.

## Architecture: keep the CAM engine separate from the interface
- The engine (Rust) owns everything that affects the machine: DXF/SVG import, geometry, toolpath generation, tool library, post-processors and G-code output.
- The engine is its own Rust crate/library with its own tests. It must build and pass its tests without the UI.
- The interface only displays things and passes settings to the engine. No toolpath maths, geometry calculations or G-code generation in the front-end code.
- UI changes must never change G-code output. If a UI change needs an engine change, stop and explain it before doing it.
- Point and arc maths (bulge ↔ arc, arc sweep across 0°, angle normalising, point helpers) lives
  once in `engine/src/geometry.rs` with its own tests (`engine/tests/arc_maths.rs`). Use those
  helpers; don't write another copy (copies drifting apart caused the arc-across-0° display bug).

## Keep the engine WebAssembly-compatible
The engine must keep building for WebAssembly (`wasm32-unknown-unknown`). To keep that possible:
- No direct file system access, OS-specific code or native-only threading inside the engine: it takes data in and returns data. File opening/saving and machine communication live in the app layer (Tauri), not the engine.
- The interface talks to the engine through one thin connection layer (`app/src/engine/index.ts`), not calls scattered through the UI.
- Before adding a Rust library to the engine, check it compiles for wasm32-unknown-unknown. Ask before adding one that doesn't.
- CI builds the engine for WebAssembly on every push, so a break is caught straight away.

## Outside libraries and sources
These rules apply to any library, or other outside code or data, whose output affects geometry,
toolpaths or G-code. Example: cavalier_contours does the offsetting, in `engine/src/offset.rs`.
- **Pin the exact version** in `engine/Cargo.toml` (e.g. `"=0.9.0"`), not just via the lock file.
  New releases are never picked up automatically, not even bug-fix releases. Code or data that is
  not a Cargo dependency (copied code, bundled data) has its origin, version and licence noted in
  the file that uses it.
- **One adapter file per library.** Only one engine file calls it. The rest of the engine asks the
  adapter for results in our own types. An update or a library swap touches only that file.
- **Every workaround lives in the adapter**, with a comment naming the library's issue (e.g. arcs
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
- **If a library stops being maintained:** its licence lets us keep our own patched copy and
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
- Core features are committed straight to `main`. Use a branch for experimental work, and ask
  if unsure.
- Trials of new libraries go on a branch, so `main` never carries a library that doesn't work
  out. When the user is happy, merge into `main` locally and push (no pull request needed). CI runs on every branch.
- Commit small and often, with clear messages.
- Never commit secrets. Keys and credentials go in `.env`, which is git-ignored.
- Tasks are tracked in GitHub Issues. Use the `gh` CLI to read and update them.
- Planned features go in `docs/roadmap.md` and known limitations in `docs/limitations.md`, not in
  the spec or this file.
- At the end of a session, update any decisions above, `docs/roadmap.md` if new ideas came up,
  and `docs/limitations.md` if a limitation was found or fixed.

## Keeping the docs
- **One home per fact.** `CLAUDE.md` holds rules for Claude, `docs/spec.md` holds how WHAM_CAM
  behaves, `docs/roadmap.md` holds planned features, and `docs/limitations.md` holds what it
  doesn't do or does in a surprising way. Before adding something, check it isn't already in
  another file, and point to it instead of copying it.
- **Describe the current state only.** No dates, no "decided", "changed from", "moved to" or "now",
  and no history. Git and the release notes keep that.
- **Keep the files apart.** No planned features in the spec or `CLAUDE.md`. No limitations written
  into the roadmap. Roadmap items are written as things to do.
- **When something ships:** remove it from the roadmap, describe the behaviour in the spec, and
  remove it from `docs/limitations.md` if it fixed a limitation.
- **Edit as little as possible.** Change only what the task needs, don't reword or reorganise
  unrelated text, and ask before adding, removing or restructuring a section.
- **Say what changed.** At the end of a session, tell the user which docs changed and why, in plain
  English.
- **Keep `CLAUDE.md` short.** If a section only repeats the spec, cut it and point to the spec.

## Releases
- **Version numbers** use semantic versioning, `MAJOR.MINOR.PATCH`. While building towards v1,
  MAJOR stays 0: a release that adds features bumps MINOR (0.2.0), a release with only fixes
  bumps PATCH (0.2.1). 1.0.0 = the v1 scope works end to end. After that, MAJOR goes up only for
  breaking changes (e.g. old project files no longer open).
- **PATCH releases** do not change toolpath or G-code output unless they fix a safety bug, and then
  the release notes say so.
- **Project-file compatibility** (from 1.0.0): a newer WHAM_CAM opens every older `.whamcam`; older
  format versions are upgraded on open (spec 5.8); format version bumps are listed in the release
  notes.
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
     Installers are unsigned (Windows warns "unknown publisher").
- **Safety in releases:** if a release changes toolpath or G-code output, the notes say so in a
  separate "G-code output changed" section, saying what changed and reminding the user to
  re-check in a simulator and with an air cut. Every release is checked against the
  checked reference G-code (`engine/tests/reference_gcode/`) before tagging.
- **Traceability:** every G-code file's header comment includes the
  WHAM_CAM version that produced it, e.g. `( WHAM_CAM 0.4.0 )`.

## Where things are
- Docs: `docs/spec.md` (how v1 behaves), `docs/roadmap.md` (planned features), `docs/limitations.md`
  (known limitations).
- The current version is the one in `engine/Cargo.toml` (see Releases for the four files that carry it).
- Engine (`engine/src/`):
  - Project and document: `project.rs` (the open project, undo/redo, selection), `document.rs`
    (layers, shapes and machining parts), `display.rs` (what the interface draws).
  - Import: `dxf.rs`, `svg.rs`, `curve.rs` (splines and ellipses to arcs).
  - Geometry and editing: `geometry.rs` (shared point and arc maths), `draw.rs`, `edit.rs`,
    `transform.rs`, `dimension.rs`.
  - Toolpaths: `operation.rs`, `toolpath.rs`, `path.rs`, `offset.rs` (the only file that calls
    cavalier_contours), `pocket.rs`, `profile.rs`, `drill.rs`, `safety.rs` (safety checker).
  - Tools, machines and files: `tooling.rs` (master tool list and libraries), `machine.rs`
    (machine list), `project_template.rs` (project templates), `project_file.rs` (`.whamcam`
    files). `Project::resolve` works out which tool each operation uses.
  - G-code: `post.rs` (generic post-processor), `gcode_read.rs` (read-back check).
- Engine tests are in `engine/tests/`, one file per area (e.g. `pocket.rs`, `dxf.rs`).
  `reference_gcode.rs` compares output with the checked files in `engine/tests/reference_gcode/`.
- The reference examples (Drill, Pocket, Profile) are in `tests/reference/local/`. The whole
  top-level `tests/` folder is git-ignored (this machine only), so the expected moves from the
  references are written into the engine tests.
