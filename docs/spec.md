# WHAM_CAM v1 Specification

Status: **v1 in progress.** DXF import, layers, drawing, selection, transforms, edit tools
(sections 5.1–5.3, apart from SVG import and dimensions), project files (5.8), and Profile,
Pocket and Drill toolpaths with preview and playback (5.4, 5.6, 6.1–6.4) are built. The tool
library (5.5) and G-code output (5.7, 7) are next; see the Status in CLAUDE.md. Items marked
**(?)** need a decision from the user.

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

- **Menu bar** across the top: File (New, Open, Save, Save As, Import DXF), View, Edit (Undo/Redo,
  Cut/Copy/Paste, Delete, Select All / On Layer, Transform, Join, Explode, Break at
  intersections, Close polyline, Dimension), Draw (the drawing tools), Machining (Profile, Pocket, Drill, Tool library, Generate
  toolpaths, Produce G-code). Click a heading to open it; while one is open, hovering another
  switches to it. Items not built yet are greyed.
- **Toolbar**: file actions (new, open, save, import), drawing tools (line, arc, circle,
  rectangle, polyline, point), operations (Profile, Pocket, Drill), Generate toolpaths and
  Produce G-code. Everything on the toolbar is also in the menus.
- **Tree**: Layers → drawing objects; Machining → Parts → operations. Click to select,
  right-click for actions (rename, delete, enable/disable, generate toolpath).
- **Properties panel**: every setting of whatever is selected, editable in place. A point is one
  `x,y` box. Polyline and point-list points open in a pop-up table
  ("Points (Collection) …": X, Y and, for polylines, bulge; add/remove points, OK saves) instead
  of filling the panel. Closed polylines and rectangles show their direction (CW/CCW).
- **Right-click menu** in the drawing area (right-click without dragging; right-drag pans):
  View (Zoom to fit, 2D/3D), Draw (the drawing tools), Transform, Edit, Select All,
  Select All On Layer (current layer), Cut, Copy, Paste, Undo, Redo. Items that can't be used
  right now, or aren't built yet, are greyed.
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
  - **Line**: start, end. Every two clicks make one separate line; the tool stays active.
  - **Arc**: start, end, then centre. The centre slides along the line halfway between the ends,
    so it is always the same distance from both. The arc runs counter-clockwise from start to end:
    putting the centre on the other side switches between the short and the long arc.
  - **Circle**: centre, then a point on the circle.
  - **Rectangle**: two opposite corners. Makes a **PolyRectangle**: Lower Left, Width, Height and
    Rotation, always counter-clockwise. Moving, scaling, mirroring and rotating keep it a
    PolyRectangle; rotations by multiples of 90° are stored unrotated with the true lower-left
    corner.
  - **Polyline**: click points; **A** makes the next segments tangent arcs, **L** straight lines
    (the first segment is always straight). Enter or double-click finishes; clicking the first
    point closes it. Backspace removes the last point.
  - **Point**: each click adds a point; Enter or double-click makes them one point list.
  - Esc cancels the shape in progress; a second Esc stops drawing.
  - Shortcut keys start a tool: **L** line, **A** arc, **C** circle, **R** rectangle, **P**
    polyline, **.** (full stop) point (shown in the Draw menu). While a polyline is being drawn (after its first click),
    A and L instead switch its next segment between arc and line.
- New shapes go onto the **current layer**, marked ✎ in the tree. Clicking a layer makes it current.
- Points are set by clicking. Clicks snap to end points, mid points and centres of shapes
  (within a few pixels) or else to the grid; **Grid** and **Objects** snap toggles sit in the
  drawing area, and a marker shows what was snapped to. A finished shape becomes the selection.
- Exact values are edited in the properties panel afterwards: line ends, arc centre/radius/angles,
  circle centre/radius, polyline closed and points, rectangle lower left/width/height/rotation,
  point-list points. Splines and ellipses are read-only.
  (A typed-coordinate box while drawing was tried and removed as unintuitive.)
- Grid spacing: a project setting (default 10 mm), in the project's properties.

### 5.2a Selection
- Click selects a shape; Shift/Ctrl-click adds or removes. Esc clears.
- Box select: drag left-to-right selects shapes fully inside; right-to-left selects anything touched.
- Up/Down arrow keys select the previous/next shape in tree order (stopping at the ends); the
  tree scrolls to keep it in view.
- The view zooms to fit after a DXF import and on Zoom to fit; drawing never moves the view.
- Delete removes the selection. Undo/redo (Ctrl+Z, Ctrl+Y) cover every change.

