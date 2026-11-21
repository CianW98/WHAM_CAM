# WHAM_CAM known limitations

Things WHAM_CAM does not do, or does in a way that can surprise you. Planned features are in `docs/roadmap.md`.

## Toolpaths

### Pocket: uncut slivers with a stepover above 0.5

With a stepover above 0.5 × tool diameter, the rings are further apart than the tool reaches from one ring alone. Where the pocket narrows (lobes, tips, thin branches) and the next ring doesn't fit, a sliver down the middle can be left uncut. The extra-ring check works on whole rings, so it misses a branch of a larger ring.

- **What to do:** keep the stepover at 0.5 or less. Everything is cleared at 0.5 or less (checked on a complex pocket).
- The default stepover is 0.4.
- Planned fix: see the roadmap (1.4).

### Pocket: open and crossing shapes are refused

Only closed shapes (polylines, PolyRectangles, circles) can be pocketed. Open shapes are refused, and so are shapes that cross each other.

- Spec: 6.3 Pocket.

### Profile: each shape is cut on its own

There is no nesting between shapes. A hole inside an outline needs its own Inside operation. Open shapes can only be cut On line, and point lists are refused.

- Spec: 6.2 Profile.

### Profile: no start point setting

The cut starts at the shape's first point, or at the nearest point on the offset path. The start point cannot be chosen.

- Spec: 6.2 Profile.

### Helical drilling: large holes exit straight up

For holes more than twice the tool diameter, a core of material is left in the middle, so the tool rapids straight up from the wall instead of moving inward. Clearing the core needs a Pocket.

- Spec: 6.4 Drill.

### Peck drilling: re-entry height differs between preview and machine

After each peck the controller decides how far above the last depth the tool stops before feeding on (G83). The preview uses the fast plunge height, so the machine can differ slightly there.

- Spec: 7 G-code output (Drilling).

## Import and drawing

### HATCH is not imported

HATCH entities in a DXF are skipped. The import message lists everything that was skipped.

- Spec: 5.1 Import.

### Splines and ellipses become arcs and lines

On import they are converted to polylines of arcs and lines within a tolerance (0.01 mm by default, set in a dialog), so they are not exact curves. The same applies to curves in SVG files.

- Spec: 5.1 Import.

### Dimensions measure only

A dimension shows a size and never drives the geometry. It is not machined and not exported. Zoomed far out, dimension text stops shrinking, so on small features it can look large next to the feature.

- Spec: 5.3 Dimensions.

## Outside the program

### Installers are unsigned

Windows warns "unknown publisher" when you run the installer.

### No machine control

WHAM_CAM produces G-code files. It does not send them to or control a machine.

### Other CAM programs' project files cannot be imported

Import geometry as DXF or SVG instead.
