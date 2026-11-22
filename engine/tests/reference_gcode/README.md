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
| `Profile_Outside_Conventional.nc` | Arc test shape (square corners, convex and concave arcs): outside, conventional |
| `Profile_Inside_Conventional.nc` | The same shape: inside, conventional |
| `Profile_On_Line_Conventional.nc` | The same shape: on the line |
| `Pocket_Conventional.nc` | The same shape: pocket, conventional |
| `Pocket_Climb.nc` | The same shape: pocket, climb |
| `Pocket_Level_First_Conventional.nc` | Two rectangles in one pocket, two depth passes, level first |
| `Pocket_Tool_Change_Conventional.nc` | Two pockets with different tools (T2 6 mm, then T1 3 mm): tool change |
| `Drill_Standard.nc` | Four holes, standard drilling (`G81` cycle) |
| `Drill_Peck.nc` | The same holes, peck drilling (`G83` cycle) |
| `Drill_Helical.nc` | The same holes, helical (8 mm holes with the 6 mm tool) |

The `( WHAM_CAM x.y.z )` version line and the `( Project: ... )` line (project name and date) are not
compared, so the files don't change with each release.

## Updating

1. Write fresh files: `cargo test -p wham_engine --test reference_gcode -- --ignored`
   (they go to `pending/` here, which git ignores).
2. Check them in a simulator and with an air cut.
3. When they are right, move them up into this folder and commit them.