### 5.2b Layers
- Every layer has a name and a colour; everything on a layer is drawn in its colour.
  New layers get a colour automatically from a set of strong, distinct colours.
- Layer colours are plain colours (no transparency). Yellow is reserved for the selection highlight.
- Right-click a layer (or the Layers heading) for New layer, Rename and Delete layer. A new layer
  is called "Layer 1", "Layer 2", ..., gets the next free colour, becomes the current layer and
  opens for renaming straight away.
- In the tree: click a layer to select it and open/close it; double-click to rename it in place;
  click its colour square to change the colour. Name and colour can also be edited in Properties.
- Colour picker: 24 preset colours plus a box for an exact #rrggbb value.
- Layer names must be unique and not empty. Renames and colour changes are undoable.
- Deleting a layer deletes everything on it. An emptied layer stays until deleted.

### 5.3 Edit
- Transform (right-click menu → Transform), always changing the selected shapes:
  - **Move…** (dX, dY), **Rotate…** (angle, + = anticlockwise), **Scale…** (one factor, more
    than 0) in a small dialog that remembers the last values.
  - **Move by clicking**: click a base point, then the destination (snapped, with a preview).
  - **Rotate by clicking**: a dashed line runs from the selection centre to the cursor; the first
    click sets the reference direction, then the shapes turn with the cursor and the second
    click sets the final angle.
  - **Mirror left–right** and **Mirror top–bottom**.
  - Rotate, scale and mirror work around the centre of the selection's bounding box.
- Cut, Copy, Paste (Ctrl+X/C/V): the clipboard is inside the app. Paste follows the snapped
  cursor (by the centre of the copied shapes) and a click drops it; pasted shapes go back onto
  the layers they came from and are selected. Select All (Ctrl+A), Select All On Layer
  (Ctrl+Shift+A, current layer).
- **Join…** asks for a tolerance (default 0.01 mm, remembered). Lines, arcs and open polylines
  whose ends are within it are chained into polylines (pieces turned round as needed; where
  several ends meet, the closest wins). A chain whose ends meet is closed. Each new polyline takes
  the layer of its first piece. Shapes that touch nothing, and other shape types, are left alone.
- **Explode**: polylines become separate lines and arcs; PolyRectangles become 4 lines.
- **Break at intersections**: wherever two or more selected shapes cross or touch, each shape
  running through that point is cut there. Lines, arcs, circles, polylines and PolyRectangles
  (which become polylines). A circle needs 2 cuts to become arcs; a closed polyline cut once
  opens at the cut. A shape is not cut where it crosses itself.
- **Close polyline**: closes open polylines; a last point on top of the first is dropped.
- Each of these is one undo step, and the results become the selection.
- **Dimensions** are put off until later in v1, together with more DXF features (to be listed).
- Undo/redo.

### 5.4 Operations
Profile (inside / outside / on line), Pocket, Drill. Settings in section 6.

### 5.5 Tool library
Decided with the user 2026-11-19.
- **Master tool list** (one per computer, shared by every project): every physical tool the user
  has. Each tool: name, end type (End mill, Ball nose, V-bit, Drill), diameter, shank
  diameter (blank = same as the diameter), flutes, flute type (Spiral = upcut, Straight, Downcut =
  reverse spiral; not drills, decided 2026-11-19), helix angle (Spiral and Downcut, not drills; default 30°, the common
  general-purpose angle), flute length, overall length, V angle (V-bit
  only). All mm.
- **Automatic tool names** (decided 2026-11-19): made from diameter, V angle (V-bits), flutes,
  flute type (not for drills), end type and flute length (not the helix angle), e.g. "6mm 2F Spiral End Mill, 20mm
  flute", and kept up to date when those
  change. Typing a name (or unticking *Automatic name*) keeps the typed name. Tools saved before
  this keep their names.
- The Add/Edit tool form shows a **live side view** of the tool on the right (silhouette from the
  engine: flat, ball, V point; drills drawn with a 118° point; flute edges drawn by flute type
  and helix angle), with the diameters and the flute
  and overall lengths marked.
- **Tools hold no feeds or speeds**: those depend on the material as well as the tool. In v1 each
  operation keeps its own feeds, speeds, depth increment and stepover; the materials library
  (roadmap) will later fill them in from tool + material.
- **Tool libraries** (also per computer): named sets of master tools, each given a tool number
  (unique in the library, from 1), e.g. "mm tooling" and "inch tooling" (values still in mm), or
  what is loaded in a tool changer. The same master tool can be in several libraries under
  different numbers.
