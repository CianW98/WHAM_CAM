// The drawing area. It only DRAWS what the engine sends (display point lists) and reports
// where the user clicked or dragged, in mm. Deciding which shape was clicked, where a point
// snaps to and what a shape being drawn looks like is all done by the engine.

import { type ReactNode, useEffect, useRef, useState } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import * as engine from "../engine";
import { toolAt, type ToolAt } from "../playback";
import type {
  BoxMode,
  CutWidthStep,
  DisplayData,
  DisplayShape,
  GridArea,
  Label,
  Draft,
  MoveKind,
  Playback,
  Point,
  Snap,
  SnapKind,
  SnapOptions,
  ToolpathDisplay,
} from "../engine";

/**
 * Something being placed by clicking in the drawing area: a shape being drawn, selected shapes
 * being moved (base point, then destination) or rotated (reference direction, then final
 * direction, around `centre`), or the clipboard being pasted.
 */
export type Placing =
  | { kind: "draw"; draft: Draft }
  | { kind: "move"; ids: number[]; base: Point | null }
  | { kind: "rotate"; ids: number[]; centre: Point; reference: Point | null }
  | { kind: "paste" };

/** A dashed guide line between two points (drawn as-is; no calculation). */
const guide = (a: Point, b: Point): DisplayShape => ({
  id: 0,
  points: [
    [a.x, a.y],
    [b.x, b.y],
  ],
  closed: false,
  markers: false,
  color: "",
  direction: null,
  extra: [],
  label: null,
});

export type ViewMode = "2D" | "3D";

const COLORS = {
  background: 0x2b2b33,
  gridMinor: 0x363640,
  gridMajor: 0x4a4a58,
  axisX: 0xd03030,
  axisY: 0x30b030,
  selected: 0xffd24a,
  draft: 0xe8e8f0,
};
/** Toolpath colours (spec 5.6): cuts, plunges and (dashed) rapids. */
const TOOLPATH_COLORS: Record<MoveKind, number> = {
  Cut: 0x4da6ff,
  Plunge: 0xff66cc,
  Rapid: 0xb0b0b8,
};
/** Toolpaths still to come while playing back are drawn this faint. */
const TOOLPATH_FADED_OPACITY = 0.22;
/** A dimension following the mouse is drawn at this opacity until placed. */
const DIMENSION_PREVIEW_OPACITY = 0.45;
const TOOL_COLOR = 0xf0f0f4;
/** The shaded cutter width under the toolpath lines (opaque, so overlaps don't darken). */
const CUT_WIDTH_COLOR = 0x35506c;
/** The cutter width still to come while playing back (fainter, like the toolpath lines). */
const CUT_WIDTH_FADED_COLOR = 0x2e353f;
const RAPID_DASH_MM = 2;
const RAPID_GAP_MM = 1.5;
const GRID_MAX_LINES = 500; // most grid lines either side of the origin (fine grids cover less)
const ZOOM_STEP = 1.15;
const PICK_RADIUS_PX = 6; // how close (in screen pixels) a click must be to a shape
const DRAG_THRESHOLD_PX = 4; // movement before a click becomes a box drag
const MARKER_SIZE_PX = 7;
const SNAP_RADIUS_PX = 10; // how close (in screen pixels) the cursor must be to snap to a shape
const DASH_PX = 6; // dash and gap length of the shape being drawn
const GAP_PX = 4;

/** 2D view: centre of the screen in mm, and mm per screen pixel. */
interface View2D {
  cx: number;
  cy: number;
  scale: number;
}

interface Scene3 {
  renderer: THREE.WebGLRenderer;
  scene: THREE.Scene;
  ortho: THREE.OrthographicCamera;
  persp: THREE.PerspectiveCamera;
  orbit: OrbitControls;
  shapes: THREE.Group;
  /** Generated toolpaths. */
  toolpaths: THREE.Group;
  /** The shaded width the cutter sweeps, under the toolpath lines. */
  cutWidths: THREE.Group;
  /** The cut width meshes, for playback (null when the cut width is off). */
  cut: {
    /** Every step's band (faded once playback has started). */
    all: THREE.Mesh;
    /** The same band revealed up to the tool. */
    done: THREE.Mesh;
    /** The part of the current segment done so far. */
    partial: THREE.Mesh;
    steps: CutWidthStep[];
    /** Index of each step's first triangle in the combined band. */
    offsets: number[];
    data: Float32Array;
  } | null;
  /** Playback: the moves done so far in full colour, drawn over the faded toolpaths. */
  done: THREE.Group;
  /** Playback: segments of each move kind done before each step (for the draw ranges). */
  doneCounts: Record<MoveKind, number[]> | null;
  /** Playback: the part of the current segment done so far. */
  current: THREE.Line;
  /** Playback: the cutter (outline in 2D and 3D, solid body in 3D only). */
  tool: {
    group: THREE.Group;
    outline: THREE.LineLoop;
    /** Holds the 3D cutters; only the current operation's is visible. */
    body: THREE.Group;
    /** The 3D cutter of each operation, by operation id. */
    bodies: Map<number, THREE.Group>;
  };
  grid: THREE.Group;
  /** The dashed preview of whatever is being placed. */
  preview: THREE.Group;
  /** Re-checks the snap and preview at the last cursor position. */
  hover: () => void;
  /** Call after the zoom changes (dimension sizes follow it). */
  scaleChanged: () => void;
  /** Drawn objects by engine id, so selection can recolour them without rebuilding. */
  byId: Map<number, THREE.Object3D[]>;
  materials: {
    /** One line / marker material per layer colour, created on first use. */
    line: (color: string) => THREE.LineBasicMaterial;
    marker: (color: string) => THREE.PointsMaterial;
    lineSelected: THREE.LineBasicMaterial;
    markerSelected: THREE.PointsMaterial;
  };
  view: View2D;
  mode: "2D" | "3D";
  width: number;
  height: number;
  render: () => void;
}

interface BoxDrag {
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}

interface SnapMark {
  point: Point;
  kind: SnapKind;
  /** Where to draw the marker, in screen pixels. */
  sx: number;
  sy: number;
}

