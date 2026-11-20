// The connection layer between the interface and the CAM engine.
//
// This is the ONLY file in the interface that talks to the engine or to Tauri. Today it goes
// through Tauri; a future browser build swaps this file for a WebAssembly version and nothing
// else in the interface changes. The types mirror the engine's Rust types.

import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open, save } from "@tauri-apps/plugin-dialog";

export interface Point {
  x: number;
  y: number;
}

export interface Vertex {
  x: number;
  y: number;
  /** 0 = straight segment to the next vertex, otherwise tan(sweep / 4). */
  bulge: number;
}

export type Shape =
  | { type: "Line"; start: Point; end: Point }
  | { type: "Arc"; center: Point; radius: number; start_angle: number; end_angle: number }
  | { type: "Circle"; center: Point; radius: number }
  | { type: "Polyline"; vertices: Vertex[]; closed: boolean }
  /** A "PolyRectangle": width along `rotation` degrees CCW from +X, height at right angles. */
  | { type: "Rectangle"; lower_left: Point; width: number; height: number; rotation: number }
  | { type: "Points"; points: Point[] }
  | ({ type: "Dimension" } & Dimension);

/** A dimension (measure only, never machined). `value` is mm, or degrees for an angle. */
export type Dimension =
  | { kind: "Aligned"; from: Point; to: Point; at: Point; value: number }
  | { kind: "Radius"; center: Point; radius: number; at: Point; value: number }
  | { kind: "Diameter"; center: Point; radius: number; at: Point; value: number }
  | { kind: "Angle"; vertex: Point; start: Point; end: Point; value: number };

export interface DrawingObject {
  id: number;
  shape: Shape;
}

export interface Layer {
  name: string;
  /** Display colour for everything on the layer, "#rrggbb". */
  color: string;
  objects: DrawingObject[];
}

export interface Part {
  name: string;
  /** The part's own tool; null = use the Machining tool. */
  tool: PickedTool | null;
  operations: Operation[];
}

/** A tool picked from a library: its number there and a copy of the tool (kept in the project). */
export interface PickedTool {
  number: number;
  tool: Tool;
}

// --- Operations (spec section 6) ----------------------------------------------------------------

export type SpindleDirection = "CW" | "CCW" | "Off";
/** Spindle CW: Conventional cuts inside walls clockwise, outside walls anticlockwise; Climb the reverse. */
export type MillingDirection = "Conventional" | "Climb";
export type CutOrdering = "DepthFirst" | "LevelFirst";
export type StepoverFeed = "Plunge" | "Cut";

/** Settings common to all operations. Lengths in mm, feeds in mm/min. */
export interface CommonSettings {
  tool_number: number;
  tool_diameter: number;
  /** Inherit = the part's tool, or Machining's (the tool fields are then ignored). */
  tool_source: "Inherit" | "Library" | "Custom";
  /** Own tool (Library: a copy kept in the project; Custom: the operation's own values). */
  tool: Tool | null;
  stock_surface: number;
  /** Null until set. */
  target_depth: number | null;
  depth_increment: number;
  final_depth_increment: number;
  clearance_plane: number;
  fast_plunge_height: number;
  cut_feedrate: number;
  plunge_feedrate: number;
  spindle_speed: number;
  spindle_direction: SpindleDirection;
  roughing_clearance: number;
}

export interface PocketSettings {
  /** Fraction of the tool diameter. */
  stepover: number;
  stepover_feed: StepoverFeed;
  milling_direction: MillingDirection;
  cut_ordering: CutOrdering;
}

export type ProfileSide = "Inside" | "Outside" | "OnLine";

export interface ProfileSettings {
  side: ProfileSide;
  milling_direction: MillingDirection;
  cut_ordering: CutOrdering;
}

export type DrillMethod = "Standard" | "Peck" | "Spiral";

export interface DrillSettings {
  method: DrillMethod;
  /** Z to rapid down to before feeding into a hole, and to go back to between pecks. */
  retract_height: number;
  /** Depth of each peck (Peck only). */
  peck_distance: number;
  /** Hole diameter (Spiral only; bigger than the tool) for point lists and arcs. Circles use their own. */
  hole_diameter: number;
  /** Direction round the hole (Spiral only). */
  milling_direction: MillingDirection;
}

