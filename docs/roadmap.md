# WHAM_CAM roadmap (after v1)

Ideas for after v1, with the reasoning and any decisions made so far. Nothing here is being built
yet: v1 (see `docs/spec.md`) comes first. The order within each group is rough and can change.

## Soon after v1

(Machine settings and project preferences moved into v1 on 2026-11-20: spec 5.9 and 5.10.)

- **DXF text as outlines** (moved out of v1, 2026-11-20): TEXT and MTEXT become closed outline
  shapes for each letter, on the entity's layer, to engrave, profile or pocket like any shape
  (as decided 2026-11-19). Needs a font bundled with WHAM_CAM (its licence must allow that) and
  a font reader that builds for WebAssembly: both chosen with the user when this is built. v1
  skips TEXT and MTEXT.
- **DXF dimensions** (moved out of v1, 2026-11-20): aligned, radius, diameter and angular DXF
  DIMENSION entities become WHAM_CAM dimensions on their layer, keeping the DXF's measured value
  (as decided 2026-11-19); other kinds (horizontal/vertical "rotated", ordinate) skipped and
  listed. v1 skips all DIMENSION entities.
- **Machine-specific post-processors**: RepRapFirmware/WorkBee, Grbl, LinuxCNC, Mach3, each with its
  own settings panel per machine type. v1 has one generic post-processor only, in a machine list
  in the Setup tab (decided 2026-11-20). Each is a different set of the same format choices, e.g.
  a pause (`M0`) instead of `M6`.
  - **Drill cycles as G0/G1** (a post-processor setting): write Standard/Peck holes as plain
    G0/G1 moves instead of `G81`/`G83`, for controllers and simulators without canned cycles.
    The engine keeps each hole as a cycle, so the post-processor expands it, using the same moves
    as the preview (decided 2026-11-19/20). Found 2026-11-20: CAMotics 1.3.0 doesn't simulate
    `G83` ("Not implemented: G83 (Peck Drilling)"); `G81` is fine.
- **Inches**, together with CAM styles/templates and workspaces. v1 is mm only.
- **Lead-ins / lead-outs.** v1 uses a straight plunge at the plunge feed rate. The reference
  Profile example uses a spiral (ramp) lead-in at 15°: plunge to the stock surface, then ramp down
  along the profile to the cut depth. It needs sloped arcs (helical G2/G3), which the safety checker
  now allows going down (added for spiral drilling, 2026-11-16).
- **Holding tabs.**
- **PolyRectangle corner radius** (rounded corners as true arcs, at most half the shorter side).
  Left out of v1 (decided 2026-11-16); the PolyRectangle has Lower Left, Width, Height, Rotation.
- **Recent files** list in the File menu (not wanted yet, 2026-11-16).
- **Smarter moves between depth levels.** v1 retracts to clearance, rapids to the start, rapids
  down to fast plunge height and feeds down, as in the reference. The user wants to work on this
  later (2026-11-16), e.g. staying down when the next level starts where the last ended.
- **Pocket: full clearing at stepovers over 0.5.** Above 0.5 × tool diameter, narrow parts of a
  pocket can keep an uncut sliver down the middle (spec 6.3, found 2026-11-16 at 0.7 on a complex
  pocket; 0.5 clears it). Fix idea: after each ring, work out what is still uncut area by area
  (with the offsetting library) and add a ring or a centre-line cut only there, instead of the
  current whole-ring check. Maybe also a warning in Properties for stepovers over 0.5. Changes
  toolpath output, so it needs tests and a simulator re-check.
- **Drill dwell** (seconds at the bottom of a hole), written as the **G82** canned cycle (G81 with
  a dwell, `P` = seconds). Needs a pause move through the safety checker, preview and playback,
  and G82 in the read-back check. v1 has G81 and G83 only (noted 2026-11-20).