- The tooling is kept in `tooling.json` in WHAM_CAM's settings folder on the computer (the app
  layer reads/writes it; the engine only converts it to and from text). Tooling changes are saved
  straight away and are not undo steps. A damaged file is kept aside as `tooling.json.damaged`,
  never overwritten.
- **Left panel tabs**: *Project* (the tree) and *Setup* (Master tool list, Tool libraries).
  Clicking Master tool list opens its window (Add tool / Edit / Delete; double-click a row to
  edit). Right-click Tool libraries → New library; right-click a library → Rename, Use in this
  project, Delete library. Selecting a library shows its numbered tools in Properties, where tools
  are added from the master list, renumbered or taken out.
- **Active library**: each project chooses one in the Machining item's Properties (also the
  Machining menu → Tool library, or the library's right-click menu in Setup); saved in the
  project file by name.
- **Tools follow the Machining tree** (decided 2026-11-19):
  - *Machining* Properties: Tool library and **Tool** (from that library).
  - *Part* Properties: **Tool**, "Use Machining tool" by default, or a library tool to override
    it for the part's operations. No library choice of its own.
  - *Operation* Tool box: "Use part tool" / "Use Machining tool" (the default for new
    operations), "Typed in below" (own tool number and diameter), or a library tool.
  - While an operation follows the tree, its Tool number and Tool diameter are shown greyed out.
    Choosing "Typed in below" starts from the tool it was using. Typing a different diameter
    over a library tool makes it "typed in".
  - The engine works out each operation's tool (own, else part's, else Machining's) before
    generating; the toolpath maths is unchanged. Changing a Machining or part tool marks the
    operations following it out of date (only the tool number and diameter count).
  - An operation following the tree with no tool set anywhere: its Tool box says "No tool
    selected" (or "No library selected"), and Generate toolpaths refuses it with the message
    "No tool selected. Choose a tool for Machining, the part, or the operation."
  - Every picked tool is copied into the project, so it opens and cuts the same on a computer
    with different tooling. Only end mills (flat end) can be picked in v1; other end types are
    greyed out, and the engine refuses to generate with them.
- The tool number goes into the G-code (tool changes) when G-code output arrives.

