// The drawing area. It only DRAWS what the engine sends (display point lists);
// it does no geometry or toolpath calculations of its own.

import { useEffect, useRef, useState } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import type { DisplayData } from "../engine";

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
  view: View2D;
  mode: "2D" | "3D";
  width: number;
  height: number;
  fitted: boolean;
  render: () => void;
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

export default function Viewport({ display, selectedId }: { display: DisplayData | null; selectedId: number | null }) {
  const containerRef = useRef<HTMLDivElement>(null);
  const sceneRef = useRef<Scene3 | null>(null);
  const [mode, setMode] = useState<"2D" | "3D">("2D");
  const [cursor, setCursor] = useState<{ x: number; y: number } | null>(null);

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

    const s: Scene3 = {
      renderer,
      scene,
      ortho,
      persp,
      orbit,
      shapes,
      view: { cx: 0, cy: 0, scale: 0.5 },
      mode: "2D",
      width: 1,
      height: 1,
      fitted: false,
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

    // 2D pan (middle or right drag) and zoom at cursor (wheel). 3D uses the orbit controls.
    const canvas = renderer.domElement;
    let panFrom: { x: number; y: number } | null = null;

    const onWheel = (e: WheelEvent) => {
      if (s.mode !== "2D") return;
      e.preventDefault();
      const rect = canvas.getBoundingClientRect();
      const p = screenToWorld(s, e.clientX - rect.left, e.clientY - rect.top);
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
      if (s.mode !== "2D" || (e.button !== 1 && e.button !== 2)) return;
      e.preventDefault();
      panFrom = { x: e.clientX, y: e.clientY };
      canvas.setPointerCapture(e.pointerId);
    };
    const onPointerMove = (e: PointerEvent) => {
      if (s.mode !== "2D") return;
      if (panFrom) {
        s.view.cx -= (e.clientX - panFrom.x) * s.view.scale;
        s.view.cy += (e.clientY - panFrom.y) * s.view.scale;
        panFrom = { x: e.clientX, y: e.clientY };
        applyOrtho(s);
        s.render();
      }
      const rect = canvas.getBoundingClientRect();
      setCursor(screenToWorld(s, e.clientX - rect.left, e.clientY - rect.top));
    };
    const onPointerUp = () => {
      panFrom = null;
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

  // Redraw shapes whenever the engine's display data or the selection changes.
  useEffect(() => {
    const s = sceneRef.current;
    if (!s) return;
    for (const child of [...s.shapes.children]) {
      const line = child as THREE.Line;
      line.geometry.dispose();
      (line.material as THREE.Material).dispose();
      s.shapes.remove(line);
    }
    if (!display) {
      s.render();
      return;
    }
    let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
    for (const shape of display.shapes) {
      const positions: number[] = [];
      for (const [x, y] of shape.points) {
        positions.push(x, y, 0);
        minX = Math.min(minX, x);
        minY = Math.min(minY, y);
        maxX = Math.max(maxX, x);
        maxY = Math.max(maxY, y);
      }
      const geom = new THREE.BufferGeometry();
      geom.setAttribute("position", new THREE.Float32BufferAttribute(positions, 3));
      const selected = shape.id === selectedId;
      const mat = new THREE.LineBasicMaterial({
        color: selected ? COLORS.selected : COLORS.shape,
        depthTest: false,
      });
      const line = shape.closed ? new THREE.LineLoop(geom, mat) : new THREE.Line(geom, mat);
      line.renderOrder = selected ? 11 : 10;
      s.shapes.add(line);
    }
    // Zoom to fit the drawing the first time it arrives.
    if (!s.fitted && Number.isFinite(minX)) {
      const w = Math.max(maxX - minX, 1);
      const h = Math.max(maxY - minY, 1);
      s.view = {
        cx: (minX + maxX) / 2,
        cy: (minY + maxY) / 2,
        scale: Math.max(w / s.width, h / s.height) * 1.3,
      };
      s.fitted = true;
      applyOrtho(s);
    }
    s.render();
  }, [display, selectedId]);

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