interface Props {
  display: DisplayData | null;
  /** Toolpaths to draw (cut, plunge and rapid moves, in 3D). */
  toolpaths: ToolpathDisplay[];
  /** Shaded cutter width of each playback step, to draw under the toolpaths (empty = off). */
  cutWidths: CutWidthStep[];
  /** Tool playback data, or null when there's nothing to play. */
  playback: Playback | null;
  /** Playback time in seconds (0 = not started: toolpaths look normal). */
  playTime: number;
  /** Shown along the bottom of the drawing area (the playback bar). */
  children?: ReactNode;
  selectedIds: Set<number>;
  /** Changes whenever the view should zoom to fit the drawing. */
  fitRequest: number;
  gridSpacing: number;
  /** Spacing of the heavier grid lines (display only). */
  majorGridSpacing: number;
  /** The area the grid is drawn over (display only). */
  gridArea: GridArea;
  mode: ViewMode;
  onMode: (m: ViewMode) => void;
  /** What is being placed by clicking, or null when selecting. */
  placing: Placing | null;
  /** What to tell the user while placing, e.g. "Click the start point". */
  hint: string | null;
  snapOptions: SnapOptions;
  onSnapOptions: (o: SnapOptions) => void;
  onPick: (x: number, y: number, tolerance: number, add: boolean) => void;
  onBox: (x0: number, y0: number, x1: number, y1: number, mode: BoxMode, add: boolean) => void;
  /** A point was clicked (already snapped) while placing. */
  /**
   * A click while placing: `p` is the snapped point. With the Dimension tool, `click` is the
   * engine's answer for that click (a point or a picked object, and whether it finishes).
   */
  onPlacePoint: (p: Point, click: engine.DimensionClick | null) => void;
  /** Colour of the layer new shapes go on (for previews). */
  drawColor: string;
  /** The zoom has settled at `mmPerPx` millimetres per screen pixel (the engine already knows). */
  onScale: (mmPerPx: number) => void;
  /** A dimension was dragged to `p`: move where it sits. */
  onRepositionDimension: (id: number, p: Point) => void;
  /** Double-click while drawing: finish a polyline or point list. */
  onFinishDraft: () => void;
  /** Right-click without dragging, at page position (x, y). */
  onContextMenu: (x: number, y: number) => void;
}

function lineSegments(positions: number[], color: number, order: number): THREE.LineSegments {
  const geom = new THREE.BufferGeometry();
  geom.setAttribute("position", new THREE.Float32BufferAttribute(positions, 3));
  const mat = new THREE.LineBasicMaterial({ color, depthTest: false });
  const lines = new THREE.LineSegments(geom, mat);
  lines.renderOrder = order;
  return lines;
}

/** Grid lines every `spacing` mm. Fine grids cover a smaller area so the line count stays sane. */
/** The grid over the project's grid area (display only): minor lines every `spacing`, heavier
 * major lines every `major`, and the X/Y axes through 0,0. Very fine grids are drawn at most
 * GRID_MAX_LINES lines either side of the origin. */
function buildGrid(spacing: number, major: number, area: GridArea): THREE.Group {
  const reach = GRID_MAX_LINES * spacing;
  const x0 = Math.max(area.x_min, -reach);
  const x1 = Math.min(area.x_max, reach);
  const y0 = Math.max(area.y_min, -reach);
  const y1 = Math.min(area.y_max, reach);
  const minorLines: number[] = [];
  const majorLines: number[] = [];
  // Is v a whole multiple of `step` (to a small fraction of the minor spacing)?
  const onStep = (v: number, step: number) => Math.abs(v - Math.round(v / step) * step) < spacing * 1e-6;
  // Lines at whole multiples of `step` inside the area; 0 is left for the axes.
  const lines = (step: number, from: number, to: number, out: number[], line: (v: number, out: number[]) => void) => {
    for (let i = Math.ceil(from / step); i <= Math.floor(to / step); i++) {
      const v = i * step;
      if (i === 0 || (out === minorLines && onStep(v, major))) continue; // a major line goes there
      line(v, out);
    }
  };
  if (x0 < x1 && y0 < y1) {
    for (const [step, out] of [
      [spacing, minorLines],
      [Math.max(major, spacing), majorLines],
    ] as const) {
      lines(step, x0, x1, out, (v, o) => o.push(v, y0, 0, v, y1, 0));
      lines(step, y0, y1, out, (v, o) => o.push(x0, v, 0, x1, v, 0));
    }
  }
  const group = new THREE.Group();
  group.add(lineSegments(minorLines, COLORS.gridMinor, 0));
  group.add(lineSegments(majorLines, COLORS.gridMajor, 1));
  if (y0 <= 0 && 0 <= y1 && x0 < x1) group.add(lineSegments([x0, 0, 0, x1, 0, 0], COLORS.axisX, 2));
  if (x0 <= 0 && 0 <= x1 && y0 < y1) group.add(lineSegments([0, y0, 0, 0, y1, 0], COLORS.axisY, 2));
  return group;
}

/** Removes and frees everything in a group. */
function clearGroup(group: THREE.Group) {
  for (const child of [...group.children]) {
    const o = child as THREE.Line;
    o.geometry?.dispose();
    for (const m of [o.material].flat()) {
      (m as THREE.MeshBasicMaterial | undefined)?.map?.dispose();
      m?.dispose();
    }
    group.remove(child);
  }
}

/** Removes one drawn object of a shape from the scene, freeing what it used. */
function disposeShapeObject(s: Scene3, child: THREE.Object3D) {
  (child as THREE.Line).geometry.dispose();
  if (child.userData.label) {
    const m = (child as THREE.Mesh).material as THREE.MeshBasicMaterial;
    m.map?.dispose();
    m.dispose();
  }
  s.shapes.remove(child);
}

/** Draws one shape (or dimension) from the engine's display data. */
function addShape(s: Scene3, shape: DisplayShape) {
  const objs: THREE.Object3D[] = [];
  if (shape.markers) {
    const positions: number[] = [];
    for (const [x, y] of shape.points) positions.push(x, y, 0);
    const geom = new THREE.BufferGeometry();
    geom.setAttribute("position", new THREE.Float32BufferAttribute(positions, 3));
    objs.push(new THREE.Points(geom, s.materials.marker(shape.color)));
  } else {
    for (const path of paths(shape)) {
      const positions: number[] = [];
      for (const [x, y] of path.points) positions.push(x, y, 0);
      const geom = new THREE.BufferGeometry();
      geom.setAttribute("position", new THREE.Float32BufferAttribute(positions, 3));
      const mat = s.materials.line(shape.color);
      objs.push(path.closed ? new THREE.LineLoop(geom, mat) : new THREE.Line(geom, mat));
    }
  }
  if (shape.label) {
    const label = makeLabel(shape.label, shape.color);
    label.userData.label = true;
    objs.push(label);
  }
  for (const obj of objs) {
    obj.renderOrder = 10;
    obj.userData.markers = shape.markers;
    obj.userData.color = shape.color;
    s.shapes.add(obj);
  }
  const list = s.byId.get(shape.id) ?? [];
  list.push(...objs);
  s.byId.set(shape.id, list);
}

