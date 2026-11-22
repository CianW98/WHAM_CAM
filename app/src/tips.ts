// The hover descriptions: every toolbar button's and Properties setting's text, in one place.

export interface ToolbarTip {
  text: string;
  /** The shortcut key, shown on the right. */
  key?: string;
}

/** Toolbar buttons, by action name. */
export const TOOLBAR_TIPS: Record<string, ToolbarTip> = {
  New: { text: "Start a new, empty project.", key: "Ctrl+N" },
  Open: { text: "Open a saved WHAM_CAM project (.wc).", key: "Ctrl+O" },
  Save: { text: "Save the project. A project not saved before asks for a file name.", key: "Ctrl+S" },
  Import: { text: "Bring in shapes from a DXF or SVG file." },
  Undo: { text: "Undo the last change to the drawing or operations.", key: "Ctrl+Z" },
  Redo: { text: "Redo the change just undone.", key: "Ctrl+Y" },
  Delete: { text: "Delete the selected shapes or operation.", key: "Del" },
  Line: { text: "Draw a straight line: click its start, then its end.", key: "L" },
  Arc: { text: "Draw an arc: click its start, its end, then its centre.", key: "A" },
  Circle: { text: "Draw a circle: click its centre, then a point on it.", key: "C" },
  Rectangle: { text: "Draw a rectangle: click two opposite corners.", key: "R" },
  Polyline: {
    text: "Draw connected lines and arcs. A for arcs, L for lines, Enter or double-click to finish.",
    key: "P",
  },
  Point: { text: "Place points (e.g. hole centres). Enter or double-click makes them one point list.", key: "." },
  Dimension: { text: "Measure a length, diameter or angle on the drawing.", key: "D" },
  Profile: { text: "Cut along the selected shapes: inside, outside or on the line." },
  Pocket: { text: "Clear out the area inside the selected closed shapes, leaving any islands." },
  Drill: { text: "Drill holes at the selected points, circle centres and arc centres." },
  "Generate toolpaths": { text: "Work out the toolpaths for every operation, and check them with the safety checker." },
  "Show toolpaths": { text: "Show or hide all toolpaths in the drawing area.", key: "T" },
  "Show cut width": { text: "Shade the band the cutter sweeps along each cut (tool diameter wide).", key: "W" },
  "Produce G-code": { text: "Write the G-code file (.nc) for the whole job." },
};

/**
 * Properties settings, by the setting's name. A name that means different things in different
 * places has its own entry, picked with the Row's `tipKey` (e.g. "Name (operation)").
 */
