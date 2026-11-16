import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import * as engine from "./engine";
import {
  CLICKS_NEEDED,
  type BoxMode,
  type Draft,
  type DrawTool,
  type ImportReport,
  type Point,
  type Shape,
  type SnapOptions,
  type TransformOp,
  type View,
} from "./engine";
import { useNotify } from "./components/Notifications";
import Toolbar from "./components/Toolbar";
import Tree from "./components/Tree";
import Properties from "./components/Properties";
import Viewport, { type Placing, type ViewMode } from "./components/Viewport";
import ChoiceDialog, { type Choice } from "./components/ChoiceDialog";
import ContextMenu, { type MenuItem } from "./components/ContextMenu";
import NumberDialog, { type NumberQuestion } from "./components/NumberDialog";
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

/** Moving or rotating by clicking, or pasting: what is being placed besides a drawn shape. */
type Special = Exclude<Placing, { kind: "draw" }>;

function specialHint(sp: Special): string {
  switch (sp.kind) {
    case "paste":
      return "Click where the centre of the pasted shapes goes · Esc cancels";
    case "move":
      return `${sp.base ? "Click the destination" : "Click the base point"} · Esc cancels`;
    case "rotate":
      return `${sp.reference ? "Click to set the new angle" : "Click to set the reference direction"} · Esc cancels`;
  }
}

const plural = (n: number, what: string) => `${n} ${what}${n === 1 ? "" : "s"}`;

const samePoint = (a: Point | undefined, b: Point) => a !== undefined && a.x === b.x && a.y === b.y;

