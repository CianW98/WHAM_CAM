import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import * as engine from "./engine";
import type { BoxMode, ImportReport, View } from "./engine";
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

export default function App() {
  const notify = useNotify();
  const [view, setView] = useState<View | null>(null);
  const [selection, setSelection] = useState<Selection>(NOTHING);
  const [fitRequest, setFitRequest] = useState(0);
  const [choice, setChoice] = useState<(Choice & { resolve: (a: string | null) => void }) | null>(null);

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

  const undoRedo = useCallback(
    async (which: "undo" | "redo") => {
      const done = await (which === "undo" ? engine.undo() : engine.redo());
      if (done) await refresh();
    },
    [refresh],
  );

  /** Runs an action, turning any failure into an error pop-up. */
  const run = useCallback(
    (action: () => Promise<unknown>) => {
      action().catch((err) => notify("error", String(err)));
    },
    [notify],
  );

  const onAction = (name: string) => {
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
    if (key === "escape") setSelection(NOTHING);
    else if (key === "delete") run(deleteSelected);
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
  const disabled = new Set<string>();
  if (!view?.canUndo) disabled.add("Undo");
  if (!view?.canRedo) disabled.add("Redo");
  if (ids.length === 0 && selection.kind !== "layer") disabled.add("Delete");

  return (
    <div className="app">
      <Toolbar onAction={onAction} disabled={disabled} />
      <div className="side">
        <section className="panel tree-panel">
          <h2>Project</h2>
          <Tree doc={view?.document ?? null} selection={selection} onSelect={setSelection} />
        </section>
        <section className="panel props-panel">
          <h2>Properties</h2>
          <Properties doc={view?.document ?? null} selection={selection} />
        </section>
      </div>
      <Viewport
        display={view?.display ?? null}
        selectedIds={selectedSet}
        fitRequest={fitRequest}
        onPick={onPick}
        onBox={onBox}
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