export const SETTING_TIPS: Record<string, string> = {
  // Shapes
  ID: "The shape's number, as shown in the tree. Operations list their shapes by these numbers.",
  Layer: "The layer this shape is on.",
  "Type (shape)": "What kind of shape this is.",
  Start: "X, Y of the start point.",
  End: "X, Y of the end point.",
  Center: "X, Y of the centre.",
  Radius: "Radius in mm.",
  "Start angle": "Angle where the arc starts, in degrees (0° = +X, anticlockwise positive).",
  "End angle": "Angle where the arc ends, in degrees (0° = +X, anticlockwise positive).",
  Closed: "Yes joins the last point back to the first.",
  Direction: "Which way round the shape goes: clockwise (CW) or anticlockwise (CCW).",
  "Number of segments": "How many lines and arcs the shape is made of.",
  Points: "Edit the points in a table.",
  "Lower left": "X, Y of the lower-left corner (before rotation).",
  Width: "Size along X in mm (before rotation).",
  Height: "Size along Y in mm (before rotation).",
  Rotation: "Turned about the lower-left corner, in degrees, anticlockwise positive.",
  "Number of points": "How many points are in the list.",

  // Dimensions
  "ID (dimension)": "The dimension's number.",
  "Type (dimension)": "What the dimension measures: aligned length, diameter, radius or angle.",
  Value: "The measured size.",

  // Layers
  "Name (layer)": "The layer's name. Double-click it in the tree to rename it there.",
  Colour: "The colour the layer's shapes are drawn in.",
  Objects: "How many shapes are on this layer.",

  // Project
  "Name (project)": "The project's name.",
  Template: "The project template the grid and snap settings come from.",
  "Minor grid spacing (mm)": "Spacing of the fine grid lines. Grid snapping uses this spacing.",
  "Major grid spacing (mm)": "Spacing of the heavier grid lines.",
  "Grid X min (mm)": "Left edge of the drawn grid.",
  "Grid X max (mm)": "Right edge of the drawn grid.",
  "Grid Y min (mm)": "Bottom edge of the drawn grid.",
  "Grid Y max (mm)": "Top edge of the drawn grid.",
  "Snap to grid": "Clicks snap to the nearest grid point while drawing.",
  "Snap to objects": "Clicks snap to end points, mid points and centres of shapes while drawing.",

  // Machining, parts
  "Tool library": "The tool library this project picks its tools from.",
  "Tool (machining)": "The tool used by every part and operation that doesn't pick its own.",
  Machine: "The machine the G-code is for. Its settings are edited in Setup.",
  "Name (part)": "The part's name.",
  "Tool (part)": "The tool for this part's operations, or use the Machining tool.",

  // Operation
  "Name (operation)": "The operation's name, written into the G-code as a comment.",
  "Type (operation)": "Profile, Pocket or Drill.",
  Enabled: "Include this operation in the G-code. A disabled operation isn't drawn either.",
  "Show toolpath": "Show this operation's toolpath in the drawing area.",
  Shapes: "The numbers of the shapes this operation cuts, e.g. 1, 2, 3.",
  "Tool (operation)": "Use the part's or Machining tool, a tool from the library, or a custom tool.",
  "Tool number": "The tool number written in the G-code (T number).",
  "Tool diameter": "Cutter diameter in mm. Toolpaths are offset by half of this.",
  "Stock surface": "Z of the top of the material.",
  "Target depth": "Final Z of the cut (negative = into the material).",
  "Depth increment": "The most material taken off in one pass down.",
  "Depth per turn": "How far the tool goes down for each turn of the helix.",
  "Final depth increment": "Size of the last pass, for a lighter finishing pass. 0 = all passes equal.",
  "Clearance plane": "Z the tool rapids at when moving between cuts.",
  "Fast plunge height":
    "Between depth levels, the tool rapids down to this far above the last cut level, then feeds down.",
  "Cut feed rate": "Feed rate while cutting, in mm/min.",
  "Plunge feed rate": "Feed rate going down into the material, in mm/min.",
  "Spindle speed": "Spindle speed in RPM.",
  "Spindle direction": "CW (normal), CCW, or Off.",
  "Roughing clearance": "Material left on the wall for a finishing pass (negative = cut past the line).",
  "Milling direction": "Conventional or climb milling (with the spindle running CW).",
  "Cut ordering":
    "With several shapes: Depth first cuts each shape to full depth in turn; Level first cuts every shape one level at a time.",
  Side: "Cut inside the shape, outside it, or on the line. Open shapes can only be cut on the line.",
  Method: "Standard (straight plunge), Peck (in steps, lifting to clear chips) or Helical (spiral down).",
  "Retract height": "Z the tool rapids down to before feeding in, and goes back to between pecks.",
  "Peck distance": "How deep each peck goes before lifting to clear chips.",
  "Hole diameter (points, arcs)":
    "Helical: the hole size cut at points and arc centres. Must be bigger than the tool. Circles are cut to their own size.",
  Stepover: "Distance between rings, as a fraction of the tool diameter (0.4 = 40%).",
  "Stepover feed rate": "Whether the moves from one ring to the next use the cut or the plunge feed rate.",

  // Custom tool (on an operation) and tools
  "End type": "The shape of the tool's end. Only end mills are supported in v1.",
  "Flute type": "Upcut (lifts chips out), Downcut (pushes chips down for a clean top edge) or Straight.",
  "Helix angle (°)": "The angle of the flutes' twist.",
  "V angle (°)": "The included angle of the V-bit's point.",
  Diameter: "Cutting diameter in mm.",
  "Shank diameter": "Diameter of the shank held in the collet. Blank = same as the cutting diameter.",
  Flutes: "Number of cutting edges.",
  "Flute length": "Length of the cutting part, in mm.",
  "Overall length": "Total length of the tool, in mm.",

  // Setup: tool libraries, machines, templates
  "Name (library)": "The library's name.",
  "Name (machine)": "The machine's name.",
  "Name (template)": "The template's name.",
  "Post-processor": "Turns toolpaths into G-code for this machine's controller.",
  "Rapid rate XY (mm/min)": "How fast the machine moves sideways on rapids. Used for the playback timing.",
  "Rapid rate Z (mm/min)": "How fast the machine moves up and down on rapids. Used for the playback timing.",
  "Spindle control": "Variable RPM, or on/off only. Recorded only: not used yet.",
  "Max spindle speed (RPM)": "The machine's top spindle speed. Recorded only: not used yet.",
  "Max feed (mm/min)": "The machine's top feed rate. Recorded only: not used yet.",
  "Tool changer": "Whether the machine changes tools itself. Recorded only: not used yet.",
  "X travel (mm)": "How far the machine can move in X (min, max). Recorded only: not used yet.",
  "Y travel (mm)": "How far the machine can move in Y (min, max). Recorded only: not used yet.",
  "Z travel (mm)": "How far the machine can move in Z (min, max). Recorded only: not used yet.",
};
