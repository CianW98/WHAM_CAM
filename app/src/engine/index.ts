// The connection layer between the interface and the CAM engine.
//
// This is the ONLY file in the interface that talks to the engine. Today it goes through
// Tauri; a future browser build swaps this file for a WebAssembly version and nothing
// else in the interface changes. The types mirror the engine's Rust types.

import { invoke } from "@tauri-apps/api/core";

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
  | { type: "Polyline"; vertices: Vertex[]; closed: boolean }
  | { type: "Circle"; center: Point; radius: number };

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
}

export interface DisplayData {
  shapes: DisplayShape[];
}

export function getDocument(): Promise<Document> {
  return invoke<Document>("get_document");
}

export function getDisplayData(): Promise<DisplayData> {
  return invoke<DisplayData>("get_display_data");
}
