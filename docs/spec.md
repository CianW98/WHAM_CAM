# WHAM_CAM v1 Specification

Status: **draft for review**. Nothing here is built yet. Items marked **(?)** need a decision from the user.

## 1. Purpose

WHAM_CAM is a simple 2.5D CAM program in the spirit of the classic simple 2.5D CAM programs. You draw or import 2D geometry,
select shapes, apply machining operations (profile, pocket, drill), preview the toolpaths and
output G-code for the machine.

Design principles (from CLAUDE.md):
- **Geometry first, operations second.** Select shapes, then apply an operation to them.
- **One main screen.** Drawing area, object/operation tree, properties panel.
- **Every setting visible and editable.** No hidden wizards, no 3D modelling, no parametric history.
- **Simple wins.** If a feature makes the common workflow slower or more confusing, it doesn't go in.
- **An established simple CAM program is the behaviour reference** (clean room: we study behaviour and output, never code).

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

A classic CAD/CAM layout, with a toolbar:

```
┌──────────────────────────────────────────────────────────────┐
│ Menus                                                        │
│ Toolbar: [New Open Save Import] [Line Arc Circle Rect Pline  │
│          Point] [Profile Pocket Drill] [Generate] [G-code]   │
├─────────────────────┬────────────────────────────────────────┤
│ Tree                │                                        │
│  Project            │                                        │
│   Layers            │          Drawing area                  │
│    Default          │          (grid, geometry, toolpaths)   │
│     Polyline (4)    │                                        │
│   Machining         │          2D top view  /  3D view       │
│    Part1            │                                        │
│     Profile1        │                      ┌──────────────┐  │
├─────────────────────┤                      │ notification │  │
│ Properties          │                      └──────────────┘  │
│  (settings of the   │                                        │
│   selected item)    │  X 12.000  Y -4.500                    │
└─────────────────────┴────────────────────────────────────────┘
```

- **Toolbar**: file actions (new, open, save, import), drawing tools (line, arc, circle,
  rectangle, polyline, point), operations (Profile, Pocket, Drill), Generate toolpaths and
  Produce G-code. Everything on the toolbar is also in the menus.
- **Tree**: Layers → drawing objects; Machining → Parts → operations. Click to select,
  right-click for actions (rename, delete, enable/disable, generate toolpath).
- **Properties panel**: every setting of whatever is selected, editable in place.
- **Drawing area**: dark background, grid, X/Y axes. Pan, zoom, select (click and box-select).
  A toggle switches between the 2D top view and a 3D orbit view of the toolpaths. The cursor
  X/Y position is shown in a small readout in a corner of the drawing area.
- **Notifications**: no status bar. Important messages (items skipped on import, toolpath errors,
  file saved) pop up over the drawing area. Errors stay until dismissed; info messages fade out.
- **Workflow**: select shapes → toolbar or Machining menu → Profile, Pocket or Drill → the new
  operation appears in the tree and its properties are shown → Generate toolpaths → Produce G-code.

## 5. v1 Features

### 5.1 Import
- **DXF** (ASCII), read by the engine's own reader:

  | DXF entity | Becomes |
  |---|---|
  | LINE | Line |
  | ARC | Arc |
  | CIRCLE | Circle |
  | LWPOLYLINE, 2D POLYLINE | Polyline (bulge arcs kept as true arcs) |
  | POINT | One point list per layer (for drilling) |
  | SPLINE | Spline, kept exact |
  | ELLIPSE | Ellipse, kept exact |
  | INSERT (blocks) | Block contents placed with position, rotation, scale and arrays |

  Everything else (TEXT, HATCH, DIMENSION, 3D entities, paper space, etc.) is skipped, and the
  import message lists what was skipped. Binary DXF and broken files give a clear error.
- **Splines and ellipses stay exact** in the drawing. They are converted to arcs and lines only
  when a toolpath is made (conversion tolerance is a toolpath setting). A "Convert to polyline"
  command will allow editing them like any polyline.
- **Units**: if a DXF says it is in inches (or other non-mm units), import asks whether to
  convert to mm or treat the numbers as mm.
- Import adds to the current drawing as one undo step.
- **SVG**: path, line, polyline, polygon, rect, circle, ellipse. Curves are converted to arcs/lines.
- Imported objects go onto layers (DXF layer names are kept).

### 5.2 Drawing
- Line, arc, circle, rectangle, polyline (with arcs), point/point list.
- Points are set by clicking (with snapping to grid, end points, mid points and centres) or by
  typing coordinates while drawing; exact values can also be edited in the properties panel afterwards.