/** Colours a shape's drawn objects as selected or in their own colour. */
function colourShape(s: Scene3, objs: THREE.Object3D[], selected: boolean) {
  for (const obj of objs) {
    const o = obj as THREE.Line | THREE.Points;
    const color: string = obj.userData.color;
    o.renderOrder = selected ? 11 : 10;
    if (obj.userData.label) {
      ((obj as THREE.Mesh).material as THREE.MeshBasicMaterial).color.set(selected ? COLORS.selected : color);
      continue;
    }
    o.material = obj.userData.markers
      ? selected
        ? s.materials.markerSelected
        : s.materials.marker(color)
      : selected
        ? s.materials.lineSelected
        : s.materials.line(color);
  }
}

/** Swaps in dimensions laid out for a new zoom (while zooming, without a full redraw). */
function replaceDimensions(s: Scene3, shapes: DisplayShape[], selected: Set<number>) {
  for (const shape of shapes) {
    for (const obj of s.byId.get(shape.id) ?? []) disposeShapeObject(s, obj);
    s.byId.delete(shape.id);
    addShape(s, shape);
    colourShape(s, s.byId.get(shape.id) ?? [], selected.has(shape.id));
  }
  s.render();
}

/** Canvas pixels per text height for label textures. */
const LABEL_PX = 64;

/**
 * A dimension's value as a flat rectangle at the engine's position, angle and height: white text
 * on a transparent texture, tinted by the material colour (so selecting it can recolour it).
 */
function makeLabel(label: Label, color: THREE.ColorRepresentation): THREE.Mesh {
  const canvas = document.createElement("canvas");
  const ctx = canvas.getContext("2d")!;
  const font = `${LABEL_PX}px sans-serif`;
  ctx.font = font;
  canvas.width = Math.ceil(ctx.measureText(label.text).width) + 8;
  canvas.height = Math.round(LABEL_PX * 1.25);
  ctx.font = font; // resizing the canvas resets it
  ctx.fillStyle = "#ffffff";
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  ctx.fillText(label.text, canvas.width / 2, canvas.height / 2);
  const texture = new THREE.CanvasTexture(canvas);
  texture.minFilter = THREE.LinearFilter;
  const height = (label.height * canvas.height) / LABEL_PX;
  const width = (height * canvas.width) / canvas.height;
  const mesh = new THREE.Mesh(
    new THREE.PlaneGeometry(width, height),
    new THREE.MeshBasicMaterial({ map: texture, color, transparent: true, depthTest: false }),
  );
  mesh.position.set(label.at[0], label.at[1], 0);
  mesh.rotation.z = (label.angle * Math.PI) / 180;
  return mesh;
}

/** A shape's paths: its main path (if any) and its extra paths, as point lists. */
function paths(shape: DisplayShape): { points: [number, number][]; closed: boolean }[] {
  const out = shape.points.length > 0 ? [{ points: shape.points, closed: shape.closed }] : [];
  for (const points of shape.extra) out.push({ points, closed: false });
  return out;
}

const KINDS: MoveKind[] = ["Cut", "Plunge", "Rapid"];

/**
 * The playback cutter, tip at the origin: an outline circle 1 mm across (scaled to the tool
 * diameter), and in 3D the operation's real cutter (built per playback by `buildToolBodies`).
 */
function buildTool(): Scene3["tool"] {
  const group = new THREE.Group();
  group.visible = false;
  const circle: number[] = [];
  for (let i = 0; i < 64; i++) {
    const a = (i / 64) * Math.PI * 2;
    circle.push(0.5 * Math.cos(a), 0.5 * Math.sin(a), 0);
  }
  const outline = new THREE.LineLoop(
    new THREE.BufferGeometry().setAttribute("position", new THREE.Float32BufferAttribute(circle, 3)),
    new THREE.LineBasicMaterial({ color: TOOL_COLOR, depthTest: false }),
  );
  outline.renderOrder = 8;
  group.add(outline);
  const body = new THREE.Group();
  group.add(body);
  return { group, outline, body, bodies: new Map() };
}

/** Turns a profile (x = radius, y = height) round the tool axis, standing along +Z. */
function lathe(points: [number, number][], opacity: number): THREE.Mesh {
  const geom = new THREE.LatheGeometry(
    points.map(([x, y]) => new THREE.Vector2(x, y)),
    48,
  );
  const mat = new THREE.MeshBasicMaterial({
    color: TOOL_COLOR,
    transparent: true,
    opacity,
    depthWrite: false,
    side: THREE.DoubleSide,
  });
  const mesh = new THREE.Mesh(geom, mat);
  // LatheGeometry turns round Y; turn it to stand along Z.
  mesh.rotation.x = Math.PI / 2;
  mesh.renderOrder = 7;
  return mesh;
}

/**
 * The 3D cutter of each operation in the playback, from the tool outline the engine sends:
 * the fluted part a little stronger, the shank fainter. Real size (not scaled).
 */
function buildToolBodies(s: Scene3, pb: Playback | null) {
  const { body, bodies } = s.tool;
  // Free the old cutters' meshes (they sit one level down, in a group per operation).
  for (const g of bodies.values()) clearGroup(g);
  clearGroup(body);
  bodies.clear();
  for (const t of pb?.tools ?? []) {
    const pts = t.profile.map((p) => [p.x, p.y] as [number, number]);
    if (pts.length < 3) continue;
    const fl = t.flute_length;
    const eps = 1e-6;
    const top = pts[pts.length - 1][1];
    // Flutes: from the tip up to the flute top, closed back to the axis there.
    const flutes = pts.filter(([, y]) => y <= fl + eps);
    flutes.push([0, Math.min(fl, top)]);
    const g = new THREE.Group();
    g.add(lathe(flutes, 0.55));
    // Shank: a plain cylinder from the flute top to the top of the tool.
    const shankRadius = Math.max(0, ...pts.filter(([x, y]) => y > fl + eps && x > 0).map(([x]) => x));
    if (shankRadius > 0 && top > fl) {
      g.add(
        lathe(
          [
            [0, fl],
            [shankRadius, fl],
            [shankRadius, top],
            [0, top],
          ],
          0.35,
        ),
      );
    }
    g.visible = false;
    body.add(g);
    bodies.set(t.operation_id, g);
  }
}

/**
 * The "done" overlay: every playback segment of each move kind in one line set, in order, so
 * the moves done so far can be revealed with a draw range.
 */
