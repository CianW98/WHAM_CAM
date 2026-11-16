import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import * as engine from "./engine";
import { CLICKS_NEEDED, type BoxMode, type Draft, type DrawTool, type ImportReport, type Point, type Shape, type SnapOptions, type View } from "./engine";
import { useNotify } from "./components/Notifications";
import Toolbar from "./components/Toolbar";
import Tree from "./components/Tree";
import Properties from "./components/Properties";
import Viewport from "./components/Viewport";
import ChoiceDialog, { type Choice } from "./components/ChoiceDialog";
import { NOTHING, selectObjects, selectedIds, type Selection } from "./selection";

function importMessage(r: ImportReport, unitsName: string | null): string {
  const layers = r.layers.length === 1 ? "1 layer" : `${r.layers.length} layers`;
  let msg =
    r.objects_added === 0
      ? "Nothing was imported."
      : `Imported ${r.objects_added} object${r.objects_added === 1 ? "" : "s"} on ${layers}.`;
  if (r.scale !== 1 && unitsName) msg += ` Converted from ${unitsName} to mm.`;
  const skipped = Object.entries(r.skipped);
  if (skipped.length > 0) msg += ` Skipped: ${skipped.map(([what, n]) => `${n} ${what}`).join(", ")}.`;
  return msg;
}

/** Toolbar button → drawing tool. */
const TOOL_BUTTONS: Record<string, DrawTool> = {
  Line: "Line",
  Arc: "Arc",
  Circle: "Circle",
  Rectangle: "Rectangle",
  Polyline: "Polyline",
  Point: "Points",
};
const buttonForTool = (tool: DrawTool) => Object.keys(TOOL_BUTTONS).find((b) => TOOL_BUTTONS[b] === tool) ?? null;

const emptyDraft = (tool: DrawTool): Draft => ({ tool, points: [], arcs: [], arc_next: false });

/** What to tell the user at each step of drawing. */
function drawHint(d: Draft): string {
  const n = d.points.length;
  const steps: Partial<Record<DrawTool, string[]>> = {
    Line: ["Click the start point", "Click the end point"],
    Arc: ["Click the start point", "Click the end point", "Click the centre"],
    Circle: ["Click the centre", "Click a point on the circle"],
    Rectangle: ["Click the first corner", "Click the opposite corner"],
  };
  let hint: string;
  if (d.tool === "Polyline") {
    hint =
      n === 0
        ? "Click the first point"
        : `Click the next point (${d.arc_next ? "arc" : "line"}: A = arc, L = line) · Enter or double-click finishes` +
          (n >= 2 ? " · click the first point to close" : "") +
          " · Backspace removes a point";
  } else if (d.tool === "Points") {
    hint = "Click to add points · Enter or double-click finishes" + (n > 0 ? " · Backspace removes a point" : "");
  } else {
    hint = steps[d.tool]![n] ?? "";
  }
  return `${hint} · Esc ${n > 0 ? "cancels" : "stops drawing"}`;
}

const samePoint = (a: Point | undefined, b: Point) => a !== undefined && a.x === b.x && a.y === b.y;