export type OperationKind =
  | ({ type: "Profile" } & ProfileSettings)
  | ({ type: "Pocket" } & PocketSettings)
  | ({ type: "Drill" } & DrillSettings);

export interface Operation {
  id: number;
  name: string;
  enabled: boolean;
  show_toolpath: boolean;
  shape_ids: number[];
  common: CommonSettings;
  kind: OperationKind;
}

export interface OperationState {
  id: number;
  status: "none" | "ok" | "outdated" | "error";
  message: string | null;
  /** It machines open shapes: a Profile then only offers On line. */
  has_open_shapes: boolean;
  /** Where its tool comes from; "none" = follows the tree but no tool is set there. */
  tool_from: "operation" | "part" | "machining" | "none";
  tool_number: number | null;
  tool_diameter: number | null;
  /** The tool's name (null for none). */
  tool_name: string | null;
}

export type MoveKind = "Cut" | "Plunge" | "Rapid";

export interface ToolpathDisplay {
  operation_id: number;
  runs: { kind: MoveKind; points: [number, number, number][] }[];
}

/** The band the cutter sweeps along one playback step, as flat triangles (empty if it doesn't cut). */
export interface CutWidthStep {
  /** x, y, z of each triangle corner, three corners per triangle. */
  triangles: number[];
  /** For each segment of the step's points, the triangle its strip starts at. */
  segment_starts: number[];
}

/** One move of the tool playback (times in seconds, points in mm). */
export interface PlaybackStep {
  operation_id: number;
  /** The move's number in its operation (1 = first), as in safety messages. */
  move_number: number;
  kind: MoveKind;
  /** mm/min; null for rapids. */
  feed: number | null;
  tool_diameter: number;
  points: [number, number, number][];
  start_s: number;
  duration_s: number;
}

/** Every shown toolpath one after another, for the tool playback. */
export interface Playback {
  steps: PlaybackStep[];
  total_s: number;
  /** Each operation's cutter, to draw in 3D. */
  tools: PlaybackTool[];
}

/** One operation's cutter for the 3D playback: its outline turned round its axis. */
export interface PlaybackTool {
  operation_id: number;
  /** Tip on the axis, out and up the side, to the shank top on the axis (x = radius, y = height, mm). */
  profile: Point[];
  /** Height where the flutes end. */
  flute_length: number;
}

export interface GenerateReport {
  generated: number;
  /** [operation name, error]. */
  errors: [string, string][];
  /** Things to know that didn't stop generating (e.g. the tool library isn't on this computer). */
  warnings: string[];
}

export interface Document {
  name: string;
  layers: Layer[];
  parts: Part[];
  /** Drawing grid spacing in mm. */
  grid_spacing: number;
  /** Name of the tool library operations pick tools from; null = not chosen. */
  active_library: string | null;
  /** The tool every part and operation uses unless it has its own; null = none. */
  machining_tool: PickedTool | null;
  /** The machine the G-code is for (by name); null = the generic post-processor. */
  machine: string | null;
}

// --- Machines (spec 5.7) ------------------------------------------------------------------------

export type PostProcessor = "Generic";

export interface Machine {
  name: string;
  post: PostProcessor;
}

/** This computer's machines. */
export interface Machines {
  machines: Machine[];
}

// --- Tooling (spec 5.5) -------------------------------------------------------------------------

export type EndType = "Flat" | "Ball" | "VBit" | "Drill";
/** Spiral = right-hand (upcut); Downcut = reverse spiral. */
export type FluteType = "Spiral" | "Straight" | "Downcut";

/** A physical tool in the master tool list. Lengths in mm. */
export interface Tool {
  id: number;
  name: string;
  end_type: EndType;
  diameter: number;
  /** Null = same as the diameter. */
  shank_diameter: number | null;
  flutes: number;
  flute_type: FluteType;
  /** Helix angle in degrees (Spiral and Downcut; not drills). */
  helix_angle: number;
  flute_length: number;
  overall_length: number;
  /** Included angle (V-bit only). */
  v_angle: number;
  /** Name made from the values (and kept up to date); false once a name is typed in. */
  auto_name: boolean;
}