function buildDone(s: Scene3, pb: Playback | null) {
  clearGroup(s.done);
  s.doneCounts = null;
  if (!pb || pb.steps.length === 0) return;
  const positions: Record<MoveKind, number[]> = { Cut: [], Plunge: [], Rapid: [] };
  const counts: Record<MoveKind, number[]> = { Cut: [], Plunge: [], Rapid: [] };
  const n: Record<MoveKind, number> = { Cut: 0, Plunge: 0, Rapid: 0 };
  for (const step of pb.steps) {
    for (const k of KINDS) counts[k].push(n[k]);
    for (let i = 1; i < step.points.length; i++) {
      positions[step.kind].push(...step.points[i - 1], ...step.points[i]);
      n[step.kind]++;
    }
  }
  for (const k of KINDS) {
    const geom = new THREE.BufferGeometry();
    geom.setAttribute("position", new THREE.Float32BufferAttribute(positions[k], 3));
    const color = TOOLPATH_COLORS[k];
    const mat =
      k === "Rapid"
        ? new THREE.LineDashedMaterial({ color, dashSize: RAPID_DASH_MM, gapSize: RAPID_GAP_MM, depthTest: false })
        : new THREE.LineBasicMaterial({ color, depthTest: false });
    const lines = new THREE.LineSegments(geom, mat);
    if (k === "Rapid") lines.computeLineDistances();
    lines.renderOrder = 6;
    lines.userData.kind = k;
    s.done.add(lines);
  }
  s.doneCounts = counts;
}

/** Builds the cut width meshes: the whole band, the part done so far, and the current segment. */
function buildCutWidths(s: Scene3, steps: CutWidthStep[]) {
  clearGroup(s.cutWidths);
  s.cut = null;
  const offsets: number[] = [];
  let n = 0;
  for (const st of steps) {
    offsets.push(n);
    n += st.triangles.length / 9;
  }
  if (n === 0) return;
  const data = new Float32Array(n * 9);
  let k = 0;
  for (const st of steps) {
    data.set(st.triangles, k);
    k += st.triangles.length;
  }
  const attr = new THREE.BufferAttribute(data, 3);
  const mesh = (geom: THREE.BufferGeometry, color: number, order: number) => {
    const m = new THREE.Mesh(geom, new THREE.MeshBasicMaterial({ color, side: THREE.DoubleSide, depthTest: false }));
    m.renderOrder = order; // under the toolpath lines and the drawing
    s.cutWidths.add(m);
    return m;
  };
  const all = mesh(new THREE.BufferGeometry().setAttribute("position", attr), CUT_WIDTH_COLOR, 3);
  const done = mesh(new THREE.BufferGeometry().setAttribute("position", attr), CUT_WIDTH_COLOR, 4);
  const partial = mesh(
    new THREE.BufferGeometry().setAttribute("position", new THREE.BufferAttribute(new Float32Array(18), 3)),
    CUT_WIDTH_COLOR,
    4,
  );
  done.visible = false;
  partial.visible = false;
  s.cut = { all, done, partial, steps, offsets, data };
}

/** Shows the cut width done by the tool at `at` in full colour and the rest faded. */
function showCutWidth(s: Scene3, pb: Playback | null, at: ToolAt | null, started: boolean, finished: boolean) {
  const cut = s.cut;
  if (!cut) return;
  const aligned = pb !== null && cut.steps.length === pb.steps.length;
  const playing = started && !finished && aligned && at !== null;
  (cut.all.material as THREE.MeshBasicMaterial).color.setHex(playing ? CUT_WIDTH_FADED_COLOR : CUT_WIDTH_COLOR);
  cut.done.visible = playing;
  cut.partial.visible = false;
  if (!playing || !pb || !at) return;
  const w = cut.steps[at.step];
  const segStart = cut.offsets[at.step] + (w.segment_starts[at.seg] ?? w.triangles.length / 9);
  cut.done.geometry.setDrawRange(0, segStart * 3);
  // The strip of the current segment, cut off at the tool (its far corners moved back).
  const pts = pb.steps[at.step].points;
  const a = pts[at.seg];
  const b = pts[at.seg + 1];
  const hasStrip = w.triangles.length > 0 && b !== undefined && Math.hypot(b[0] - a[0], b[1] - a[1]) > 1e-9;
  if (!hasStrip) return;
  const v = cut.data.subarray(segStart * 9, segStart * 9 + 18);
  const out = new Float32Array(v);
  const lerp = (to: number, from: number) => {
    for (let c = 0; c < 3; c++) out[to * 3 + c] = v[from * 3 + c] + (v[to * 3 + c] - v[from * 3 + c]) * at.u;
  };
  // Strip triangles: (a+n, a−n, b+n) and (b+n, a−n, b−n).
  lerp(2, 0);
  lerp(3, 0);
  lerp(5, 4);
  const attr = cut.partial.geometry.getAttribute("position") as THREE.BufferAttribute;
  (attr.array as Float32Array).set(out);
  attr.needsUpdate = true;
  cut.partial.geometry.computeBoundingSphere();
  cut.partial.visible = true;
}

/** Places the tool and reveals the moves done by time `t`; fades the rest once started. */
function showPlayback(s: Scene3, pb: Playback | null, t: number) {
  const at = pb ? toolAt(pb, t) : null;
  const started = at !== null && t > 0;
  showCutWidth(s, pb, at, started, pb !== null && t >= pb.total_s);
  for (const child of s.toolpaths.children) {
    const m = (child as THREE.Line).material as THREE.Material;
    m.opacity = started ? TOOLPATH_FADED_OPACITY : 1;
  }
  s.done.visible = started;
  s.current.visible = false;
  s.tool.group.visible = at !== null;
  if (!pb || !at) return;

  const step = pb.steps[at.step];
  if (s.doneCounts) {
    for (const child of s.done.children) {
      const k = child.userData.kind as MoveKind;
      const segs = s.doneCounts[k][at.step] + (step.kind === k ? at.seg : 0);
      (child as THREE.LineSegments).geometry.setDrawRange(0, segs * 2);
    }
  }
  if (started) {
    const a = step.points[at.seg];
    const pos = s.current.geometry.getAttribute("position") as THREE.BufferAttribute;
    pos.setXYZ(0, a[0], a[1], a[2]);
    pos.setXYZ(1, at.point[0], at.point[1], at.point[2]);
    pos.needsUpdate = true;
    s.current.geometry.computeBoundingSphere();
    (s.current.material as THREE.LineBasicMaterial).color.setHex(TOOLPATH_COLORS[step.kind]);
    s.current.visible = true;
  }
  const d = Math.max(step.tool_diameter, 0.01);
  s.tool.group.position.set(at.point[0], at.point[1], at.point[2]);
  s.tool.outline.scale.set(d, d, d);
  for (const [id, g] of s.tool.bodies) g.visible = id === step.operation_id;
  s.tool.body.visible = s.mode === "3D";
}