### 5.6 Toolpath preview
- Cutting moves, plunges and rapids drawn in different colours (rapids dashed).
- 2D top view and 3D view showing the depth levels.
- Per-operation show/hide ("Show toolpath"); disabled operations aren't drawn.
- Toolpaths are recalculated only on **Generate toolpaths** (toolbar, Machining menu, or an
  operation's right-click menu). An operation whose settings or shapes changed since is marked
  `⟳` (out of date) in the tree; one that failed is marked `!` with the message in Properties.
- Every toolpath is checked by the safety checker (section 7 rules) when it is generated; one that
  fails is not used.
- **Show toolpaths** (toolbar button next to Generate toolpaths, and the View menu) hides or shows
  all toolpaths at once. View state only: not saved, not an undo step. Each operation's own "Show
  toolpath" still applies.
- **Show cut width** (toolbar button next to Show toolpaths, and the View menu; off by default)
  shades the band the cutter sweeps along every cutting move (tool diameter wide, rounded at
  bends and ends), under the toolpath lines. Rapids and plunges aren't shaded. View state only.
  During playback it fades and brightens with the tool, like the toolpath lines.
- **Tool playback** (decided 2026-11-16): a bar along the bottom of the drawing area whenever
  toolpaths are shown (View → Toolpath playback hides it).
  - Back to start, one move back, play/pause, one move forward, a slider across the whole run, and
    a speed of 1×, 2×, 5×, 10×, 50× or 100× real time.
  - Real time follows the feed rates (arcs by their true length). Rapids use an **assumed rapid
    rate of 5000 mm/min** until machine settings exist, so the run time is shown as an estimate.
  - The readout shows operation, move number (as in safety checker messages), kind, feed and X Y Z.
  - The tool is a circle of the tool diameter in 2D, and a flat-end cutter with shank in 3D.
  - All shown operations play one after another in tree order. Once started, moves still to come
    are faded and moves done are in full colour. New or changed toolpaths start again from the
    beginning.
  - Material removal simulation is not in v1 (see the roadmap).

### 5.7 G-code output
Standard, widely compatible G-code through one generic post-processor (section 7).
Machine-specific post-processors, each with its own settings panel per machine type, come later.

### 5.8 Project files
Save/open a WHAM_CAM project: one readable JSON file, `.whamcam`, with a format version number.
- File ▸ New (Ctrl+N), Open… (Ctrl+O), Save (Ctrl+S), Save As… (Ctrl+Shift+S); New, Open and Save
  also on the toolbar. Save on a never-saved project does Save As.
- The file holds `"format": "WHAM_CAM project"`, `"format_version"` (1), `"saved_by"` (the WHAM_CAM
  version) and the document: project name, grid spacing, layers (name, colour, shapes) and parts.
  Operations, tools used and post-processor settings are added as they are built.
- Numbers are written so they read back exactly. Files are written to a temporary file and then
  renamed, so a failed save never leaves a half-written project.
- Opening checks the file (format, unique ids and layer names, valid colours and shapes) and gives
  a plain-English message on a damaged or non-WHAM_CAM file, leaving the current project as it
  was. A file from a newer format version is refused, naming the WHAM_CAM version that made it.
  Older format versions will be upgraded on open.
- The project name (top of the tree) follows the file name. The window title shows the file name
  and `*` while there are unsaved changes (undoing back to the saved state clears it).
- New, Open and closing the window with unsaved changes ask Save / Don't save / Cancel.
- Opening starts a fresh undo history, zooms to fit and resets the current layer to the first.
  The copy/paste clipboard is kept, so shapes can be copied between projects.
- Not saved (yet): current layer, view position, snap settings. These wait for project
  preferences (see `docs/roadmap.md`).

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
| Clearance plane | Z for rapids between cuts | 3 | v1 |
| Fast plunge height | Between depth levels: rapid down to this far above the last cut level, then feed down | 1 | v1 |
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
| Inside/Outside | Inside, Outside, or On line (no offset). Open shapes: On line only | Outside (On line if any shape is open) | v1 |
| Milling direction | Conventional / Climb | Conventional | v1 |
| Cut ordering | Depth first / Level first (across multiple shapes) | Depth first | v1 |
| Cut width | Total wall width; extra passes stepped by Stepover | 0 (= one tool width) | later |
| Corner overcut | Dog-bone style corner relief | false | later |
| Holding tabs | | none | later |

Behaviour (observed in the reference output): outside corners get an arc of tool radius (G2/G3).
Decided for when Profile is built (2026-11-16):
- **Milling direction** (spindle CW): Conventional cuts outside walls anticlockwise and inside
  walls clockwise; Climb is the reverse (matches the reference output).
- **Start point**: the shape's first point. No start point setting.

Decided when Profile was built (2026-11-16):
- **Shapes:** lines, arcs, circles, PolyRectangles and polylines (open or closed). Open shapes
  can only be cut **On line** (only that choice is offered). Splines, ellipses and point lists
  are refused.
- **On line direction:** closed shapes are cut like Outside (Conventional = anticlockwise, Climb
  = clockwise); open shapes follow their drawn direction from the first point.
- **Each shape is cut on its own** with the chosen side; nesting is not used (a hole inside an
  outline needs its own Inside operation). An inside offset that splits into several loops cuts
  them all; if nothing is left the tool is too big and that's an error.
- **Start point on an offset path:** the point nearest the shape's first point (the first one
  along the cut on a tie, e.g. round an outside corner arc).
- **Several shapes:** the first listed, then always the nearest shape not yet cut (to its start).
- **Entry:** straight plunge at the start point, as for Pocket (retract, rapid, rapid down to
  fast plunge height above the last level, feed down). The reference Profile example uses a 15°
  spiral lead-in that was set on purpose; its cut loop is matched move for move, the entry is
  not (lead-ins are on the roadmap).

### 6.3 Pocket

| Setting | Meaning | Default | v1 |
|---|---|---|---|
| Stepover | Fraction of tool diameter | 0.4 | v1 |
| Stepover feedrate | Use Cut or Plunge feedrate for stepover moves | Plunge feedrate | v1 |
| Milling direction | Conventional / Climb | Conventional | v1 |
| Region fill style | Inside→outside offsets | Inside→outside offsets | v1 (only style) |
| Cut ordering | Depth first / Level first | Depth first | v1 |
| Finish stepover | Light final wall pass | 0 | later |

Behaviour (built 2026-11-16, matches the reference Pocket example move for move):
- Select closed shapes (polylines, PolyRectangles, circles), then Pocket. The operation's
  **Shapes** in Properties lists their ids (e.g. `1, 2, 3`, as shown in the tree) and can be
  edited; ids that don't exist or can't be pocketed are refused. Nesting decides the
  roles: a shape inside the boundary is an **island** (left uncut); a shape inside an island is a
  new boundary, pocketed on its own. Shapes that cross are refused. Open shapes, splines and
  ellipses are refused with a message.