/** Side view of a tool, tip down at (0, 0), in mm. */
export interface ToolDrawing {
  /** Closed silhouette. */
  outline: Point[];
  /** Height where the flutes end. */
  flute_top: number;
  /** Flute radius at that height. */
  flute_top_radius: number;
  /** Cutting edges seen on the flutes, each a line through its points. */
  flute_lines: Point[][];
}

/** What the tool form shows while being filled in. */
export interface ToolPreview {
  name: string;
  /** Null while the values aren't valid. */
  drawing: ToolDrawing | null;
}

/** A master tool in a library, with its tool number there. */
export interface LibraryTool {
  number: number;
  tool_id: number;
}

export interface ToolLibrary {
  name: string;
  /** In tool number order. */
  tools: LibraryTool[];
}

/** This computer's master tool list and tool libraries. */
export interface Tooling {
  tools: Tool[];
  libraries: ToolLibrary[];
}

export interface DisplayShape {
  id: number;
  /** [x, y] pairs in mm, ready to draw. */
  points: [number, number][];
  closed: boolean;
  /** Separate point markers rather than a connected path. */
  markers: boolean;
  /** Colour to draw it in, "#rrggbb" (its layer's colour). */
  color: string;
  /** Which way a closed polyline or rectangle runs; null for other shapes. */
  direction: "CW" | "CCW" | null;
  /** More open paths drawn the same way (a dimension's lines and arrowheads). */
  extra: [number, number][][];
  /** Text to draw (a dimension's value). */
  label: Label | null;
}

/** Text placed on the drawing (a dimension's value). */
export interface Label {
  text: string;
  /** Centre of the text, in mm. */
  at: [number, number];
  /** Baseline direction, degrees anticlockwise from +X. */
  angle: number;
  /** Text height in mm. */
  height: number;
}

export interface DisplayData {
  shapes: DisplayShape[];
  /** [minX, minY, maxX, maxY], or null for an empty drawing. */
  bounds: [number, number, number, number] | null;
}

/** Everything needed to redraw after a change. */
export interface View {
  document: Document;
  display: DisplayData;
  canUndo: boolean;
  canRedo: boolean;
  /** Unsaved changes since the last save or open. */
  modified: boolean;
  /** The project file's name (e.g. "bracket.whamcam"), or null if never saved. */
  fileName: string | null;
  /** Toolpath state of each operation. */
  operations: OperationState[];
  /** Toolpaths to draw. */
  toolpaths: ToolpathDisplay[];
  /** This computer's master tool list and tool libraries. */
  tooling: Tooling;
  /** Layers hidden from view (not saved). */
  hiddenLayers: string[];
  /** This computer's machines. */
  machines: Machines;
}

export interface DxfUnits {
  code: number;
  name: string;
  mm_per_unit: number | null;
}

export interface DxfInfo {
  units: DxfUnits;
  shape_count: number;
  /** Splines and ellipses, which are converted into polylines on import. */
  curve_count: number;
}

export interface ImportReport {
  objects_added: number;
  layers: string[];
  skipped: Record<string, number>;
  scale: number;
  /** Splines and ellipses converted into polylines of arcs and lines. */
  curves_converted: number;
}

export type BoxMode = "Window" | "Crossing";

export type DrawTool =
  | "Line"
  | "Arc"
  | "Circle"
  | "Rectangle"
  | "Polyline"
  | "Points"
  /** One tool for every dimension: the kind comes from what is clicked (see `dimensionClick`). */
  | "Dimension";

/** An object picked by a dimension click. */
export interface Pick {
  id: number;
  kind: "Circle" | "Line";
}

/** What a Dimension tool click is, from the engine. */
export interface DimensionClick {
  /** The snapped point, or the click itself for a pick. */
  point: Point;
  /** What a point snapped to ("None" for a pick). */
  kind: SnapKind;
  pick: Pick | null;
  /** This click completes the dimension. */
  finishes: boolean;
}

/** How many clicks finish each tool's shape; null = finished with Enter (polyline, points). */
export const CLICKS_NEEDED: Record<DrawTool, number | null> = {
  Line: 2,
  Circle: 2,
  Rectangle: 2,
  Arc: 3,
  Polyline: null,
  Points: null,
  Dimension: null, // the engine says when it is complete
};