/** Draws the preview of what is being placed: dashed lines, or markers for point lists. */
function showPreview(s: Scene3, shapes: DisplayShape[], drawColor: string) {
  clearGroup(s.preview);
  for (const shape of shapes) {
    if (shape.label) {
      // A dimension following the mouse: drawn as it will be, in its own colour but faded
      // (like the toolpaths during playback) until the click places it.
      const color = shape.color || drawColor;
      const label = makeLabel(shape.label, color);
      const fade = (m: THREE.Material) => {
        m.transparent = true;
        m.opacity = DIMENSION_PREVIEW_OPACITY;
        return m;
      };
      fade(label.material as THREE.Material);
      label.renderOrder = 12;
      s.preview.add(label);
      for (const path of paths(shape)) {
        const geom = new THREE.BufferGeometry();
        geom.setAttribute("position", new THREE.Float32BufferAttribute(path.points.flatMap(([x, y]) => [x, y, 0]), 3));
        const line = new THREE.Line(geom, fade(new THREE.LineBasicMaterial({ color, depthTest: false })));
        line.renderOrder = 12;
        s.preview.add(line);
      }
      continue;
    }
    if (shape.markers) {
      const positions: number[] = [];
      for (const [x, y] of shape.points) positions.push(x, y, 0);
      const geom = new THREE.BufferGeometry();
      geom.setAttribute("position", new THREE.Float32BufferAttribute(positions, 3));
      const obj = new THREE.Points(
        geom,
        new THREE.PointsMaterial({ color: COLORS.draft, size: MARKER_SIZE_PX, sizeAttenuation: false, depthTest: false }),
      );
      obj.renderOrder = 12;
      s.preview.add(obj);
      continue;
    }
    for (const path of paths(shape)) {
      const positions: number[] = [];
      for (const [x, y] of path.points) positions.push(x, y, 0);
      const geom = new THREE.BufferGeometry();
      geom.setAttribute("position", new THREE.Float32BufferAttribute(positions, 3));
      const mat = new THREE.LineDashedMaterial({
        color: COLORS.draft,
        dashSize: DASH_PX * s.view.scale,
        gapSize: GAP_PX * s.view.scale,
        depthTest: false,
      });
      const line = path.closed ? new THREE.LineLoop(geom, mat) : new THREE.Line(geom, mat);
      line.computeLineDistances();
      const obj: THREE.Object3D = line;
      obj.renderOrder = 12;
      s.preview.add(obj);
    }
  }
  s.render();
}

function applyOrtho(s: Scene3) {
  const { cx, cy, scale } = s.view;
  const hw = (s.width / 2) * scale;
  const hh = (s.height / 2) * scale;
  s.ortho.left = -hw;
  s.ortho.right = hw;
  s.ortho.top = hh;
  s.ortho.bottom = -hh;
  s.ortho.position.set(cx, cy, 1000);
  s.ortho.updateProjectionMatrix();
}

function screenToWorld(s: Scene3, px: number, py: number) {
  return {
    x: s.view.cx + (px - s.width / 2) * s.view.scale,
    y: s.view.cy - (py - s.height / 2) * s.view.scale,
  };
}

function worldToScreen(s: Scene3, p: Point) {
  return {
    x: (p.x - s.view.cx) / s.view.scale + s.width / 2,
    y: s.height / 2 - (p.y - s.view.cy) / s.view.scale,
  };
}

function fitTo(s: Scene3, bounds: [number, number, number, number]) {
  const [minX, minY, maxX, maxY] = bounds;
  const w = Math.max(maxX - minX, 1);
  const h = Math.max(maxY - minY, 1);
  s.view = {
    cx: (minX + maxX) / 2,
    cy: (minY + maxY) / 2,
    scale: Math.max(w / s.width, h / s.height) * 1.15,
  };
  applyOrtho(s);
  s.scaleChanged();
}

