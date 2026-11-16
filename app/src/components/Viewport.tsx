// The drawing area. It only DRAWS what the engine sends (display point lists) and reports
// where the user clicked or dragged, in mm. Deciding which shape was clicked, where a point
// snaps to and what a shape being drawn looks like is all done by the engine.

import { useEffect, useRef, useState } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import * as engine from "../engine";
import type { BoxMode, DisplayData, DisplayShape, Draft, Point, SnapKind, SnapOptions } from "../engine";

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
const GRID_EXTENT = 1000; // mm either side of the origin (less for very fine grids)
const GRID_MAX_LINES = 500; // most grid lines either side of the origin
const GRID_MAJOR_EVERY = 5; // every 5th grid line is drawn brighter
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
  grid: THREE.Group;
  /** The dashed preview of whatever is being placed. */
  preview: THREE.Group;
  /** Re-checks the snap and preview at the last cursor position. */
  hover: () => void;
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
  selectedIds: Set<number>;
  /** Changes whenever the view should zoom to fit the drawing. */
  fitRequest: number;
  gridSpacing: number;
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
  onPlacePoint: (p: Point) => void;
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
function buildGrid(spacing: number): THREE.Group {
  const n = Math.max(1, Math.min(GRID_MAX_LINES, Math.floor(GRID_EXTENT / spacing)));
  const extent = n * spacing;
  const minor: number[] = [];
  const major: number[] = [];
  for (let i = -n; i <= n; i++) {
    if (i === 0) continue; // the axes are drawn there
    const v = i * spacing;
    const target = i % GRID_MAJOR_EVERY === 0 ? major : minor;
    target.push(v, -extent, 0, v, extent, 0);
    target.push(-extent, v, 0, extent, v, 0);
  }
  const group = new THREE.Group();
  group.add(lineSegments(minor, COLORS.gridMinor, 0));
  group.add(lineSegments(major, COLORS.gridMajor, 1));
  group.add(lineSegments([-extent, 0, 0, extent, 0, 0], COLORS.axisX, 2));
  group.add(lineSegments([0, -extent, 0, 0, extent, 0], COLORS.axisY, 2));
  return group;
}

/** Removes and frees everything in a group. */
function clearGroup(group: THREE.Group) {
  for (const child of [...group.children]) {
    const o = child as THREE.Line;
    o.geometry?.dispose();
    for (const m of [o.material].flat()) m?.dispose();
    group.remove(child);
  }
}