/** A shape being drawn: the points clicked so far. */
export interface Draft {
  tool: DrawTool;
  points: Point[];
  /** Polyline: arcs[i] = segment from points[i] to points[i+1] is a tangent arc. */
  arcs: boolean[];
  /** Polyline: the next segment (to the cursor) is a tangent arc. */
  arc_next: boolean;
  /** Dimension: for each click in `points`, the object it picked, or null for a point. */
  picks: (Pick | null)[];
}

export type SnapKind = "None" | "Grid" | "End" | "Mid" | "Centre";

export interface Snap {
  point: Point;
  kind: SnapKind;
}

/** Rotate, scale and mirror work around the centre of the selected shapes. */
export type TransformOp =
  | { type: "Move"; dx: number; dy: number }
  | { type: "Rotate"; degrees: number }
  /** Turn by the angle from the direction of `from` to that of `to`, seen from the centre. */
  | { type: "RotateFromTo"; from: Point; to: Point }
  | { type: "Scale"; factor: number }
  | { type: "MirrorLeftRight" }
  | { type: "MirrorTopBottom" };

export interface SnapOptions {
  grid: boolean;
  objects: boolean;
}

export function getView(): Promise<View> {
  return invoke<View>("get_view");
}

/** Shows the file-open dialog for DXF files. Returns the chosen path, or null if cancelled. */
// --- Project files ---------------------------------------------------------------------------

const PROJECT_FILTER = { name: "WHAM_CAM projects", extensions: ["whamcam"] };

/** Asks which project file to open. Null if cancelled. */
export async function chooseProjectFile(): Promise<string | null> {
  const path = await open({ title: "Open project", multiple: false, directory: false, filters: [PROJECT_FILTER] });
  return typeof path === "string" ? path : null;
}

/** Asks where to save the project. Null if cancelled. */
export async function chooseSavePath(defaultName: string): Promise<string | null> {
  return (await save({ title: "Save project as", defaultPath: defaultName, filters: [PROJECT_FILTER] })) ?? null;
}

// --- G-code ----------------------------------------------------------------------------------

/** Checks the G-code can be written. Resolves to the suggested file name; rejects with the reason. */
export function checkGcode(date: string): Promise<string> {
  return invoke<string>("check_gcode", { date });
}

/** Asks where to save the G-code. Null if cancelled. */
export async function chooseGcodePath(defaultName: string): Promise<string | null> {
  return (
    (await save({
      title: "Produce G-code",
      defaultPath: defaultName,
      filters: [{ name: "G-code", extensions: ["nc"] }],
    })) ?? null
  );
}

/** Writes the G-code to `path`. Resolves to the file name written. */
export function saveGcode(path: string, date: string): Promise<string> {
  return invoke<string>("save_gcode", { path, date });
}

/** Sets the window's title bar text. */
export function setWindowTitle(title: string): Promise<void> {
  return getCurrentWindow().setTitle(title);
}

/**
 * Runs `canClose` when the user tries to close the window; the window only closes if it
 * resolves true. Returns a function that stops listening.
 */
export function onCloseRequested(canClose: () => Promise<boolean>): Promise<() => void> {
  return getCurrentWindow().onCloseRequested(async (event) => {
    if (!(await canClose())) event.preventDefault();
  });
}

// --- Tooling ---------------------------------------------------------------------------------
// The master tool list and libraries belong to this computer and are saved straight away
// (not undo steps). The active library and an operation's tool are part of the project.

/** Problems found loading the tool list and machine list (each reported once). */
export function settingsProblems(): Promise<string[]> {
  return invoke<string[]>("settings_problems");
}

/** Adds a machine ("Machine 1", ...) with the generic post-processor. Returns its name. */
export function addMachine(): Promise<string> {
  return invoke<string>("add_machine");
}

/** Renames a machine. Resolves to the name as stored. */
export function renameMachine(oldName: string, newName: string): Promise<string> {
  return invoke<string>("rename_machine", { old: oldName, new: newName });
}

export function deleteMachine(name: string): Promise<boolean> {
  return invoke<boolean>("delete_machine", { name });
}

export function setMachinePost(name: string, post: PostProcessor): Promise<void> {
  return invoke<void>("set_machine_post", { name, post });
}

/** Sets the project's machine (null = the generic post-processor). An undo step. */
export function setProjectMachine(name: string | null): Promise<void> {
  return invoke<void>("set_project_machine", { name });
}