export default function Viewport({
  display,
  toolpaths,
  cutWidths,
  playback,
  playTime,
  children,
  selectedIds,
  fitRequest,
  gridSpacing,
  majorGridSpacing,
  gridArea,
  mode,
  onMode,
  placing,
  hint,
  snapOptions,
  onSnapOptions,
  onPick,
  onBox,
  onPlacePoint,
  drawColor,
  onScale,
  onRepositionDimension,
  onFinishDraft,
  onContextMenu,
}: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const sceneRef = useRef<Scene3 | null>(null);
  const [cursor, setCursor] = useState<{ x: number; y: number } | null>(null);
  const [box, setBox] = useState<BoxDrag | null>(null);
  const [snapMark, setSnapMark] = useState<SnapMark | null>(null);

  // Latest props, so the once-only event listeners below always use the current ones.
  const live = useRef({
    onPick,
    onBox,
    onPlacePoint,
    onFinishDraft,
    onContextMenu,
    placing,
    snapOptions,
    drawColor,
    onScale,
    onRepositionDimension,
    selectedIds,
  });
  live.current = {
    onPick,
    onBox,
    onPlacePoint,
    onFinishDraft,
    onContextMenu,
    placing,
    snapOptions,
    drawColor,
    onScale,
    onRepositionDimension,
    selectedIds,
  };

  // Create the WebGL scene once.
  useEffect(() => {
    const container = containerRef.current!;
    const renderer = new THREE.WebGLRenderer({ antialias: true });
    renderer.setPixelRatio(window.devicePixelRatio);
    renderer.setClearColor(COLORS.background);
    container.appendChild(renderer.domElement);

    const scene = new THREE.Scene();
    const grid = buildGrid(10, 50, { x_min: -1000, x_max: 1000, y_min: -1000, y_max: 1000 }) // replaced once the project arrives;
    scene.add(grid);
    const cutWidthGroup = new THREE.Group();
    scene.add(cutWidthGroup);
    const toolpathGroup = new THREE.Group();
    scene.add(toolpathGroup);
    const doneGroup = new THREE.Group();
    scene.add(doneGroup);
    const current = new THREE.Line(
      new THREE.BufferGeometry().setAttribute("position", new THREE.Float32BufferAttribute(new Float32Array(6), 3)),
      new THREE.LineBasicMaterial({ color: TOOLPATH_COLORS.Cut, depthTest: false }),
    );
    current.renderOrder = 6;
    current.visible = false;
    scene.add(current);
    const tool = buildTool();
    scene.add(tool.group);
    const shapes = new THREE.Group();
    scene.add(shapes);
    const previewGroup = new THREE.Group();
    scene.add(previewGroup);

    const ortho = new THREE.OrthographicCamera(-1, 1, 1, -1, 0.1, 5000);
    const persp = new THREE.PerspectiveCamera(45, 1, 0.1, 20000);
    persp.up.set(0, 0, 1);
    const orbit = new OrbitControls(persp, renderer.domElement);
    orbit.enabled = false;

    const marker = (color: number | string) =>
      new THREE.PointsMaterial({ color, size: MARKER_SIZE_PX, sizeAttenuation: false, depthTest: false });
    const lineCache = new Map<string, THREE.LineBasicMaterial>();
    const markerCache = new Map<string, THREE.PointsMaterial>();
    const cached = <M,>(cache: Map<string, M>, make: (c: string) => M) => (color: string) => {
      let m = cache.get(color);
      if (!m) {
        m = make(color);
        cache.set(color, m);
      }
      return m;
    };
    const s: Scene3 = {
      renderer,
      scene,
      ortho,
      persp,
      orbit,
      shapes,
      toolpaths: toolpathGroup,
      cutWidths: cutWidthGroup,
      cut: null,
      done: doneGroup,
      doneCounts: null,
      current,
      tool,
      grid,
      preview: previewGroup,
      hover: () => {},
      scaleChanged: () => {},
      byId: new Map(),
      materials: {
        line: cached(lineCache, (color) => new THREE.LineBasicMaterial({ color, depthTest: false })),
        marker: cached(markerCache, marker),
        lineSelected: new THREE.LineBasicMaterial({ color: COLORS.selected, depthTest: false }),
        markerSelected: marker(COLORS.selected),
      },
      view: { cx: 0, cy: 0, scale: 0.5 },
      mode: "2D",
      width: 1,
      height: 1,
      render: () => renderer.render(scene, s.mode === "2D" ? ortho : persp),
    };
    sceneRef.current = s;
    orbit.addEventListener("change", s.render);

    const resize = () => {
      s.width = Math.max(1, container.clientWidth);
      s.height = Math.max(1, container.clientHeight);
      renderer.setSize(s.width, s.height);
      persp.aspect = s.width / s.height;
      persp.updateProjectionMatrix();
      applyOrtho(s);
      s.render();
    };
    const observer = new ResizeObserver(resize);
    observer.observe(container);
    resize();

    // 2D mouse: left click = pick (or place a point while placing), left drag = box select,
    // middle/right drag = pan, wheel = zoom. In 3D the orbit controls take over.
    // A right-click without dragging opens the menu (2D and 3D).
    const canvas = renderer.domElement;
    let panFrom: { x: number; y: number } | null = null;
    let rightDown: { x: number; y: number; moved: boolean } | null = null;
    /** A left button press: where, whether it has become a drag, and the dimension under it
     * (undefined while the engine is being asked, null for none). Dragging a dimension moves
     * it; dragging anywhere else draws a selection box. */
    let leftDown: { x: number; y: number; dragging: boolean; dimension?: number | null } | null = null;
    /** The zoom changed: redraw the dimensions for it straight away (one engine request at a
     * time, going again if the zoom moved meanwhile), and tell the app once it settles. */
    let scaleTimer: ReturnType<typeof setTimeout> | undefined;
    let scaleBusy = false;
    let scaleAgain = false;
    const scaleChanged = async () => {
      clearTimeout(scaleTimer);
      scaleTimer = setTimeout(() => live.current.onScale(s.view.scale), 250);
      if (scaleBusy) {
        scaleAgain = true;
        return;
      }
      scaleBusy = true;
      try {
        do {
          scaleAgain = false;
          const shapes = await engine.setViewScale(s.view.scale);
          if (!scaleAgain) replaceDimensions(s, shapes, live.current.selectedIds);
        } while (scaleAgain);
      } catch (err) {
        console.error(err);
      } finally {
        scaleBusy = false;
      }
    };
    s.scaleChanged = () => void scaleChanged();
    void scaleChanged();
    /** While dragging a dimension: show it at the cursor (one engine request at a time). */
    let dragBusy = false;
    let dragAgain: Point | null = null;
    const dragTo = async (id: number, p: Point) => {
      if (dragBusy) {
        dragAgain = p;
        return;
      }
      dragBusy = true;
      try {
        let at: Point | null = p;
        while (at) {
          dragAgain = null;
          const shape = await engine.dimensionDragPreview(id, at.x, at.y);
          if (leftDown?.dimension !== id) break; // let go meanwhile
          for (const obj of s.byId.get(id) ?? []) obj.visible = false;
          showPreview(s, shape ? [shape] : [], live.current.drawColor);
          at = dragAgain;
        }
      } finally {
        dragBusy = false;
      }
    };
    let mouse: { x: number; y: number } | null = null;
    const local = (e: MouseEvent) => {
      const rect = canvas.getBoundingClientRect();
      return { x: e.clientX - rect.left, y: e.clientY - rect.top };
    };
    /** Where the cursor snaps to. With the Dimension tool the engine also says whether the
     * click would pick a circle, arc or line (then nothing snaps). */
    const snapAt = async (m: { x: number; y: number }): Promise<Snap & { click: engine.DimensionClick | null }> => {
      const w = screenToWorld(s, m.x, m.y);
      const { placing, snapOptions } = live.current;
      const tolerance = SNAP_RADIUS_PX * s.view.scale;
      if (placing?.kind === "draw" && placing.draft.tool === "Dimension") {
        const click = await engine.dimensionClick(placing.draft, w.x, w.y, tolerance, snapOptions);
        return { point: click.point, kind: click.kind, click };
      }
      const extra = placing?.kind === "draw" ? placing.draft.points : [];
      return { ...(await engine.snap(w.x, w.y, tolerance, snapOptions, extra)), click: null };
    };

    /** What the placing looks like with the cursor at `p`, from the engine. */
    const previewAt = async (placing: Placing, p: Point): Promise<DisplayShape[]> => {
      switch (placing.kind) {
        case "draw": {
          const shape = await engine.previewShape(placing.draft, p);
          return shape ? [shape] : [];
        }
        case "move":
          return placing.base
            ? engine.transformPreview(placing.ids, { type: "Move", dx: p.x - placing.base.x, dy: p.y - placing.base.y })
            : [];
        case "rotate": {
          const { ids, centre, reference } = placing;
          if (!reference) return [guide(centre, p)];
          const turned = await engine.transformPreview(ids, { type: "RotateFromTo", from: reference, to: p });
          return [guide(centre, reference), guide(centre, p), ...turned];
        }
        case "paste":
          return engine.pastePreview(p);
      }
    };

    // While placing: ask the engine where the cursor snaps to and what the preview looks like
    // there. Only one request at a time; if the mouse moved meanwhile, go again with the
    // latest position.
    let hoverBusy = false;
    let hoverAgain = false;
    s.hover = async () => {
      if (hoverBusy) {
        hoverAgain = true;
        return;
      }
      hoverBusy = true;
      try {
        do {
          hoverAgain = false;
          const placing = live.current.placing;
          if (!placing || !mouse || s.mode !== "2D") {
            showPreview(s, [], live.current.drawColor);
            setSnapMark(null);
            break;
          }
          const snap = await snapAt(mouse);
          const shapes = await previewAt(placing, snap.point);
          if (hoverAgain) continue;
          showPreview(s, live.current.placing ? shapes : [], live.current.drawColor);
          const sp = worldToScreen(s, snap.point);
          setSnapMark({ ...snap, sx: sp.x, sy: sp.y });
        } while (hoverAgain);
      } catch (err) {
        console.error(err);
      } finally {
        hoverBusy = false;
      }
    };

    // Clicks and double-clicks while placing are handled strictly in order, so a double-click
    // never finishes a shape before its last point has been added.
    let drawQueue: Promise<void> = Promise.resolve();
    const enqueue = (step: () => Promise<void> | void) => {
      drawQueue = drawQueue.then(step).catch((err) => console.error(err));
    };

    const onWheel = (e: WheelEvent) => {
      if (s.mode !== "2D") return;
      e.preventDefault();
      const m = local(e);
      const p = screenToWorld(s, m.x, m.y);
      const factor = e.deltaY > 0 ? ZOOM_STEP : 1 / ZOOM_STEP;
      s.view = {
        cx: p.x - (p.x - s.view.cx) * factor,
        cy: p.y - (p.y - s.view.cy) * factor,
        scale: s.view.scale * factor,
      };
      applyOrtho(s);
      s.render();
      s.hover();
      void scaleChanged();
    };
    const onPointerDown = (e: PointerEvent) => {
      if (e.button === 2) rightDown = { x: e.clientX, y: e.clientY, moved: false };
      if (s.mode !== "2D") return;
      if (e.button === 1 || e.button === 2) {
        e.preventDefault();
        panFrom = { x: e.clientX, y: e.clientY };
      } else if (e.button === 0) {
        const m = local(e);
        const down: NonNullable<typeof leftDown> = { ...m, dragging: false };
        leftDown = down;
        if (!live.current.placing) {
          const p = screenToWorld(s, m.x, m.y);
          engine.dimensionAt(p.x, p.y, PICK_RADIUS_PX * s.view.scale).then(
            (id) => (down.dimension = id),
            () => (down.dimension = null),
          );
        } else {
          down.dimension = null;
        }
      } else return;
      canvas.setPointerCapture(e.pointerId);
    };
    const onPointerMove = (e: PointerEvent) => {
      if (rightDown && Math.hypot(e.clientX - rightDown.x, e.clientY - rightDown.y) > DRAG_THRESHOLD_PX) {
        rightDown.moved = true;
      }
      if (s.mode !== "2D") return;
      const m = local(e);
      if (panFrom) {
        s.view.cx -= (e.clientX - panFrom.x) * s.view.scale;
        s.view.cy += (e.clientY - panFrom.y) * s.view.scale;
        panFrom = { x: e.clientX, y: e.clientY };
        applyOrtho(s);
        s.render();
      }
      if (leftDown) {
        if (!leftDown.dragging && Math.hypot(m.x - leftDown.x, m.y - leftDown.y) > DRAG_THRESHOLD_PX) {
          leftDown.dragging = true;
        }
        if (leftDown.dragging && !live.current.placing) {
          const id = leftDown.dimension;
          if (typeof id === "number") void dragTo(id, screenToWorld(s, m.x, m.y));
          else if (id === null) setBox({ x0: leftDown.x, y0: leftDown.y, x1: m.x, y1: m.y });
        }
      }
      mouse = m;
      setCursor(screenToWorld(s, m.x, m.y));
      if (live.current.placing || panFrom) s.hover();
    };
    const onPointerUp = (e: PointerEvent) => {
      panFrom = null;
      if (e.button === 2 && rightDown) {
        if (!rightDown.moved) live.current.onContextMenu(e.clientX, e.clientY);
        rightDown = null;
        return;
      }
      if (s.mode !== "2D" || !leftDown || e.button !== 0) return;
      const m = local(e);
      const add = e.shiftKey || e.ctrlKey;
      if (live.current.placing) {
        // Placing: a click (or a short drag) places the snapped point.
        enqueue(async () => {
          const snap = await snapAt(m);
          live.current.onPlacePoint(snap.point, snap.click);
        });
      } else if (leftDown.dragging && typeof leftDown.dimension === "number") {
        // Let go of a dragged dimension: it sits here now.
        const id = leftDown.dimension;
        const p = screenToWorld(s, m.x, m.y);
        showPreview(s, [], live.current.drawColor);
        for (const obj of s.byId.get(id) ?? []) obj.visible = true;
        live.current.onRepositionDimension(id, p);
      } else if (leftDown.dragging) {
        const a = screenToWorld(s, leftDown.x, leftDown.y);
        const b = screenToWorld(s, m.x, m.y);
        // CAD convention: drag left-to-right = fully inside; right-to-left = touching.
        const mode: BoxMode = m.x >= leftDown.x ? "Window" : "Crossing";
        live.current.onBox(a.x, a.y, b.x, b.y, mode, add);
      } else {
        const p = screenToWorld(s, m.x, m.y);
        live.current.onPick(p.x, p.y, PICK_RADIUS_PX * s.view.scale, add);
      }
      leftDown = null;
      setBox(null);
    };
    const onDoubleClick = () => {
      if (s.mode === "2D" && live.current.placing?.kind === "draw") enqueue(() => live.current.onFinishDraft());
    };
    const onContextMenu = (e: Event) => e.preventDefault();
    const onLeave = () => {
      mouse = null;
      setCursor(null);
      s.hover();
    };

    canvas.addEventListener("wheel", onWheel, { passive: false });
    canvas.addEventListener("pointerdown", onPointerDown);
    canvas.addEventListener("pointermove", onPointerMove);
    canvas.addEventListener("pointerup", onPointerUp);
    canvas.addEventListener("dblclick", onDoubleClick);
    canvas.addEventListener("contextmenu", onContextMenu);
    canvas.addEventListener("pointerleave", onLeave);

    return () => {
      observer.disconnect();
      orbit.dispose();
      renderer.dispose();
      container.removeChild(canvas);
      sceneRef.current = null;
    };
  }, []);

  // Rebuild the grid when its spacings or area change.
  const { x_min, x_max, y_min, y_max } = gridArea;
  useEffect(() => {
    const s = sceneRef.current;
    if (!s || !(gridSpacing > 0 && majorGridSpacing > 0)) return;
    s.scene.remove(s.grid);
    clearGroup(s.grid);
    s.grid = buildGrid(gridSpacing, majorGridSpacing, { x_min, x_max, y_min, y_max });
    s.scene.add(s.grid);
    s.render();
  }, [gridSpacing, majorGridSpacing, x_min, x_max, y_min, y_max]);

  // Update the preview and snap marker when what is being placed or the snap settings change
  // (also clears them when placing stops).
  useEffect(() => {
    sceneRef.current?.hover();
  }, [placing, snapOptions, display]);

  // Rebuild the drawn shapes when the engine's display data changes.
  useEffect(() => {
    const s = sceneRef.current;
    if (!s) return;
    for (const child of [...s.shapes.children]) disposeShapeObject(s, child);
    s.byId.clear();
    for (const shape of display?.shapes ?? []) addShape(s, shape);
    s.render();
  }, [display]);

  // Rebuild the drawn toolpaths when they change.
  useEffect(() => {
    const s = sceneRef.current;
    if (!s) return;
    clearGroup(s.toolpaths);
    for (const tp of toolpaths) {
      for (const run of tp.runs) {
        const positions: number[] = [];
        for (const [x, y, z] of run.points) positions.push(x, y, z);
        const geom = new THREE.BufferGeometry();
        geom.setAttribute("position", new THREE.Float32BufferAttribute(positions, 3));
        const color = TOOLPATH_COLORS[run.kind];
        const mat =
          run.kind === "Rapid"
            ? new THREE.LineDashedMaterial({ color, dashSize: RAPID_DASH_MM, gapSize: RAPID_GAP_MM, depthTest: false })
            : new THREE.LineBasicMaterial({ color, depthTest: false });
        mat.transparent = true;
        const line = new THREE.Line(geom, mat);
        if (run.kind === "Rapid") line.computeLineDistances();
        line.renderOrder = 5; // under the drawing, so shapes stay visible and clickable
        s.toolpaths.add(line);
      }
    }
    showPlayback(s, playback, playTime);
    s.render();
  }, [toolpaths]);

  // Rebuild the shaded cut widths when they change.
  useEffect(() => {
    const s = sceneRef.current;
    if (!s) return;
    buildCutWidths(s, cutWidths);
    showPlayback(s, playback, playTime);
    s.render();
  }, [cutWidths]);

  // Rebuild the "done" overlay when the playback changes.
  useEffect(() => {
    const s = sceneRef.current;
    if (!s) return;
    buildDone(s, playback);
    buildToolBodies(s, playback);
    showPlayback(s, playback, playTime);
    s.render();
  }, [playback]);

  // Move the tool and reveal the done moves as the playback time changes.
  useEffect(() => {
    const s = sceneRef.current;
    if (!s) return;
    showPlayback(s, playback, playTime);
    s.render();
  }, [playTime]);

  // Recolour selected shapes.
  useEffect(() => {
    const s = sceneRef.current;
    if (!s) return;
    for (const [id, objs] of s.byId) colourShape(s, objs, selectedIds.has(id));
    s.render();
  }, [display, selectedIds]);

  // Zoom to fit only when asked (Zoom to fit, or after a DXF import). Drawing the first shape
  // into an empty project leaves the view where it is.
  const lastFit = useRef(fitRequest);
  useEffect(() => {
    const s = sceneRef.current;
    const bounds = display?.bounds;
    if (!s || !bounds) return;
    if (lastFit.current === fitRequest) return;
    lastFit.current = fitRequest;
    fitTo(s, bounds);
    s.render();
  }, [fitRequest, display]);

  // Switch between 2D (top view) and 3D (orbit).
  useEffect(() => {
    const s = sceneRef.current;
    if (!s) return;
    s.mode = mode;
    s.tool.body.visible = mode === "3D";
    s.orbit.enabled = mode === "3D";
    if (mode === "3D") {
      const { cx, cy, scale } = s.view;
      const dist = s.height * scale;
      s.orbit.target.set(cx, cy, 0);
      s.persp.position.set(cx, cy - dist * 0.8, dist * 0.8);
      s.orbit.update();
      setCursor(null);
    }
    s.hover();
    s.render();
  }, [mode]);

  return (
    <main className={`viewport${children ? " with-bar" : ""}`} ref={containerRef}>
      {box && (
        <div
          className={`select-box ${box.x1 >= box.x0 ? "window" : "crossing"}`}
          style={{
            left: Math.min(box.x0, box.x1),
            top: Math.min(box.y0, box.y1),
            width: Math.abs(box.x1 - box.x0),
            height: Math.abs(box.y1 - box.y0),
          }}
        />
      )}
      <div className="view-toggle">
        {(["2D", "3D"] as const).map((m) => (
          <button key={m} className={mode === m ? "active" : ""} onClick={() => onMode(m)}>
            {m}
          </button>
        ))}
      </div>
      {mode === "2D" && (
        <div className="snap-toggle" title="Snapping while drawing">
          <span>Snap</span>
          <button
            className={snapOptions.grid ? "active" : ""}
            onClick={() => onSnapOptions({ ...snapOptions, grid: !snapOptions.grid })}
          >
            Grid
          </button>
          <button
            className={snapOptions.objects ? "active" : ""}
            onClick={() => onSnapOptions({ ...snapOptions, objects: !snapOptions.objects })}
          >
            Objects
          </button>
        </div>
      )}
      {placing && snapMark && snapMark.kind !== "None" && (
        <svg className="snap-marker" style={{ left: snapMark.sx, top: snapMark.sy }} viewBox="-8 -8 16 16">
          <title>{snapMark.kind}</title>
          {snapMark.kind === "End" && <rect x={-5} y={-5} width={10} height={10} />}
          {snapMark.kind === "Mid" && <polygon points="0,-6 6,5 -6,5" />}
          {snapMark.kind === "Centre" && <circle r={5.5} />}
          {snapMark.kind === "Grid" && <path d="M-5 0H5M0 -5V5" />}
        </svg>
      )}
      <div className="cursor-readout">
        {placing && snapMark && cursor
          ? `X ${snapMark.point.x.toFixed(3)}   Y ${snapMark.point.y.toFixed(3)}` +
            (snapMark.kind === "None" ? "" : `   ${snapMark.kind}`)
          : cursor
            ? `X ${cursor.x.toFixed(3)}   Y ${cursor.y.toFixed(3)}`
            : mode === "3D"
              ? "3D view"
              : ""}
        {hint && mode === "2D" && <span className="draw-hint">{hint}</span>}
      </div>
      {children}
    </main>
  );
}
