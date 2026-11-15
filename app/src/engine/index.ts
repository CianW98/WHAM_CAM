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
  objects: DrawingObject[];
}

export interface Part {
  name: string;
}

export interface Document {
  name: string;
  layers: Layer[];
  parts: Part[];
}

export interface DisplayShape {
  id: number;
  /** [x, y] pairs in mm, ready to draw. */
  points: [number, number][];
  closed: boolean;
  /** Separate point markers rather than a connected path. */
  markers: boolean;
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