/** Starting values for a new tool. */
export function newTool(): Promise<Tool> {
  return invoke<Tool>("new_tool");
}

/** The name and side view a tool would have (for the tool form; changes nothing). */
export function toolPreview(tool: Tool): Promise<ToolPreview> {
  return invoke<ToolPreview>("tool_preview", { tool });
}

/** Adds a tool to the master list. Returns its id. Rejects with a plain-English message. */
export function addTool(tool: Tool): Promise<number> {
  return invoke<number>("add_tool", { tool });
}

export function setTool(tool: Tool): Promise<void> {
  return invoke<void>("set_tool", { tool });
}

/** Deletes a master tool and takes it out of every library. */
export function deleteTool(id: number): Promise<boolean> {
  return invoke<boolean>("delete_tool", { id });
}

/** Adds an empty library ("Library 1", ...). Returns its name. */
export function addLibrary(): Promise<string> {
  return invoke<string>("add_library");
}

/** Renames a library. Resolves to the name as stored. */
export function renameLibrary(oldName: string, newName: string): Promise<string> {
  return invoke<string>("rename_library", { old: oldName, new: newName });
}

export function deleteLibrary(name: string): Promise<boolean> {
  return invoke<boolean>("delete_library", { name });
}

export function addLibraryTool(library: string, toolId: number, number: number): Promise<void> {
  return invoke<void>("add_library_tool", { library, toolId, number });
}

export function setLibraryToolNumber(library: string, oldNumber: number, newNumber: number): Promise<void> {
  return invoke<void>("set_library_tool_number", { library, old: oldNumber, new: newNumber });
}

export function removeLibraryTool(library: string, number: number): Promise<boolean> {
  return invoke<boolean>("remove_library_tool", { library, number });
}

/** Sets the project's active tool library (null = none). An undo step. */
export function setActiveLibrary(name: string | null): Promise<void> {
  return invoke<void>("set_active_library", { name });
}

/** How an operation gets its tool: from its part/Machining, a custom tool, or a library tool number. */
export type OperationTool = "Inherit" | "Custom" | { Library: number };

/** Sets how an operation gets its tool. An undo step. */
export function setOperationTool(id: number, tool: OperationTool): Promise<void> {
  return invoke<void>("set_operation_tool", { id, tool });
}

/** Sets the Machining tool to tool `number` of the active library (null = none). An undo step. */
export function setMachiningTool(number: number | null): Promise<void> {
  return invoke<void>("set_machining_tool", { number });
}

/** Sets a part's own tool (null = use the Machining tool). An undo step. */
export function setPartTool(part: number, number: number | null): Promise<void> {
  return invoke<void>("set_part_tool", { part, number });
}

// --- Operations ------------------------------------------------------------------------------

/** Adds a Pocket of the given closed shapes. Returns its id. */
/** Adds a Profile of the given shapes (Outside, or On line if any are open). Returns its id. */
export function addProfile(shapeIds: number[]): Promise<number> {
  return invoke<number>("add_profile", { shapeIds });
}

/** Adds a Drill of the given point lists, circles and arcs. Returns its id. */
export function addDrill(shapeIds: number[]): Promise<number> {
  return invoke<number>("add_drill", { shapeIds });
}

export function addPocket(shapeIds: number[]): Promise<number> {
  return invoke<number>("add_pocket", { shapeIds });
}

/** Replaces an operation's settings (matched by id). */
export function setOperation(op: Operation): Promise<void> {
  return invoke<void>("set_operation", { op });
}

export function setOperationShapes(id: number, shapeIds: number[]): Promise<void> {
  return invoke<void>("set_operation_shapes", { id, shapeIds });
}

export function deleteOperation(id: number): Promise<boolean> {
  return invoke<boolean>("delete_operation", { id });
}

/** Generates toolpaths for the given operations (all enabled ones if empty). */
export function generateToolpaths(ids: number[]): Promise<GenerateReport> {
  return invoke<GenerateReport>("generate_toolpaths", { ids });
}

/** The cut width of each step of the playback (same steps, same order). */
export function getCutWidths(): Promise<CutWidthStep[]> {
  return invoke<CutWidthStep[]>("cut_widths");
}

/** The tool playback of the drawn toolpaths. */
export function getPlayback(): Promise<Playback> {
  return invoke<Playback>("toolpath_playback");
}

