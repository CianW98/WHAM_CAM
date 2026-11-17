# WHAM_CAM roadmap (after v1)

Ideas for after v1, with the reasoning and any decisions made so far. Nothing here is being built
yet: v1 (see `docs/spec.md`) comes first. The order within each group is rough and can change.

## Soon after v1

- **Machine-specific post-processors**: RepRapFirmware/WorkBee, Grbl, LinuxCNC, Mach3, each with its
  own settings panel per machine type. v1 has one generic post-processor only.
- **Inches**, together with CAM styles/templates and workspaces. v1 is mm only.
- **Lead-ins / lead-outs.** v1 uses a straight plunge at the plunge feed rate. The reference
  Profile example uses a spiral (ramp) lead-in at 15°: plunge to the stock surface, then ramp down
  along the profile to the cut depth. It needs sloped arcs (helical G2/G3), which the safety checker
  now allows going down (added for spiral drilling, 2026-11-16).
- **Holding tabs.**
- **PolyRectangle corner radius** (rounded corners as true arcs, at most half the shorter side).
  Left out of v1 (decided 2026-11-16); the PolyRectangle has Lower Left, Width, Height, Rotation.
- **Project preferences**: a section of per-project settings saved in the project file, e.g.
  current layer, view position, snap settings and grid settings. Raised 2026-11-16 when saving
  was built; until then those aren't saved (grid spacing already is, as part of the project).
- **Recent files** list in the File menu (not wanted yet, 2026-11-16).
- **Smarter moves between depth levels.** v1 retracts to clearance, rapids to the start, rapids
  down to fast plunge height and feeds down, as in the reference. The user wants to work on this
  later (2026-11-16), e.g. staying down when the next level starts where the last ended.
- **Machine settings**, including the real rapid rate, so the tool playback's run time is accurate.
  v1 assumes 5000 mm/min for rapids (decided 2026-11-16).
- **Pocket: full clearing at stepovers over 0.5.** Above 0.5 × tool diameter, narrow parts of a
  pocket can keep an uncut sliver down the middle (spec 6.3, found 2026-11-16 at 0.7 on a complex
  pocket; 0.5 clears it). Fix idea: after each ring, work out what is still uncut area by area
  (with the offsetting library) and add a ring or a centre-line cut only there, instead of the
  current whole-ring check. Maybe also a warning in Properties for stepovers over 0.5. Changes
  toolpath output, so it needs tests and a simulator re-check.
- **Drill dwell** (seconds at the bottom of a hole). Needs a pause move through the safety
  checker, preview and playback; add it with G-code output.
- **Pocketing splines and ellipses**, by converting them to arcs within a tolerance. v1 refuses
  them with a message.

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
- **Predefined styles in the library** for common kinds of CNC work, e.g. PCB cutting, V-carving
  and other common uses. List to be filled in.

## Maybe / needs thought

These may clash with the "keep it simple" principle. Ask before pursuing.

- **3D operations.**

## Decided against

- Importing other CAM programs' project files: never wanted.
- Machine control: far out of scope.
