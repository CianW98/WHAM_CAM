# WHAM_CAM roadmap

## Road to 1.0.0

- **Full end-to-end check by Cian**: vastly done already; the machine dialog and project templates are what remain.
- **Plainer settings folder name**: change the app identifier (`tauri.conf.json`) so Setup settings live in `~/.config/whamcam/` (identifier `whamcam`) instead of `~/.config/io.github.cianw98.whamcam/`. Check the installers still build with a single-word identifier. Must be done before 1.0.0, so no user's settings are left in the old folder.
- **Final project file format check**: look through a saved file and settle setting names and grouping, set the format version to 1, and have the upgrade-on-open step ready for the first format change after 1.0.0.
- **More reference G-code files**, so every feature in the v1 spec has a checked file in `engine/tests/reference_gcode/`. For each one: add the job to `engine/tests/reference_gcode.rs`, write it to `pending/`, have it checked in a simulator and with an air cut, then move it up, commit it and add it to the README table there. Files to add:
  - Drill Peck (`G83`) and Drill Helical.
  - Climb versions of the Arcs jobs: Profile Outside, Profile Inside and Pocket.
  - Profile On line: a closed shape and an open shape.
  - A job with several operations and a tool change (e.g. a pocket, a profile and a drill with two tools).
- **Remove the old reference-example tests**: delete the three move-by-move comparisons with the original reference examples (`matches_the_reference_drill_move_for_move` in `engine/tests/drill.rs`, `matches_the_reference_pocket_move_for_move` in `engine/tests/pocket.rs`, `matches_the_reference_profile_loop_move_for_move` in `engine/tests/profile.rs`). The checked `Drill.nc`, `Pocket.nc` and `Profile.nc` cover the same jobs. Also remove the "Reference toolpath comparison" bullet in spec section 8 and the `tests/reference/local/` line in `CLAUDE.md`'s "Where things are".
- **Installed-app check**: install the release build on Windows and Linux and run a real job end to end (open, save, reopen, write G-code).
- **Final air cut** of a real multi-operation part with the release candidate.

## Release themes after 1.0.0

Each release has one purpose. The order is proposed.

### 1.1: Other cutters

Ball nose, V-bit and drill tools are already recorded in the tool list. This release makes them usable in operations. Each cutter needs tests, safety checker work and a simulator check.

- **V-bit**: V-carving, chamfering (depth follows width), engraving on a line. A new operation type whose cut width depends on depth. Pairs with the text outlines in 1.3.
- **Real drill bits**: Standard and Peck only, point length counted for through holes, new point angle setting (the picture draws 118° for now).
- **Ball nose**: depths mean the tip; 3D preview and cut width show the round end. Mainly useful with 3D work, also fillets and grooves.
- **More flute types**, e.g. compression (upcut tip, downcut above) for sheet goods.

### 1.2: Libraries, materials and stock

A library only fills in values, which stay editable on the operation.

- **Material library**: plywood, MDF, aluminium, acrylic, etc. Initially it drives feed rates and speeds. Feeds and speeds are not stored on tools. A project's material is assigned in the stock's preferences.
- **Template library**: saved sets of operation settings. This covers predefined templates for common work such as V-carving. Proposal:
  - A template holds one operation: its type and all its settings, with no geometry. Applying it to the selected shapes creates the operation in one go, and every value stays editable.
  - A template can also hold a group of operations, e.g. a profile and a drill for the same part, applied together to the selection.
  - Templates are saved from an existing operation or configured with a dialog, and live on the computer like tool libraries, in named libraries that export and import as files.
  - Feeds and speeds are not stored in a template. They fill in from the material library, so one template works across different materials.
  - Predefined templates ship for common work such as V-carving.
- **Tool library additions**: add a working length to each tool, meaning how much of the tool is held. Also export and import of tool sets (the tooling file).
- **More machine library and Setup features**, building on the 1.0.0 machine dialog.
- **UI stock**: a wireframe of the raw material you will cut into, shown in the drawing area as a 3D box in X, Y and Z, so it shows the depth of the material and not only a flat outline. Stock has its own place in the project tree. Its preferences are where the material is assigned, picked from the material library.

### 1.3: Import and drawing completeness

- **DXF text as outlines**: TEXT and MTEXT become closed outline shapes per letter on the entity's layer, to engrave, profile or pocket. Decision needed: which font to bundle and which font reader.
- **DXF dimensions**: more research is needed first, using DXFs exported from other programs, to see how their dimensions come through and how WHAM_CAM should handle them. The starting plan is to turn aligned, radius, diameter and angular DIMENSION entities into WHAM_CAM dimensions keeping the DXF's measured value, and to skip and list the other kinds.
- **PolyRectangle corner radius**: a corner radius property on the PolyRectangle itself, next to Lower Left, Width, Height and Rotation. Rounded corners are true arcs, at most half the shorter side.
- **Polygon draw tool**: a regular polygon from its centre, then a corner (setting size and rotation). Kept as a polygon, like the PolyRectangle, with Centre, Sides, Radius and Rotation editable in Properties.
- **Flatten layers**: move every shape on the chosen layers onto one layer and remove the emptied layers, e.g. after a DXF import with many layers.
- **Grouping**: group drawing objects so one click selects, moves and transforms them together, and ungroup them again. Shown in the Project tree.
- **Show an operation's shapes**: selecting an operation in the Project tree highlights the shapes it applies to in the tree.
- **Measure tool**: click two points (snapping as when drawing) to read the distance, X and Y difference and angle, without adding anything to the drawing. Unlike a dimension, which stays in the project.
- **Recent files** in the File menu.