/** Starts a new, empty project. */
export function newProject(): Promise<void> {
  return invoke<void>("new_project");
}

/** Opens a project file, replacing the current project. Fails with a plain-English message. */
export function openProject(path: string): Promise<void> {
  return invoke<void>("open_project", { path });
}

/** Saves to `path` (".whamcam" added if missing), or to the current file if null. */
export function saveProject(path: string | null): Promise<void> {
  return invoke<void>("save_project", { path });
}

export async function chooseImportFile(): Promise<string | null> {
  const path = await open({
    title: "Import a drawing",
    multiple: false,
    directory: false,
    filters: [
      { name: "DXF or SVG drawings", extensions: ["dxf", "DXF", "svg", "SVG"] },
      { name: "DXF drawings", extensions: ["dxf", "DXF"] },
      { name: "SVG drawings", extensions: ["svg", "SVG"] },
    ],
  });
  return typeof path === "string" ? path : null;
}

/** What an SVG file holds, before importing it. */
export interface SvgInfo {
  /** The file gives its real size; otherwise import asks how big one unit is. */
  size_known: boolean;
  shape_count: number;
  /** Curves that will be converted into arcs and lines. */
  curve_count: number;
}

export function svgInfo(path: string): Promise<SvgInfo> {
  return invoke<SvgInfo>("svg_info", { path });
}

/**
 * Imports an SVG. `mmPerUnit` is used only if the file doesn't give its real size; curves become
 * arcs and lines within `curveTolerance` mm; shapes outside Inkscape layers go on `defaultLayer`.
 */
export function importSvg(
  path: string,
  mmPerUnit: number,
  curveTolerance: number,
  defaultLayer: string,
): Promise<ImportReport> {
  return invoke<ImportReport>("import_svg", { path, mmPerUnit, curveTolerance, defaultLayer });
}

export function dxfInfo(path: string): Promise<DxfInfo> {
  return invoke<DxfInfo>("dxf_info", { path });
}

/**
 * `scale` multiplies every coordinate: 1 = numbers are mm, 25.4 = numbers are inches.
 * Splines and ellipses become polylines of arcs and lines within `curveTolerance` mm.
 */
export function importDxf(path: string, scale: number, curveTolerance: number): Promise<ImportReport> {
  return invoke<ImportReport>("import_dxf", { path, scale, curveTolerance });
}

export function deleteObjects(ids: number[]): Promise<number> {
  return invoke<number>("delete_objects", { ids });
}

/** Deletes a layer and everything on it. */
/** Adds a new empty layer ("Layer 1", "Layer 2", ...). Returns its name. */
export function addLayer(): Promise<string> {
  return invoke<string>("add_layer");
}

export function deleteLayer(name: string): Promise<boolean> {
  return invoke<boolean>("delete_layer", { name });
}

/** Renames a layer. Rejects with a plain-English message if the name isn't allowed. */
export function renameLayer(oldName: string, newName: string): Promise<void> {
  return invoke<void>("rename_layer", { old: oldName, new: newName });
}

/** Sets a layer's colour ("#rrggbb"). */
export function setLayerColor(name: string, color: string): Promise<void> {
  return invoke<void>("set_layer_color", { name, color });
}

/** Shows or hides a layer (a view setting: not saved, not an undo step). */
export function setLayerVisible(name: string, visible: boolean): Promise<void> {
  return invoke<void>("set_layer_visible", { name, visible });
}

export function undo(): Promise<boolean> {
  return invoke<boolean>("undo");
}

export function redo(): Promise<boolean> {
  return invoke<boolean>("redo");
}

/** The object nearest to (x, y) within `tolerance` mm, or null. */
export function pick(x: number, y: number, tolerance: number): Promise<number | null> {
  return invoke<number | null>("pick", { x, y, tolerance });
}

export function pickBox(x0: number, y0: number, x1: number, y1: number, mode: BoxMode): Promise<number[]> {
  return invoke<number[]>("pick_box", { x0, y0, x1, y1, mode });
}

/** Snaps a cursor position. `extra` are points of the shape being drawn. */
export function snap(x: number, y: number, tolerance: number, options: SnapOptions, extra: Point[]): Promise<Snap> {
  return invoke<Snap>("snap", { x, y, tolerance, options, extra });
}

