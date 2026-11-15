# WHAM_CAM v1 Specification

Status: **draft for review**. Nothing here is built yet. Items marked **(?)** need a decision from the user.

## 1. Purpose

WHAM_CAM is a simple 2.5D CAM program in the spirit of the reference program. You draw or import 2D geometry,
select shapes, apply machining operations (profile, pocket, drill), preview the toolpaths and
output G-code for the machine.

Design principles (from CLAUDE.md):
- **Geometry first, operations second.** Select shapes, then apply an operation to them.
- **One main screen.** Drawing area, object/operation tree, properties panel.
- **Every setting visible and editable.** No hidden wizards, no 3D modelling, no parametric history.
- **Simple wins.** If a feature makes the common workflow slower or more confusing, it doesn't go in.
- **The reference program is the behaviour reference** (clean room: we study its behaviour and output, never its code).

## 2. Stack

| Part | Choice |
|---|---|
| Desktop shell | Tauri (Windows + Linux, fully offline) |
| CAM engine | Rust library crate, no UI, own tests, must also build for WebAssembly |
| Interface | React + TypeScript |
| Drawing area | WebGL via three.js (MIT licence): 2D top view and 3D orbit view |
| Units | mm only in v1 |

## 3. Architecture

```
┌──────────────────────────────────────────────────────────┐
│ Interface (React + TypeScript)                           │
│   tree · properties panel · three.js drawing area        │
│   Displays things, collects settings. No CAM maths.      │
└───────────────────────┬──────────────────────────────────┘
                        │  one thin connection layer
                        │  (Tauri commands now, WebAssembly later)
┌───────────────────────┴──────────────────────────────────┐
│ App layer (Tauri, Rust)                                  │
│   open/save dialogs, reading & writing files             │
└───────────────────────┬──────────────────────────────────┘
                        │  data in, data out
┌───────────────────────┴──────────────────────────────────┐
│ Engine (Rust crate, WASM-compatible)                     │
│   geometry · DXF/SVG parsing · offsets · toolpaths ·     │
│   tool library · post-processor · G-code · project data  │
│   No file system, no OS calls, no native threads.        │
└──────────────────────────────────────────────────────────┘
```

Rules:
- Everything that affects the machine lives in the engine.
- The engine receives file *contents* (text/bytes), never file paths.
- The interface talks to the engine only through the connection layer (one TypeScript module).
- A UI change must never change G-code output.

## 4. Screen layout

Matches the the reference program layout the user already knows:

```
┌─────────────────────┬────────────────────────────────────────┐
│ Tree                │                                        │
│  Project            │                                        │
│   Layers            │          Drawing area                  │
│    Default          │          (grid, geometry, toolpaths)   │
│     Polyline (4)    │                                        │
│   Machining         │          2D top view  /  3D view       │
│    Part1            │                                        │
│     Profile1        │                                        │
├─────────────────────┤                                        │
│ Properties          │                                        │
│  (settings of the   │                                        │
│   selected item)    ├────────────────────────────────────────┤
│                     │ Messages / status · cursor X,Y         │
└─────────────────────┴────────────────────────────────────────┘
```

- **Tree**: Layers → drawing objects; Machining → Parts → operations. Click to select,
  right-click for actions (rename, delete, enable/disable, generate toolpath).
- **Properties panel**: every setting of whatever is selected, editable in place.
- **Drawing area**: dark background, grid, X/Y axes, cursor coordinates in the status bar.
  Pan, zoom, select (click and box-select). A toggle switches between the 2D top view and a 3D orbit
  view of the toolpaths.
- **Workflow**: select shapes → Machining menu / toolbar → Profile, Pocket or Drill → the new
  operation appears in the tree and its properties are shown → Generate toolpaths → Produce G-code.

## 5. v1 Features

### 5.1 Import
- **DXF**: LINE, ARC, CIRCLE, LWPOLYLINE (with bulges), POLYLINE, POINT. Others are skipped
  with a message listing what was ignored. **(?)** Also SPLINE/ELLIPSE (converted to polylines)?
- **SVG**: path, line, polyline, polygon, rect, circle, ellipse. Curves are converted to arcs/lines.
- Imported objects go onto layers (DXF layer names are kept).