- Rings: the first is offset by tool radius + roughing clearance (inward from the boundary,
  outward round islands, with tool-radius arcs on corners); then every stepover × tool diameter
  until nothing is left. If a stepover over half the tool would leave a spot uncut in the middle,
  an extra ring is added.
- **Known limitation (stepover over 0.5):** the rings are then further apart than the tool
  reaches from one ring alone, so where the pocket narrows (lobes, tips, thin branches) and the
  next ring doesn't fit, a sliver down the middle can be left uncut. The extra-ring check above
  works on whole rings, so it misses a branch of a larger ring. At 0.5 or less everything is
  cleared (checked by the user 2026-11-16 on a complex pocket). Left as is for v1 and documented
  (user's decision); the fix is on the roadmap.
- Rings are cut from the **inside out**, each starting at its point nearest to where the last
  ended. Conventional cuts the rings clockwise (and round islands anticlockwise); Climb reverses.
- The move to the next ring is fed at the stepover feed rate when it is short (up to 2 ×
  stepover) and crosses no other ring or island; otherwise the tool retracts and approaches as
  between depth levels.
- Between depth levels (and areas): retract to clearance, rapid to the start, rapid down to fast
  plunge height above the last cut level, feed down at the plunge rate.

### 6.4 Drill

| Setting | Meaning | Default | v1 |
|---|---|---|---|
| Points | Point lists (every point), circle centres and arc centres | — | v1 |
| Method | Standard (straight plunge), Peck, or Spiral | Standard | v1 |
| Retract height | Z the tool rapids down to before feeding in, and goes back to between pecks. Above the stock surface, not above the clearance plane | 3 | v1 |
| Peck distance | Depth of each peck (Peck only) | 1 | v1 |
| Hole diameter | Spiral only, for point lists and arcs; must be bigger than the tool. Circles are cut to their own diameter | 10 | v1 |
| Milling direction | Spiral only: Conventional (clockwise round the hole) / Climb | Conventional | v1 |
| Depth per turn | Spiral only: the common Depth increment, shown under this name | 1 | v1 |
| Dwell | Seconds at the bottom | 0 | later (needs a pause move; with G-code) |

Decided and built 2026-11-16 (matches the reference Drill example, written out in full, move for
move; the reference itself uses a canned cycle G81 with G98):
- **Hole order:** the first listed hole, then always the nearest hole not yet drilled.
- **Between holes** the tool always rises to the clearance plane (no sideways rapid below it).
- **Standard:** rapid over the hole at clearance, rapid down to the retract height (if lower),
  feed to the target depth at the plunge feed rate, rapid back up to clearance.
- **Peck:** as Standard, but feed down one peck distance at a time; after each peck, rapid up to
  the retract height to clear chips, rapid back down to fast plunge height above the last depth,
  and feed on.
- **Spiral:** the tool centre goes round at (hole diameter − tool diameter) / 2 from the hole
  centre. A circle is cut to its own diameter (user's decision, 2026-11-17); point lists and arcs
  use the Hole diameter setting. A circle no bigger than the tool is refused with a message. Rapid over the wall position, down to the retract height, feed to the stock surface,
  then a helix down in equal turns no deeper than the depth per turn (at the cut feed rate), one
  flat full circle at the target depth to clean the floor. Then:
  - hole no more than twice the tool diameter (the centre is cleared): feed to the hole centre,
    off the wall, and rapid straight up;
  - bigger hole: a core of material is left in the middle, so the tool must not move inward at
    the bottom (it would run into the core). It rapids straight up from the wall (user's decision,
    2026-11-17). Clearing the core is not done; use a Pocket for that.
- Depth increment (except as Spiral's depth per turn), final depth increment and roughing
  clearance don't apply to drilling and are hidden for Drill.

The exact G-code format for drilling (written out in full, or canned cycles) is decided with the
post-processors.

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
- Nothing goes below the target depth.
- Arcs never go up. They may go down as they turn (a helix, e.g. spiral drilling); changed from
  "arcs stay flat" when spiral drilling was added (user's decision, 2026-11-16).

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

None at the moment. Answered 2026-11-16: fast plunge height is a setting, default 1 mm (6.1).
Profile has no overlap setting (user's decision, 2026-11-16).

## 10. Out of scope for v1

Ideas for after v1 are kept in `docs/roadmap.md`.
