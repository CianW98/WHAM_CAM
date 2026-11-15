// The drawing area. It only DRAWS what the engine sends (display point lists) and reports
// where the user clicked or dragged, in mm. Deciding which shape was clicked is done by the engine.

import { useEffect, useRef, useState } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import type { BoxMode, DisplayData } from "../engine";

const COLORS = {
  background: 0x2b2b33,
  gridMinor: 0x363640,
  gridMajor: 0x4a4a58,
  axisX: 0xd03030,
  axisY: 0x30b030,
  shape: 0xff4a4a,
  selected: 0xffd24a,
};
const GRID_EXTENT = 1000; // mm either side of the origin
const GRID_MINOR = 10; // mm
const GRID_MAJOR = 50; // mm
const ZOOM_STEP = 1.15;
const PICK_RADIUS_PX = 6; // how close (in screen pixels) a click must be to a shape
const DRAG_THRESHOLD_PX = 4; // movement before a click becomes a box drag
const MARKER_SIZE_PX = 7;

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
  /** Drawn objects by engine id, so selection can recolour them without rebuilding. */
  byId: Map<number, THREE.Object3D[]>;
  materials: {
    line: THREE.LineBasicMaterial;
    lineSelected: THREE.LineBasicMaterial;
    marker: THREE.PointsMaterial;
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

interface Props {
  display: DisplayData | null;
  selectedIds: Set<number>;
  /** Changes whenever the view should zoom to fit the drawing. */
  fitRequest: number;
  onPick: (x: number, y: number, tolerance: number, add: boolean) => void;
  onBox: (x0: number, y0: number, x1: number, y1: number, mode: BoxMode, add: boolean) => void;
}

function lineSegments(positions: number[], color: number, order: number): THREE.LineSegments {
  const geom = new THREE.BufferGeometry();
  geom.setAttribute("position", new THREE.Float32BufferAttribute(positions, 3));
  const mat = new THREE.LineBasicMaterial({ color, depthTest: false });
  const lines = new THREE.LineSegments(geom, mat);
  lines.renderOrder = order;
  return lines;
}

function buildGrid(): THREE.Group {
  const minor: number[] = [];
  const major: number[] = [];
  for (let v = -GRID_EXTENT; v <= GRID_EXTENT; v += GRID_MINOR) {
    if (v === 0) continue; // the axes are drawn there
    const target = v % GRID_MAJOR === 0 ? major : minor;
    target.push(v, -GRID_EXTENT, 0, v, GRID_EXTENT, 0);
    target.push(-GRID_EXTENT, v, 0, GRID_EXTENT, v, 0);
  }
  const group = new THREE.Group();
  group.add(lineSegments(minor, COLORS.gridMinor, 0));
  group.add(lineSegments(major, COLORS.gridMajor, 1));
  group.add(lineSegments([-GRID_EXTENT, 0, 0, GRID_EXTENT, 0, 0], COLORS.axisX, 2));
  group.add(lineSegments([0, -GRID_EXTENT, 0, 0, GRID_EXTENT, 0], COLORS.axisY, 2));
  return group;
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

export default function Viewport({ display, selectedIds, fitRequest, onPick, onBox }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const sceneRef = useRef<Scene3 | null>(null);
  const [mode, setMode] = useState<"2D" | "3D">("2D");
  const [cursor, setCursor] = useState<{ x: number; y: number } | null>(null);
  const [box, setBox] = useState<BoxDrag | null>(null);

  // Latest callbacks, so the once-only event listeners below always call the current ones.
  const callbacks = useRef({ onPick, onBox });
  callbacks.current = { onPick, onBox };

  // Create the WebGL scene once.
  useEffect(() => {
    const container = containerRef.current!;
    const renderer = new THREE.WebGLRenderer({ antialias: true });
    renderer.setPixelRatio(window.devicePixelRatio);
    renderer.setClearColor(COLORS.background);
    container.appendChild(renderer.domElement);

    const scene = new THREE.Scene();
    scene.add(buildGrid());
    const shapes = new THREE.Group();
    scene.add(shapes);

    const ortho = new THREE.OrthographicCamera(-1, 1, 1, -1, 0.1, 5000);
    const persp = new THREE.PerspectiveCamera(45, 1, 0.1, 20000);
    persp.up.set(0, 0, 1);
    const orbit = new OrbitControls(persp, renderer.domElement);
    orbit.enabled = false;

    const marker = (color: number) =>
      new THREE.PointsMaterial({ color, size: MARKER_SIZE_PX, sizeAttenuation: false, depthTest: false });
    const s: Scene3 = {
      renderer,
      scene,
      ortho,
      persp,
      orbit,
      shapes,
      byId: new Map(),
      materials: {
        line: new THREE.LineBasicMaterial({ color: COLORS.shape, depthTest: false }),
        lineSelected: new THREE.LineBasicMaterial({ color: COLORS.selected, depthTest: false }),
        marker: marker(COLORS.shape),
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

    // 2D mouse: left click = pick, left drag = box select, middle/right drag = pan, wheel = zoom.
    // In 3D the orbit controls take over.
    const canvas = renderer.domElement;
    let panFrom: { x: number; y: number } | null = null;
    let leftDown: { x: number; y: number; dragging: boolean } | null = null;
    const local = (e: MouseEvent) => {
      const rect = canvas.getBoundingClientRect();
      return { x: e.clientX - rect.left, y: e.clientY - rect.top };
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
    };
    const onPointerDown = (e: PointerEvent) => {
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
        if (leftDown.dragging) setBox({ x0: leftDown.x, y0: leftDown.y, x1: m.x, y1: m.y });
      }
      setCursor(screenToWorld(s, m.x, m.y));
    };
    const onPointerUp = (e: PointerEvent) => {
      panFrom = null;
      if (!leftDown || e.button !== 0) return;
      const m = local(e);
      const add = e.shiftKey || e.ctrlKey;
      if (leftDown.dragging) {
        const a = screenToWorld(s, leftDown.x, leftDown.y);
        const b = screenToWorld(s, m.x, m.y);
        // CAD convention: drag left-to-right = fully inside; right-to-left = touching.
        const mode: BoxMode = m.x >= leftDown.x ? "Window" : "Crossing";
        callbacks.current.onBox(a.x, a.y, b.x, b.y, mode, add);
      } else {
        const p = screenToWorld(s, m.x, m.y);
        callbacks.current.onPick(p.x, p.y, PICK_RADIUS_PX * s.view.scale, add);
      }
      leftDown = null;
      setBox(null);
    };
    const onContextMenu = (e: Event) => e.preventDefault();
    const onLeave = () => setCursor(null);

    canvas.addEventListener("wheel", onWheel, { passive: false });
    canvas.addEventListener("pointerdown", onPointerDown);
    canvas.addEventListener("pointermove", onPointerMove);
    canvas.addEventListener("pointerup", onPointerUp);
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
        ? new THREE.Points(geom, s.materials.marker)
        : shape.closed
          ? new THREE.LineLoop(geom, s.materials.line)
          : new THREE.Line(geom, s.materials.line);
      obj.renderOrder = 10;
      obj.userData.markers = shape.markers;
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
        o.material = obj.userData.markers
          ? selected
            ? s.materials.markerSelected
            : s.materials.marker
          : selected
            ? s.materials.lineSelected
            : s.materials.line;
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
          <button key={m} className={mode === m ? "active" : ""} onClick={() => setMode(m)}>
            {m}
          </button>
        ))}
      </div>
      <div className="cursor-readout">
        {cursor ? `X ${cursor.x.toFixed(3)}   Y ${cursor.y.toFixed(3)}` : mode === "3D" ? "3D view" : ""}
      </div>
    </main>
  );
}