### 5.2 Drawing
- Line, arc, circle, rectangle, polyline (with arcs), point/point list.
- Snap to grid, and to end points, mid points and centres.
- Grid spacing setting.

### 5.3 Edit
- Transform: move, rotate, scale, mirror, with typed values.
- Join (lines/arcs into polylines, with a tolerance), split/explode, close polyline.
- Basic dimensions: linear and radius/diameter annotations (display only, no effect on G-code).
- Undo/redo.

### 5.4 Operations
Profile (inside / outside / on line), Pocket, Drill. Settings in section 6.

### 5.5 Tool library
Each tool has: number, name, diameter, flutes, profile (end mill only in v1), default
cut feed, plunge feed, spindle speed, depth increment, stepover. An operation can pick a tool
from the library (which fills in its defaults) or have values typed in directly.

### 5.6 Toolpath preview
- Cutting moves, plunges and rapids drawn in different colours (rapids dashed).
- 2D top view and 3D view showing the depth levels.
- Per-operation show/hide.

### 5.7 G-code output
Through a post-processor. v1 has one: **WorkBee / RepRapFirmware** (section 7).

### 5.8 Project files
Save/open a WHAM_CAM project: one readable JSON file (`.wcam` (?)) with a format version number.
Holds geometry, layers, parts, operations, tools used and post-processor settings.

## 6. Operation settings

Names follow the reference program's where sensible. "v1" = editable in v1; "later" = shown greyed out or left out.

### 6.1 Common to all operations

| Setting | Meaning | Default | v1 |
|---|---|---|---|
| Name | Operation name, written as a G-code comment | Profile1, Pocket1… | v1 |
| Enabled | Include in G-code output | true | v1 |
| Tool number | Tool from library | 0 | v1 |
| Tool diameter | mm | from tool | v1 |
| Stock surface | Z of top of material | 0 | v1 |
| Target depth | Final Z (negative = into material) | — (must be set) | v1 |
| Depth increment | Max depth per pass | from tool | v1 |
| Final depth increment | Size of the last pass (0 = equal passes) | 0 | v1 |
| Clearance plane | Z for rapids between cuts | 3 **(?)** | v1 |
| Cut feedrate | mm/min | from tool | v1 |
| Plunge feedrate | mm/min | from tool | v1 |
| Spindle speed | RPM | from tool | v1 |
| Spindle direction | CW / CCW / Off | CW | v1 |
| Roughing clearance | Stock left on the wall (negative = overcut) | 0 | v1 |
| Custom header/footer | Extra G-code before/after this operation | empty | later |
| Lead-in / lead-out | Spiral, ramp, tangent | none (straight plunge) | later |

### 6.2 Profile

| Setting | Meaning | Default | v1 |
|---|---|---|---|
| Inside/Outside | Inside, Outside, or On line (no offset) | Outside | v1 |
| Milling direction | Conventional / Climb | Conventional **(?)** | v1 |
| Cut ordering | Depth first / Level first (across multiple shapes) | Depth first | v1 |
| Cut width | Total wall width; extra passes stepped by Stepover | 0 (= one tool width) | later |
| Corner overcut | Dog-bone style corner relief | false | later |
| Holding tabs | | none | later |

Behaviour (observed in the reference program): outside corners get an arc of tool radius (G2/G3).
For a closed shape, the cut overlaps back over its start point. **(?)** How much overlap does v1 need without lead-ins?

### 6.3 Pocket

| Setting | Meaning | Default | v1 |
|---|---|---|---|
| Stepover | Fraction of tool diameter | 0.4 | v1 |
| Stepover feedrate | Use Cut or Plunge feedrate for stepover moves | Plunge feedrate | v1 |
| Milling direction | Conventional / Climb | Conventional | v1 |
| Region fill style | Inside→outside offsets | Inside→outside offsets | v1 (only style) |
| Cut ordering | Depth first / Level first | Depth first | v1 |
| Finish stepover | Light final wall pass | 0 | later |

Islands: closed shapes inside the selected boundary are left uncut. **(?)** Confirm this matches how you use the reference program.

### 6.4 Drill