/**
 * What a Dimension tool click at (x, y) would be, given the clicks so far: a snapped point or a
 * picked circle/arc/line, and whether it completes the dimension. Also used while hovering.
 */
export function dimensionClick(
  draft: Draft,
  x: number,
  y: number,
  tolerance: number,
  options: SnapOptions,
): Promise<DimensionClick> {
  return invoke<DimensionClick>("dimension_click", { draft, x, y, tolerance, options });
}

/** The rubber-band preview of a shape being drawn, with the cursor as its next point. */
export function previewShape(draft: Draft, cursor: Point | null): Promise<DisplayShape | null> {
  return invoke<DisplayShape | null>("preview_shape", { draft, cursor });
}

/** Finishes a drawn shape and adds it to a layer. Returns the new object's id. */
export function addDraft(layer: string, draft: Draft): Promise<number> {
  return invoke<number>("add_draft", { layer, draft });
}

/** Replaces an object's shape. Rejects with a plain-English message if the shape isn't valid. */
export function setShape(id: number, shape: Shape): Promise<void> {
  return invoke<void>("set_shape", { id, shape });
}

export function setGridSpacing(mm: number): Promise<void> {
  return invoke<void>("set_grid_spacing", { mm });
}

/** Moves, rotates, scales or mirrors objects. Rejects with a plain-English message. */
export function transform(ids: number[], op: TransformOp): Promise<void> {
  return invoke<void>("transform", { ids, op });
}

/** Result of a join. */
export interface JoinReport {
  /** The new polylines' ids. */
  ids: number[];
  /** How many shapes went into them. */
  joined: number;
  /** How many of the new polylines are closed. */
  closed: number;
  /** Selected shapes left as they were. */
  skipped: number;
}

/** Result of breaking at intersections. */
export interface BreakReport {
  /** The pieces' ids. */
  ids: number[];
  /** How many shapes were broken. */
  broken: number;
  /** How many distinct crossing points were found. */
  crossings: number;
}

/** Joins lines, arcs and open polylines whose ends are within `tolerance` mm into polylines. */
export function join(ids: number[], tolerance: number): Promise<JoinReport> {
  return invoke<JoinReport>("join", { ids, tolerance });
}

/** Breaks polylines and rectangles into lines and arcs. Returns the pieces' ids. */
export function explode(ids: number[]): Promise<number[]> {
  return invoke<number[]>("explode", { ids });
}

/** Closes open polylines. Returns how many were closed. */
export function closePolylines(ids: number[]): Promise<number> {
  return invoke<number>("close_polylines", { ids });
}

/** Breaks shapes where they cross or touch each other. */
export function breakAtIntersections(ids: number[]): Promise<BreakReport> {
  return invoke<BreakReport>("break_at_intersections", { ids });
}

/** What `transform` would give, for a preview. Changes nothing. */
export function transformPreview(ids: number[], op: TransformOp): Promise<DisplayShape[]> {
  return invoke<DisplayShape[]>("transform_preview", { ids, op });
}

/** Copies objects to the app's clipboard. Returns how many are on the clipboard. */
export function copy(ids: number[]): Promise<number> {
  return invoke<number>("copy", { ids });
}

/** Copies objects to the clipboard, then deletes them. Returns how many were cut. */
export function cut(ids: number[]): Promise<number> {
  return invoke<number>("cut", { ids });
}

export function hasClipboard(): Promise<boolean> {
  return invoke<boolean>("has_clipboard");
}

/** Preview of pasting with the clipboard's centre at `at`. */
export function pastePreview(at: Point): Promise<DisplayShape[]> {
  return invoke<DisplayShape[]>("paste_preview", { at });
}

/** Pastes with the clipboard's centre at `at`. Returns the new objects' ids. */
export function paste(at: Point): Promise<number[]> {
  return invoke<number[]>("paste", { at });
}

export function allIds(): Promise<number[]> {
  return invoke<number[]>("all_ids");
}

export function idsOnLayer(name: string): Promise<number[]> {
  return invoke<number[]>("ids_on_layer", { name });
}

/** Centre of the objects' bounding box (what rotate, scale and mirror turn around). */
export function selectionCentre(ids: number[]): Promise<Point | null> {
  return invoke<Point | null>("selection_centre", { ids });
}