/** Draws the preview of what is being placed: dashed lines, or markers for point lists. */
function showPreview(s: Scene3, shapes: DisplayShape[]) {
  clearGroup(s.preview);
  for (const shape of shapes) {
    if (shape.points.length === 0) continue;
    const positions: number[] = [];
    for (const [x, y] of shape.points) positions.push(x, y, 0);
    const geom = new THREE.BufferGeometry();
    geom.setAttribute("position", new THREE.Float32BufferAttribute(positions, 3));
    let obj: THREE.Object3D;
    if (shape.markers) {
      obj = new THREE.Points(
        geom,
        new THREE.PointsMaterial({ color: COLORS.draft, size: MARKER_SIZE_PX, sizeAttenuation: false, depthTest: false }),
      );
    } else {
      const mat = new THREE.LineDashedMaterial({
        color: COLORS.draft,
        dashSize: DASH_PX * s.view.scale,
        gapSize: GAP_PX * s.view.scale,
        depthTest: false,
      });
      const line = shape.closed ? new THREE.LineLoop(geom, mat) : new THREE.Line(geom, mat);
      line.computeLineDistances();
      obj = line;
    }
    obj.renderOrder = 12;
    s.preview.add(obj);
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
}

export default function Viewport({
  display,
  selectedIds,
  fitRequest,
  gridSpacing,
  mode,
  onMode,
  placing,
  hint,
  snapOptions,
  onSnapOptions,
  onPick,
  onBox,
  onPlacePoint,
  onFinishDraft,
  onContextMenu,
}: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const sceneRef = useRef<Scene3 | null>(null);
  const [cursor, setCursor] = useState<{ x: number; y: number } | null>(null);
  const [box, setBox] = useState<BoxDrag | null>(null);
  const [snapMark, setSnapMark] = useState<SnapMark | null>(null);

  // Latest props, so the once-only event listeners below always use the current ones.
  const live = useRef({ onPick, onBox, onPlacePoint, onFinishDraft, onContextMenu, placing, snapOptions });
  live.current = { onPick, onBox, onPlacePoint, onFinishDraft, onContextMenu, placing, snapOptions };

  // Create the WebGL scene once.
  useEffect(() => {
    const container = containerRef.current!;
    const renderer = new THREE.WebGLRenderer({ antialias: true });
    renderer.setPixelRatio(window.devicePixelRatio);
    renderer.setClearColor(COLORS.background);
    container.appendChild(renderer.domElement);

    const scene = new THREE.Scene();
    const grid = buildGrid(10);
    scene.add(grid);
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
      grid,
      preview: previewGroup,
      hover: () => {},
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
    let leftDown: { x: number; y: number; dragging: boolean } | null = null;
    let mouse: { x: number; y: number } | null = null;
    const local = (e: MouseEvent) => {
      const rect = canvas.getBoundingClientRect();
      return { x: e.clientX - rect.left, y: e.clientY - rect.top };
    };
    const snapAt = (m: { x: number; y: number }) => {
      const w = screenToWorld(s, m.x, m.y);
      const { placing, snapOptions } = live.current;
      const extra = placing?.kind === "draw" ? placing.draft.points : [];
      return engine.snap(w.x, w.y, SNAP_RADIUS_PX * s.view.scale, snapOptions, extra);
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
            showPreview(s, []);
            setSnapMark(null);
            break;
          }
          const snap = await snapAt(mouse);
          const shapes = await previewAt(placing, snap.point);
          if (hoverAgain) continue;
          showPreview(s, live.current.placing ? shapes : []);
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
    };
    const onPointerDown = (e: PointerEvent) => {
      if (e.button === 2) rightDown = { x: e.clientX, y: e.clientY, moved: false };
      if (s.mode !== "2D") return;
      if (e.button === 1 || e.button === 2) {
        e.preventDefault();
        panFrom = { x: e.clientX, y: e.clientY };
      } else if (e.button === 0) {
        const m = local(e);
        leftDown = { ...m, dragging: false };
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
        if (leftDown.dragging && !live.current.placing) setBox({ x0: leftDown.x, y0: leftDown.y, x1: m.x, y1: m.y });
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
          live.current.onPlacePoint(snap.point);
        });
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

  // Rebuild the grid when its spacing changes.
  useEffect(() => {
    const s = sceneRef.current;
    if (!s || !(gridSpacing > 0)) return;
    s.scene.remove(s.grid);
    clearGroup(s.grid);
    s.grid = buildGrid(gridSpacing);
    s.scene.add(s.grid);
    s.render();
  }, [gridSpacing]);

  // Update the preview and snap marker when what is being placed or the snap settings change
  // (also clears them when placing stops).
  useEffect(() => {
    sceneRef.current?.hover();
  }, [placing, snapOptions, display]);

  // Rebuild the drawn shapes when the engine's display data changes.
  useEffect(() => {
    const s = sceneRef.current;
    if (!s) return;
    for (const child of [...s.shapes.children]) {
      (child as THREE.Line).geometry.dispose();
      s.shapes.remove(child);
    }
    s.byId.clear();
    for (const shape of display?.shapes ?? []) {
      const positions: number[] = [];
      for (const [x, y] of shape.points) positions.push(x, y, 0);
      const geom = new THREE.BufferGeometry();
      geom.setAttribute("position", new THREE.Float32BufferAttribute(positions, 3));
      const obj: THREE.Object3D = shape.markers
        ? new THREE.Points(geom, s.materials.marker(shape.color))
        : shape.closed
          ? new THREE.LineLoop(geom, s.materials.line(shape.color))
          : new THREE.Line(geom, s.materials.line(shape.color));
      obj.renderOrder = 10;
      obj.userData.markers = shape.markers;
      obj.userData.color = shape.color;
      s.shapes.add(obj);
      const list = s.byId.get(shape.id) ?? [];
      list.push(obj);
      s.byId.set(shape.id, list);
    }
    s.render();
  }, [display]);

  // Recolour selected shapes.
  useEffect(() => {
    const s = sceneRef.current;
    if (!s) return;
    for (const [id, objs] of s.byId) {
      const selected = selectedIds.has(id);
      for (const obj of objs) {
        const o = obj as THREE.Line | THREE.Points;
        const color: string = obj.userData.color;
        o.material = obj.userData.markers
          ? selected
            ? s.materials.markerSelected
            : s.materials.marker(color)
          : selected
            ? s.materials.lineSelected
            : s.materials.line(color);
        o.renderOrder = selected ? 11 : 10;
      }
    }
    s.render();
  }, [display, selectedIds]);

  // Zoom to fit: on request, and the first time there is something to show.
  const lastFit = useRef<number | null>(null);
  useEffect(() => {
    const s = sceneRef.current;
    const bounds = display?.bounds;
    if (!s || !bounds) return;
    if (lastFit.current !== null && lastFit.current === fitRequest) return;
    lastFit.current = fitRequest;
    fitTo(s, bounds);
    s.render();
  }, [fitRequest, display]);

  // Switch between 2D (top view) and 3D (orbit).
  useEffect(() => {
    const s = sceneRef.current;
    if (!s) return;
    s.mode = mode;
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
    <main className="viewport" ref={containerRef}>
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
    </main>
  );
}