| Setting | Meaning | Default | v1 |
|---|---|---|---|
| Points | Point lists, or circle centres | — | v1 |
| Peck distance | 0 = single plunge | 0 | v1 |
| Retract height | Z between pecks / holes | 3 | v1 |
| Dwell | Seconds at bottom | 0 | v1 |
| Drilling method | How the G-code is written | **(?)** see section 9 | v1 |

## 7. G-code output: WorkBee / RepRapFirmware

Based on the user's the reference program reference files (`tests/reference/local/`). Structure:

```
( Made using WHAM_CAM )
( <project name> <date/time> )
( Post processor: WorkBee RRF )
( T<n> : <diameter> )
G21 G90 G40          ← mm, absolute, no cutter comp   (G64: see section 9)
G0 Z<clearance>
( T<n> : <diameter> )
T<n> M6              (?) see section 9
M3 S<rpm>
M291 P"Spindle" R"Check running?" S3   ← operator confirms spindle is running
( <operation name> )
G17
S<rpm>
...toolpath...
G0 Z<clearance>
M5
M30
```

Format rules:
- Coordinates and feeds to 4 decimal places, trailing zeros trimmed, at least one decimal (`Z3.0`, `X-3.3431`).
- Modal: only changed axes and feeds are written.
- Arcs as G2/G3 with incremental I/J centres.
- Comments on each operation: name, tool, depths.

Safety rules (from CLAUDE.md; tests must check these on every output):
- Every rapid (G0) happens at or above the clearance plane, **except** the final approach:
  a rapid straight down to just above the material or previous cut level, then feed (G1) down.
  (the reference program's pocket does this: rapid to Z-4.0 when the previous level was Z-5.0, then G1 to Z-10.0.)
- No horizontal rapid below the clearance plane.
- The program starts and ends at the clearance plane.

## 8. Testing

- The engine has its own unit tests (`cargo test`) that run without the UI.
- **Reference comparison tests**: WHAM_CAM's output for a given project is compared against
  known-good G-code. The the reference program examples in `tests/reference/local/` are git-ignored (they stay
  on this machine), so tests that use them only run locally. Once WHAM_CAM's own output has been
  checked in a simulator and air-cut, those files become the committed reference set.
- **Safety checker**: a test helper that reads any G-code output and fails if it breaks the section 7 safety rules.
- The engine is also built for `wasm32-unknown-unknown` in CI.
- Whenever toolpath or post-processor logic changes, the user re-checks output in CAMotics/NC Viewer and with an air cut.

## 9. Open questions (for the G-code discussion)

1. **G64.** The reference files contain `G21 G90 G64 G40`, and the user says they run as-is. CLAUDE.md
   says never emit G64. Which is right? (In the reference program it comes from *Velocity Mode = Constant Velocity*.)
2. **Which post made the reference files?** Their headers say "Post processor: Default" although
   "Workbee" is selected, and the the reference program screenshots show the error *"Unexpected token at line 13"*.
   Possibly the Workbee post failed to load and the reference program fell back to Default.
3. **Drilling.** the reference program's G81 canned cycle doesn't work on the WorkBee; the user has a custom script.
   v1 needs that script, or the G-code it produces, to copy.
4. **Tool change.** Is `T0 M6` wanted on the WorkBee, or should a tool change pause with an M291 prompt?
5. **Default milling direction.** The examples use conventional milling. Should that stay the default?
6. **Profile overlap** without lead-ins: how far past the start point should a closed profile cut?
7. **Fast plunge height.** the reference program's pocket rapids down to 1 mm above the last cut level. Should this be a setting, and is 1 mm the right default?

## 10. Out of scope for v1

Inches, lead-ins/lead-outs, holding tabs, CAM styles/templates, other post-processors
(Grbl, LinuxCNC, Mach3), browser/WebAssembly app, importing other CAM programs' project files, 3D operations, machine control.

## 11. Setup (for the build step)

Linux system libraries for Tauri:
```
sudo apt install libwebkit2gtk-4.1-dev libsoup-3.0-dev librsvg2-dev build-essential libssl-dev libayatana-appindicator3-dev
```
WebAssembly target for the engine check:
```
rustup target add wasm32-unknown-unknown
```