export default function App() {
  const notify = useNotify();
  const [view, setView] = useState<View | null>(null);
  const [selection, setSelection] = useState<Selection>(NOTHING);
  const [fitRequest, setFitRequest] = useState(0);
  const [choice, setChoice] = useState<(Choice & { resolve: (a: string | null) => void }) | null>(null);
  const [snapOptions, setSnapOptions] = useState<SnapOptions>({ grid: true, objects: true });
  const [currentLayer, setCurrentLayer] = useState<string | null>(null);
  const [viewMode, setViewMode] = useState<ViewMode>("2D");
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const [numbers, setNumbers] = useState<(NumberQuestion & { resolve: (v: number[] | null) => void }) | null>(
    null,
  );
  const [hasClipboard, setHasClipboard] = useState(false);
  // Last values typed into the transform dialogs, offered again next time.
  const lastValues = useRef({ dx: 0, dy: 0, degrees: 90, factor: 1 });

  // Moving by clicking / pasting (null = neither). A ref as well, like the draft below.
  const [special, setSpecialState] = useState<Special | null>(null);
  const specialRef = useRef<Special | null>(null);
  const setSpecial = useCallback((sp: Special | null) => {
    specialRef.current = sp;
    setSpecialState(sp);
  }, []);

  // The shape being drawn (null = no drawing tool active). Kept in a ref as well, so clicks
  // that arrive before React re-renders always build on the latest points.
  const [draft, setDraftState] = useState<Draft | null>(null);
  const draftRef = useRef<Draft | null>(null);
  const setDraft = useCallback(
    (d: Draft | null) => {
      draftRef.current = d;
      setDraftState(d);
      if (d) setSpecial(null); // drawing and moving/pasting never run together
    },
    [setSpecial],
  );

  /** Ask the user a question in a dialog. */
  const ask = useCallback(
    (c: Choice) => new Promise<string | null>((resolve) => setChoice({ ...c, resolve })),
    [],
  );

  /** Ask for one or more numbers in a dialog. Resolves null if cancelled. */
  const askNumbers = useCallback(
    (q: NumberQuestion) => new Promise<number[] | null>((resolve) => setNumbers({ ...q, resolve })),
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
    engine.hasClipboard().then(setHasClipboard, () => {});
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

  const ids = selectedIds(selection);

  /** Stops drawing, moving and pasting. */
  const stopPlacing = useCallback(() => {
    setDraft(null);
    setSpecial(null);
  }, [setDraft, setSpecial]);

  const applyTransform = useCallback(
    async (targets: number[], op: TransformOp) => {
      if (targets.length === 0) return;
      await engine.transform(targets, op);
      await refresh();
    },
    [refresh],
  );

  /** Move… / Rotate… / Scale…: ask for the value, then transform the selection. */
  const transformDialog = useCallback(
    async (kind: "Move" | "Rotate" | "Scale") => {
      const targets = ids;
      if (targets.length === 0) return;
      const last = lastValues.current;
      const n = plural(targets.length, "object");
      if (kind === "Move") {
        const v = await askNumbers({
          title: `Move ${n}`,
          fields: [
            { label: "Move X by (mm)", value: last.dx },
            { label: "Move Y by (mm)", value: last.dy },
          ],
        });
        if (!v) return;
        [last.dx, last.dy] = v;
        await applyTransform(targets, { type: "Move", dx: v[0], dy: v[1] });
      } else if (kind === "Rotate") {
        const v = await askNumbers({
          title: `Rotate ${n} around their centre`,
          fields: [{ label: "Angle (°, + = anticlockwise)", value: last.degrees }],
        });
        if (!v) return;
        last.degrees = v[0];
        await applyTransform(targets, { type: "Rotate", degrees: v[0] });
      } else {
        const v = await askNumbers({
          title: `Scale ${n} around their centre`,
          fields: [{ label: "Scale factor", value: last.factor }],
        });
        if (!v) return;
        last.factor = v[0];
        await applyTransform(targets, { type: "Scale", factor: v[0] });
      }
    },
    [ids, askNumbers, applyTransform],
  );

  const selectAll = useCallback(async () => {
    setSelection({ kind: "objects", ids: await engine.allIds() });
  }, []);

  const selectAllOnLayer = useCallback(async (layer: string) => {
    setSelection({ kind: "objects", ids: await engine.idsOnLayer(layer) });
  }, []);

  const copySelected = useCallback(async () => {
    if (ids.length === 0) return;
    await engine.copy(ids);
    setHasClipboard(true);
    notify("info", `Copied ${plural(ids.length, "object")}.`);
  }, [ids, notify]);

  const cutSelected = useCallback(async () => {
    if (ids.length === 0) return;
    const n = await engine.cut(ids);
    setHasClipboard(true);
    await refresh();
    notify("info", `Cut ${plural(n, "object")}.`);
  }, [ids, notify, refresh]);

  const startPaste = useCallback(() => {
    if (!hasClipboard) return;
    setDraft(null);
    setSpecial({ kind: "paste" });
  }, [hasClipboard, setDraft, setSpecial]);

  const startMoveByClicking = useCallback(() => {
    if (ids.length === 0) return;
    setDraft(null);
    setSpecial({ kind: "move", ids, base: null });
  }, [ids, setDraft, setSpecial]);

  const startRotateByClicking = useCallback(async () => {
    if (ids.length === 0) return;
    const centre = await engine.selectionCentre(ids);
    if (!centre) return;
    setDraft(null);
    setSpecial({ kind: "rotate", ids, centre, reference: null });
  }, [ids, setDraft, setSpecial]);

  /** A snapped click while placing: a drawing point, a move/rotate step, or a paste. */
  const placePoint = useCallback(
    (p: Point) => {
      if (draftRef.current) return addDrawPoint(p);
      const sp = specialRef.current;
      if (!sp) return;
      if (sp.kind === "move" && !sp.base) return setSpecial({ ...sp, base: p });
      if (sp.kind === "rotate" && !sp.reference) {
        // A click on the centre itself gives no direction: wait for another.
        if (!samePoint(sp.centre, p)) setSpecial({ ...sp, reference: p });
        return;
      }
      setSpecial(null);
      if (sp.kind === "rotate" && sp.reference) {
        const from = sp.reference;
        run(() => applyTransform(sp.ids, { type: "RotateFromTo", from, to: p }));
      } else if (sp.kind === "move" && sp.base) {
        const base = sp.base;
        run(() => applyTransform(sp.ids, { type: "Move", dx: p.x - base.x, dy: p.y - base.y }));
      } else if (sp.kind === "paste") {
        run(async () => {
          const pasted = await engine.paste(p);
          await refresh();
          setSelection({ kind: "objects", ids: pasted });
        });
      }
    },
    [addDrawPoint, applyTransform, refresh, run, setSpecial],
  );

  const onAction = (name: string) => {
    const tool = TOOL_BUTTONS[name];
    if (tool) {
      // Pressing the active tool again stops drawing.
      if (draftRef.current?.tool === tool) stopPlacing();
      else setDraft(emptyDraft(tool));
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
    if (choice || numbers) return;
    const key = e.key.toLowerCase();
    if (menu) {
      if (key === "escape") {
        setMenu(null);
        e.preventDefault();
      }
      return;
    }
    const target = e.target as HTMLElement | null;
    if (target && ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName)) return;
    const ctrl = e.ctrlKey || e.metaKey;
    const d = draftRef.current;
    if (key === "escape") {
      // Esc: cancel a move/paste or the shape in progress, then stop drawing, then clear the selection.
      if (specialRef.current) setSpecial(null);
      else if (d && d.points.length > 0) setDraft(emptyDraft(d.tool));
      else if (d) setDraft(null);
      else setSelection(NOTHING);
    } else if (ctrl && key === "a") run(selectAll);
    else if (ctrl && key === "c") run(copySelected);
    else if (ctrl && key === "x") run(cutSelected);
    else if (ctrl && key === "v") startPaste();
    else if (d && key === "enter") finishDraft();
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

  const selectedSet = useMemo(() => new Set(ids), [ids]);
  const onSelect = (sel: Selection) => {
    setSelection(sel);
    if (sel.kind === "layer" && layers[sel.layer]) setCurrentLayer(layers[sel.layer].name);
  };

  const disabled = new Set<string>();
  if (!view?.canUndo) disabled.add("Undo");
  if (!view?.canRedo) disabled.add("Redo");
  if (ids.length === 0 && selection.kind !== "layer") disabled.add("Delete");

  const placing = useMemo<Placing | null>(
    () => (draft ? { kind: "draw", draft } : special),
    [draft, special],
  );

  // The right-click menu. Items that can't be used right now are greyed.
  const menuItems = (): MenuItem[] => {
    const none = ids.length === 0;
    const later = (label: string): MenuItem => ({ label, disabled: true });
    const draw = (label: string): MenuItem => ({ label, action: () => onAction(label) });
    return [
      {
        label: "View",
        items: [
          { label: "Zoom to fit", action: () => setFitRequest((n) => n + 1) },
          { label: "2D view", disabled: viewMode === "2D", action: () => setViewMode("2D") },
          { label: "3D view", disabled: viewMode === "3D", action: () => setViewMode("3D") },
        ],
      },
      { label: "Draw", items: ["Line", "Arc", "Circle", "Rectangle", "Polyline", "Point"].map(draw) },
      {
        label: "Transform",
        disabled: none,
        items: [
          { label: "Move…", action: () => run(() => transformDialog("Move")) },
          { label: "Move by clicking", action: startMoveByClicking },
          { label: "Rotate…", action: () => run(() => transformDialog("Rotate")) },
          { label: "Rotate by clicking", action: () => run(startRotateByClicking) },
          { label: "Scale…", action: () => run(() => transformDialog("Scale")) },
          {
            label: "Mirror left–right",
            separatorBefore: true,
            action: () => run(() => applyTransform(ids, { type: "MirrorLeftRight" })),
          },
          { label: "Mirror top–bottom", action: () => run(() => applyTransform(ids, { type: "MirrorTopBottom" })) },
        ],
      },
      {
        label: "Edit",
        items: [
          { label: "Delete", shortcut: "Del", disabled: disabled.has("Delete"), action: () => run(deleteSelected) },
          { ...later("Join"), separatorBefore: true },
          later("Split / Explode"),
          later("Close polyline"),
          later("Dimension"),
        ],
      },
      { label: "Select All", shortcut: "Ctrl+A", separatorBefore: true, action: () => run(selectAll) },
      { label: `Select All On Layer "${drawLayer}"`, action: () => run(() => selectAllOnLayer(drawLayer)) },
      { label: "Cut", shortcut: "Ctrl+X", separatorBefore: true, disabled: none, action: () => run(cutSelected) },
      { label: "Copy", shortcut: "Ctrl+C", disabled: none, action: () => run(copySelected) },
      { label: "Paste", shortcut: "Ctrl+V", disabled: !hasClipboard, action: startPaste },
      {
        label: "Undo",
        shortcut: "Ctrl+Z",
        separatorBefore: true,
        disabled: !view?.canUndo,
        action: () => run(() => undoRedo("undo")),
      },
      { label: "Redo", shortcut: "Ctrl+Y", disabled: !view?.canRedo, action: () => run(() => undoRedo("redo")) },
    ];
  };

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
        mode={viewMode}
        onMode={setViewMode}
        placing={placing}
        hint={draft ? drawHint(draft) : special ? specialHint(special) : null}
        snapOptions={snapOptions}
        onSnapOptions={setSnapOptions}
        onPick={onPick}
        onBox={onBox}
        onPlacePoint={placePoint}
        onFinishDraft={finishDraft}
        onContextMenu={(x, y) => setMenu({ x, y })}
      />
      {menu && <ContextMenu items={menuItems()} x={menu.x} y={menu.y} onClose={() => setMenu(null)} />}
      {numbers && (
        <NumberDialog
          question={numbers}
          onAnswer={(v) => {
            numbers.resolve(v);
            setNumbers(null);
          }}
        />
      )}
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