export default function App() {
  const notify = useNotify();
  const [view, setView] = useState<View | null>(null);
  const [selection, setSelection] = useState<Selection>(NOTHING);
  const [fitRequest, setFitRequest] = useState(0);
  const [choice, setChoice] = useState<(Choice & { resolve: (a: string | null) => void }) | null>(null);
  const [snapOptions, setSnapOptions] = useState<SnapOptions>({ grid: true, objects: true });
  const [currentLayer, setCurrentLayer] = useState<string | null>(null);

  // The shape being drawn (null = no drawing tool active). Kept in a ref as well, so clicks
  // that arrive before React re-renders always build on the latest points.
  const [draft, setDraftState] = useState<Draft | null>(null);
  const draftRef = useRef<Draft | null>(null);
  const setDraft = useCallback((d: Draft | null) => {
    draftRef.current = d;
    setDraftState(d);
  }, []);

  /** Ask the user a question in a dialog. */
  const ask = useCallback(
    (c: Choice) => new Promise<string | null>((resolve) => setChoice({ ...c, resolve })),
    [],
  );

  /** Reload everything from the engine after a change, dropping selected ids that no longer exist. */
  const refresh = useCallback(async () => {
    const v = await engine.getView();
    setView(v);
    const existing = new Set(v.display.shapes.map((s) => s.id));
    setSelection((sel) =>
      sel.kind === "objects" ? { kind: "objects", ids: sel.ids.filter((id) => existing.has(id)) } : sel,
    );
    return v;
  }, []);

  useEffect(() => {
    refresh().catch((err) => notify("error", `Could not reach the CAM engine: ${String(err)}`));
  }, [refresh, notify]);

  const importDxf = useCallback(async () => {
    const path = await engine.chooseDxfFile();
    if (!path) return;
    const info = await engine.dxfInfo(path);
    let scale = 1;
    const mmPerUnit = info.units.mm_per_unit;
    if (mmPerUnit !== null && mmPerUnit !== 1) {
      const convert = `Convert ${info.units.name} to mm (×${mmPerUnit})`;
      const answer = await ask({
        title: "DXF units",
        message: `This DXF says it was drawn in ${info.units.name}. WHAM_CAM works in millimetres.`,
        options: [convert, "Treat the numbers as mm"],
      });
      if (answer === null) return;
      if (answer === convert) scale = mmPerUnit;
    }
    const report = await engine.importDxf(path, scale);
    await refresh();
    setSelection(NOTHING);
    setFitRequest((n) => n + 1);
    notify("info", importMessage(report, scale !== 1 ? info.units.name : null));
  }, [ask, notify, refresh]);

  /** Deletes the selected objects, or the selected layer (with everything on it). */
  const deleteSelected = useCallback(async () => {
    if (selection.kind === "layer") {
      const layer = view?.document.layers[selection.layer];
      if (!layer) return;
      await engine.deleteLayer(layer.name);
      setSelection(NOTHING);
      await refresh();
      return;
    }
    const ids = selectedIds(selection);
    if (ids.length === 0) return;
    await engine.deleteObjects(ids);
    await refresh();
  }, [selection, view, refresh]);

  const renameLayer = useCallback(
    async (oldName: string, newName: string) => {
      try {
        await engine.renameLayer(oldName, newName);
        setCurrentLayer((c) => (c === oldName ? newName.trim() : c));
        await refresh();
        return true;
      } catch (err) {
        notify("error", String(err));
        return false;
      }
    },
    [notify, refresh],
  );

  const setLayerColor = useCallback(
    (name: string, color: string) => {
      engine
        .setLayerColor(name, color)
        .then(refresh)
        .catch((err) => notify("error", String(err)));
    },
    [notify, refresh],
  );

  /** Sends an edit from the Properties panel. Resolves false (with a pop-up) if refused. */
  const tryEdit = useCallback(
    async (change: () => Promise<void>) => {
      try {
        await change();
        await refresh();
        return true;
      } catch (err) {
        notify("error", String(err));
        return false;
      }
    },
    [notify, refresh],
  );
  const setShape = useCallback(
    (id: number, shape: Shape) => tryEdit(() => engine.setShape(id, shape)),
    [tryEdit],
  );
  const setGridSpacing = useCallback((mm: number) => tryEdit(() => engine.setGridSpacing(mm)), [tryEdit]);

  const undoRedo = useCallback(
    async (which: "undo" | "redo") => {
      const done = await (which === "undo" ? engine.undo() : engine.redo());
      if (done) await refresh();
    },
    [refresh],
  );

  // New drawings go onto the current layer; if it no longer exists, the first layer.
  const layers = view?.document.layers ?? [];
  const drawLayer = layers.find((l) => l.name === currentLayer)?.name ?? layers[0]?.name ?? "Default";
  const drawLayerRef = useRef(drawLayer);
  drawLayerRef.current = drawLayer;

  /** Adds the finished shape to the drawing and starts the next one with the same tool. */
  const commitDraft = useCallback(
    async (d: Draft) => {
      setDraft(emptyDraft(d.tool));
      try {
        const id = await engine.addDraft(drawLayerRef.current, d);
        await refresh();
        // The finished shape becomes the selection, so its values show in Properties.
        setSelection({ kind: "objects", ids: [id] });
      } catch (err) {
        notify("error", String(err));
      }
    },
    [notify, refresh, setDraft],
  );

  const addDrawPoint = useCallback(
    (p: Point) => {
      const d = draftRef.current;
      if (!d) return;
      // A second click on the same spot (e.g. a double-click) adds nothing.
      if (samePoint(d.points[d.points.length - 1], p)) return;
      const next: Draft = {
        ...d,
        points: [...d.points, p],
        arcs: d.tool === "Polyline" && d.points.length > 0 ? [...d.arcs, d.arc_next] : d.arcs,
      };
      const closes = d.tool === "Polyline" && d.points.length >= 2 && samePoint(d.points[0], p);
      if (CLICKS_NEEDED[d.tool] === next.points.length || closes) void commitDraft(next);
      else setDraft(next);
    },
    [commitDraft, setDraft],
  );

  /** Enter or double-click: finishes a polyline or point list. */
  const finishDraft = useCallback(() => {
    const d = draftRef.current;
    if (!d || CLICKS_NEEDED[d.tool] !== null) return;
    const enough = d.tool === "Polyline" ? d.points.length >= 2 : d.points.length >= 1;
    if (enough) void commitDraft(d);
  }, [commitDraft]);

  /** Runs an action, turning any failure into an error pop-up. */
  const run = useCallback(
    (action: () => Promise<unknown>) => {
      action().catch((err) => notify("error", String(err)));
    },
    [notify],
  );

  const onAction = (name: string) => {
    const tool = TOOL_BUTTONS[name];
    if (tool) {
      // Pressing the active tool again stops drawing.
      setDraft(draftRef.current?.tool === tool ? null : emptyDraft(tool));
      return;
    }
    switch (name) {
      case "Import":
        return run(importDxf);
      case "Undo":
        return run(() => undoRedo("undo"));
      case "Redo":
        return run(() => undoRedo("redo"));
      case "Delete":
        return run(deleteSelected);
      case "Zoom to fit":
        return setFitRequest((n) => n + 1);
      default:
        notify("info", `${name} is not built yet.`);
    }
  };

  // Keyboard shortcuts (ignored while typing in a text box or while a dialog is open).
  const keyHandler = useRef<(e: KeyboardEvent) => void>(() => {});
  keyHandler.current = (e: KeyboardEvent) => {
    if (choice) return;
    const target = e.target as HTMLElement | null;
    if (target && ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName)) return;
    const ctrl = e.ctrlKey || e.metaKey;
    const key = e.key.toLowerCase();
    const d = draftRef.current;
    if (key === "escape") {
      // Esc: cancel the shape in progress, then stop drawing, then clear the selection.
      if (d && d.points.length > 0) setDraft(emptyDraft(d.tool));
      else if (d) setDraft(null);
      else setSelection(NOTHING);
    } else if (d && key === "enter") finishDraft();
    else if (d && key === "backspace") {
      const points = d.points.slice(0, -1);
      setDraft({ ...d, points, arcs: d.arcs.slice(0, Math.max(0, points.length - 1)) });
    } else if (d?.tool === "Polyline" && !ctrl && (key === "a" || key === "l")) {
      setDraft({ ...d, arc_next: key === "a" });
    } else if (key === "delete") run(deleteSelected);
    else if (ctrl && key === "z" && !e.shiftKey) run(() => undoRedo("undo"));
    else if (ctrl && (key === "y" || (key === "z" && e.shiftKey))) run(() => undoRedo("redo"));
    else return;
    e.preventDefault();
  };
  useEffect(() => {
    const h = (e: KeyboardEvent) => keyHandler.current(e);
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  }, []);

  const onPick = (x: number, y: number, tolerance: number, add: boolean) =>
    run(async () => {
      const id = await engine.pick(x, y, tolerance);
      if (id !== null) setSelection((sel) => selectObjects(sel, [id], add));
      else if (!add) setSelection(NOTHING);
    });

  const onBox = (x0: number, y0: number, x1: number, y1: number, mode: BoxMode, add: boolean) =>
    run(async () => {
      const ids = await engine.pickBox(x0, y0, x1, y1, mode);
      setSelection((sel) => selectObjects(sel, ids, add));
    });

  const ids = selectedIds(selection);
  const selectedSet = useMemo(() => new Set(ids), [ids]);
  const onSelect = (sel: Selection) => {
    setSelection(sel);
    if (sel.kind === "layer" && layers[sel.layer]) setCurrentLayer(layers[sel.layer].name);
  };

  const disabled = new Set<string>();
  if (!view?.canUndo) disabled.add("Undo");
  if (!view?.canRedo) disabled.add("Redo");
  if (ids.length === 0 && selection.kind !== "layer") disabled.add("Delete");

  return (
    <div className="app">
      <Toolbar onAction={onAction} disabled={disabled} active={draft ? buttonForTool(draft.tool) : null} />
      <div className="side">
        <section className="panel tree-panel">
          <h2>Project</h2>
          <Tree
            doc={view?.document ?? null}
            selection={selection}
            onSelect={onSelect}
            onRenameLayer={renameLayer}
            onSetLayerColor={setLayerColor}
            currentLayer={drawLayer}
          />
        </section>
        <section className="panel props-panel">
          <h2>Properties</h2>
          <Properties
            doc={view?.document ?? null}
            selection={selection}
            onRenameLayer={renameLayer}
            onSetLayerColor={setLayerColor}
            onSetShape={setShape}
            onSetGridSpacing={setGridSpacing}
          />
        </section>
      </div>
      <Viewport
        display={view?.display ?? null}
        selectedIds={selectedSet}
        fitRequest={fitRequest}
        gridSpacing={view?.document.grid_spacing ?? 10}
        draft={draft}
        hint={draft ? drawHint(draft) : null}
        snapOptions={snapOptions}
        onSnapOptions={setSnapOptions}
        onPick={onPick}
        onBox={onBox}
        onDrawPoint={addDrawPoint}
        onFinishDraft={finishDraft}
      />
      {choice && (
        <ChoiceDialog
          choice={choice}
          onAnswer={(a) => {
            choice.resolve(a);
            setChoice(null);
          }}
        />
      )}
    </div>
  );
}