- Grid spacing setting.

### 5.2a Selection
- Click selects a shape; Shift/Ctrl-click adds or removes. Esc clears.
- Box select: drag left-to-right selects shapes fully inside; right-to-left selects anything touched.
- Delete removes the selection. Undo/redo (Ctrl+Z, Ctrl+Y) cover every change.

### 5.2b Layers
- Every layer has a name and a colour; everything on a layer is drawn in its colour.
  New layers get a colour automatically from a set of strong, distinct colours.
- Layer colours are plain colours (no transparency). Yellow is reserved for the selection highlight.
- In the tree: click a layer to select it and open/close it; double-click to rename it in place;
  click its colour square to change the colour. Name and colour can also be edited in Properties.
- Colour picker: 24 preset colours plus a box for an exact #rrggbb value.
- Layer names must be unique and not empty. Renames and colour changes are undoable.
- Deleting a layer deletes everything on it. An emptied layer stays until deleted.

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
Standard, widely compatible G-code through one generic post-processor (section 7).
Machine-specific post-processors, each with its own settings panel per machine type, come later.

### 5.8 Project files
Save/open a WHAM_CAM project: one readable JSON file (`.wcam` (?)) with a format version number.
Holds geometry, layers, parts, operations, tools used and post-processor settings.

## 6. Operation settings

Names follow common CAM terminology. "v1" = editable in v1; "later" = shown greyed out or left out.

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
| Milling direction | Conventional / Climb | Conventional | v1 |
| Cut ordering | Depth first / Level first (across multiple shapes) | Depth first | v1 |
| Cut width | Total wall width; extra passes stepped by Stepover | 0 (= one tool width) | later |
| Corner overcut | Dog-bone style corner relief | false | later |
| Holding tabs | | none | later |

Behaviour (observed in the reference output): outside corners get an arc of tool radius (G2/G3).
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

Islands: closed shapes inside the selected boundary are left uncut. **(?)** Confirm this matches what you expect.

### 6.4 Drill

| Setting | Meaning | Default | v1 |
|---|---|---|---|
| Points | Point lists, or circle centres | — | v1 |
| Peck distance | 0 = single plunge | 0 | v1 |
| Retract height | Z between pecks / holes | 3 | v1 |
| Dwell | Seconds at bottom | 0 | v1 |
| Drilling method | Each hole written out in full: rapid to retract height, feed down (pecking if set), back up | Full moves | v1 |

The exact G-code format for drilling is decided with the machine-specific post-processors.

## 7. G-code output

v1 writes standard G-code through one generic post-processor. It aims to run on most
hobby and small-shop controllers. The exact format is refined later, when machine-specific
post-processors are added (each with its own settings panel per machine type).

Principles:
- Plain and readable: one move per line, no unnecessary codes.
- Commented: program name and date, and for each operation its name, tool and depths.
- Millimetres, absolute coordinates.
- Arcs are output as arcs, not broken into many short lines.

Safety rules (from CLAUDE.md; tests check these on every output):
- Every rapid move happens at or above the clearance plane, **except** the final approach:
  a rapid straight down to just above the material or the previous cut level, then a feed move down.
- No horizontal rapid below the clearance plane.
- The program starts and ends at the clearance plane.

## 8. Testing

- The engine has its own unit tests (`cargo test`) that run without the UI.
- **Reference toolpath comparison**: the reference examples in `tests/reference/local/` (git-ignored,
  kept on this machine only) are used to check toolpath **shape**: offsets, stepovers, depth levels
  and cut order. They are not compared as exact text, because WHAM_CAM's G-code format differs from the reference program's.
- **WHAM_CAM reference outputs**: once WHAM_CAM's own output has been checked in a simulator and
  with an air cut, it is committed to `tests/reference/` and later output is compared to it exactly.
- **Safety checker**: a test helper that reads any G-code output and fails if it breaks the section 7 safety rules.
- The engine is also built for `wasm32-unknown-unknown` in CI.
- Whenever toolpath or post-processor logic changes, the user re-checks output in a simulator
  (CAMotics or NC Viewer) and with an air cut.

## 9. Open questions

1. **Profile overlap** without lead-ins: how far past the start point should a closed profile cut?
2. **Fast plunge height.** Between depth levels the reference output rapids down to 1 mm above the last cut level.
   Should this be a setting, and is 1 mm the right default?

## 10. Out of scope for v1

Ideas for after v1 are kept in `docs/roadmap.md`.
