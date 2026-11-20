# Reference G-code (test files)

These files are **test data, not programs for cutting.** The engine test
`engine/tests/reference_gcode.rs` builds the same projects and checks that WHAM_CAM still writes
exactly this G-code, so any change to toolpaths or G-code output is caught on every push.

- The tools, feeds, speeds and depths are placeholders for testing. They are not set up for any
  material or any machine.
- Each file was checked by the project author in a simulator and with an air cut on their own
  machine. That does not make it safe on yours. If you want to cut a part, make your own program in
  WHAM_CAM for your tool, material and machine, and check it yourself.

| File | What it tests |
|---|---|
| `Profile.nc` | Outside profile of a polyline with arcs |
| `Pocket.nc` | Rectangular pocket in two depth passes |
| `Drill.nc` | Standard drilling (`G81` cycle) |
| `Arcs Profile Outside.nc` | Square corners, convex and concave arcs: outside, conventional |
| `Arcs Profile Inside.nc` | The same shape: inside, conventional |
| `Arcs Pocket.nc` | The same shape: pocket, conventional |

The `( WHAM_CAM x.y.z )` version line is not compared, so the files don't change with each release.

## Updating

1. Write fresh files: `cargo test -p wham_engine --test reference_gcode -- --ignored`
   (they go to `pending/` here, which git ignores).
2. Check them in a simulator and with an air cut.
3. When they are right, move them up into this folder and commit them.