### 1.4: Toolpath quality

Everything here changes output, so batch it and re-verify once per release.

- **Roughing and finishing passes**, with the logic for each. Includes the Pocket finish stepover (a light final wall pass).
- **Pocket: full clearing above 0.5 stepover.** After each ring, work out what is still uncut area by area and add a ring or centre-line cut only there. A warning in Properties above 0.5 could come first.
- **Pocket fill styles**: one Fill style setting with the choices Offset, inside → out (the current style); Offset, outside → in; Horizontal hatch; Vertical hatch.
- **Nesting**: automatic arrangement of several copies of the same object, with automatic or manual placement.
- **Helical drilling**: rapid to the fast plunge height above the stock and start the helical entry from there, as Pocket and Profile approach their cuts, instead of feeding from the retract height to the stock surface. Checked against the spec only, so confirm in the code before building.
- **Lead-ins and lead-outs**: spiral, ramp and tangent. Needs helical G2/G3, which the safety checker already allows going down.
- **Start point** for Profile and Pocket: choose where the cut starts on each shape. For Profile, instead of the shape's first point; for Pocket, where the first (innermost) ring is entered. Fits with lead-ins and lead-outs, which start there.
- **Holding tabs** for Profile.
- **Smarter moves between depth levels**: e.g. stay down when the next level starts where the last one ended.
- **Other operation settings**: Profile cut width (multi-pass wall width), corner overcut (dog-bone relief), per-operation custom header/footer G-code.

### 1.5: Controllers and post-processors

- **Machine-specific post-processors**: Grbl, LinuxCNC, Mach3, RepRapFirmware. The post-processor is assigned in the machine dialog. This is a substantial feature and needs a thorough analysis of exactly what changes per controller before it is built. No machine-specific G-code outside a post-processor.
- **Drill cycles as G0/G1**: a post-processor setting that writes Standard and Peck holes as plain moves instead of G81/G83, for controllers without canned cycles. The engine keeps holes as cycles; the post-processor expands them with the same moves as the preview.
- **Arc centre mode**: a post-processor setting for absolute or relative (incremental) arc centres, `I J` as the centre's position or as an offset from the arc's start (the current output). Some controllers need it stated in the header (`G90.1` / `G91.1`). The read-back check must read both.
- **Drill dwell** via G82: needs a pause move through the safety checker, preview and playback, and G82 in the read-back check.
- **Chip-break peck (G73)**: pecks like G83 but backs off only a little. For G73 the back-off is set in the controller, so it may be better to handle this with custom G1/G0 moves for maximum control.

## Later (no release assigned)

- **Inches**, together with templates and workspaces.
- **Material removal simulation**: a stock block carved away during playback, showing gouges and leftover material.
- **Full 3D operations**: they will come at some stage.
- **Chip-load and surface speed calculations**: feeds and speeds worked out from tool size, tool type and material type, building on the material library (1.2). Spindle speed from surface speed (m/min) and tool diameter; feed rate from chip load (feed per tooth), flutes and spindle speed.
- **Collision detection**, using the tool working length added in 1.2.
- **Browser version**: same interface, engine compiled to WebAssembly, served as static files, all work on the user's own computer, nothing on a server. CI already builds the engine for `wasm32-unknown-unknown`. Decision when built: hosting on GitHub Pages (free, simplest) or Cloudflare Workers static hosting (free; allows WebAssembly multi-core and small online features such as an update check or share-by-link). The repo is public under Apache 2.0.
- **MCP control**: an MCP (Model Context Protocol) server so an AI assistant can drive WHAM_CAM: import or draw geometry, apply operations, change settings and write G-code. It goes through the same engine calls as the interface, and every toolpath still goes through the safety checker.
- **Thread milling**: internal and external threads on circles, a helical path per thread pitch using a thread mill, with pitch, thread depth, hand (right or left) and climb/conventional. Needs a thread mill tool type in the tool library.
- **Inlay toolpaths**: cut a matching pocket and plug from one shape. The pocket's inside corners can't be sharper than the cutter radius, so the plug's matching corners are rounded by the same radius and the two fit.
- **Toolbar customisation**: choose which buttons show on the toolbar and in what order, kept in the Setup settings on this computer, with a reset to the default layout. Every action stays reachable from the menus.
- **Profile modifiers**: shape the cut wall of a Profile operation so a 2D outline produces a sloped or rounded edge instead of a vertical one. The user picks a method (slope, convex radius or concave radius) and a value (an angle from vertical, or a radius). Each depth pass then follows the contour offset for its depth. For chamfered and bevelled edges, filleted edges and cove cuts using suitable tool shapes.

## Decided against

- Importing other CAM programs' project files.
- Physical machine control.