- **Chip-break peck (G73)**: a Drill method that pecks like G83 but backs off only a little
  after each peck (chip breaking) instead of returning to the R plane; quicker in deep holes. The
  back-off is set in the controller, so the preview would use an assumed amount, as the G83
  re-entry does with the fast plunge height (noted 2026-11-20).
- **Spiral drilling: feed rate of the plunge to the stock surface.** Spiral drilling feeds from
  the retract height down to the stock surface at the plunge rate (decided 2026-11-16), which can
  be a slow feed through air when the retract height is well above the stock. To revisit, e.g.
  rapid to fast plunge height above the stock, then feed, as Pocket and Profile do (user's
  request, 2026-11-20). A toolpath change: tests and a simulator re-check.

## Later

- **Browser version.** The same interface, with the engine compiled to WebAssembly and served as
  static files. All CAM work runs on the user's own computer; nothing is processed on a server.
  Open a web address and it runs, no install.
  - Hosting (discussed 2026-11-15, decide when this is built): the repo is public (Apache 2.0), so
    **GitHub Pages** hosts it for free and is the simplest option. **Cloudflare** (Workers static
    hosting, also free) is the better choice if we want WebAssembly to use several processor cores
    (this needs server settings GitHub Pages can't set) or small online features later, such as an
    update check or sharing a project by link.
- **Material removal simulation**: a block of stock carved away as the tool plays back, showing
  the finished part, gouges and leftover material (like CAMotics). v1 has tool playback only;
  until then use CAMotics or NC Viewer on the G-code (discussed 2026-11-16).
- **Libraries: tools, styles and materials** (noted 2026-11-17).
  - **Tool library** beyond the v1 basics (spec 5.5): holders/lengths, sharing and importing
    tool sets (e.g. export/import of the tooling file).
  - **Feeds and speeds are not on tools** (decided 2026-11-19): they depend on the tool *and* the
    material, so they come with the material library (tool + material → feeds and speeds).
  - **Style library**: saved sets of operation settings ("CAM styles") to apply to an operation
    in one go, e.g. "6 mm slot in plywood". Ties in with the CAM styles noted under Inches.
  - **Material library**: materials (e.g. plywood, MDF, aluminium, acrylic) with suggested feeds,
    speeds and depths per tool, so picking a tool and a material fills in sensible values.
  - To be worked out together, keeping the "every setting visible" rule: a library only fills
    in values, which stay editable on the operation.
- **Horizontal / vertical dimensions and HATCH import**, if needed later: not chosen for v1
  (2026-11-19), which has aligned, radius/diameter and angle dimensions.
- **Predefined styles in the library** for common kinds of CNC work, e.g. PCB cutting, V-carving
  and other common uses. List to be filled in.

## Other cutters (noted 2026-11-19)

v1 records ball nose, V-bit and drill tools in the master tool list and libraries, but only end
mills (flat end) can be picked for an operation: the toolpaths assume a flat cutter. Getting the
others working:
- **Ball nose**: profile and pocket depths mean the tip; the 3D preview and cut width should
  show the round end. Mostly useful with 3D work (see below), or for fillets and grooves.
- **V-bit**: V-carving and chamfering (depth follows the width), and engraving on a line. A new
  operation type with its own settings; the cut width depends on depth.
- **Drill**: drilling holes with a real drill (Standard and Peck only, no spiral/helix), with the
  point length taken into account for through holes. Drills would then get a **point angle**
  setting (the tool picture draws 118° for now); they have no flute type or helix angle in v1
  (user's decision, 2026-11-19).
- **More flute types**, e.g. compression (upcut at the tip, downcut above) for sheet goods.
- Each one changes toolpath output: tests, the safety checker and a simulator/air-cut check.

## Maybe / needs thought

These may clash with the "keep it simple" principle. Ask before pursuing.

- **3D operations.**

## Decided against

- Importing other CAM programs' project files: never wanted.
- Machine control: far out of scope.
