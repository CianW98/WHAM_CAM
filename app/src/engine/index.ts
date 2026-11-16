// The connection layer between the interface and the CAM engine.
//
// This is the ONLY file in the interface that talks to the engine or to Tauri. Today it goes
// through Tauri; a future browser build swaps this file for a WebAssembly version and nothing
// else in the interface changes. The types mirror the engine's Rust types.

import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

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
  | { type: "Points"; points: Point[] }
  | {
      type: "Spline";
      degree: number;
      control_points: Point[];
      knots: number[];
      weights: number[];
      closed: boolean;
    }
  | {
      type: "Ellipse";
      center: Point;
      major_axis: Point;
      ratio: number;
      start_param: number;
      end_param: number;
    };

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
}

export interface Document {
  name: string;
  layers: Layer[];
  parts: Part[];
  /** Drawing grid spacing in mm. */
  grid_spacing: number;
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
}

export interface DxfUnits {
  code: number;
  name: string;
  mm_per_unit: number | null;
}

export interface DxfInfo {
  units: DxfUnits;
  shape_count: number;
}

export interface ImportReport {
  objects_added: number;
  layers: string[];
  skipped: Record<string, number>;
  scale: number;
}

export type BoxMode = "Window" | "Crossing";

export type DrawTool = "Line" | "Arc" | "Circle" | "Rectangle" | "Polyline" | "Points";

/** How many clicks finish each tool's shape; null = finished with Enter (polyline, points). */
export const CLICKS_NEEDED: Record<DrawTool, number | null> = {
  Line: 2,
  Circle: 2,
  Rectangle: 2,
  Arc: 3,
  Polyline: null,
  Points: null,
};

/** A shape being drawn: the points clicked so far. */
export interface Draft {
  tool: DrawTool;
  points: Point[];
  /** Polyline: arcs[i] = segment from points[i] to points[i+1] is a tangent arc. */
  arcs: boolean[];
  /** Polyline: the next segment (to the cursor) is a tangent arc. */
  arc_next: boolean;
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
export async function chooseDxfFile(): Promise<string | null> {
  const path = await open({
    title: "Import DXF",
    multiple: false,
    directory: false,
    filters: [{ name: "DXF drawings", extensions: ["dxf", "DXF"] }],
  });
  return typeof path === "string" ? path : null;
}

export function dxfInfo(path: string): Promise<DxfInfo> {
  return invoke<DxfInfo>("dxf_info", { path });
}

/** `scale` multiplies every coordinate: 1 = numbers are mm, 25.4 = numbers are inches. */
export function importDxf(path: string, scale: number): Promise<ImportReport> {
  return invoke<ImportReport>("import_dxf", { path, scale });
}

export function deleteObjects(ids: number[]): Promise<number> {
  return invoke<number>("delete_objects", { ids });
}

/** Deletes a layer and everything on it. */
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
