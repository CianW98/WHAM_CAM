import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import * as engine from "./engine";
import {
  CLICKS_NEEDED,
  type BoxMode,
  type CutWidthStep,
  type Draft,
  type DrawTool,
  type ImportReport,
  type Operation,
  type Playback,
  type Point,
  type Shape,
  type GridArea,
  type SnapOptions,
  type Tool,
  type Tooling,
  type Machines,
  type ToolpathDisplay,
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
import MenuBar from "./components/MenuBar";
import NumberDialog, { type NumberQuestion } from "./components/NumberDialog";
import PlaybackBar from "./components/PlaybackBar";
import SetupTree from "./components/SetupTree";
import ToolListDialog from "./components/ToolListDialog";
import { NOTHING, selectObjects, selectedIds, type Selection } from "./selection";

function importMessage(r: ImportReport, unitsName: string | null): string {
  const layers = r.layers.length === 1 ? "1 layer" : `${r.layers.length} layers`;
  let msg =
    r.objects_added === 0
      ? "Nothing was imported."
      : `Imported ${r.objects_added} object${r.objects_added === 1 ? "" : "s"} on ${layers}.`;
  if (r.scale !== 1 && unitsName) msg += ` Converted from ${unitsName} to mm.`;
  if (r.curves_converted > 0)
    msg += ` ${r.curves_converted} curve${r.curves_converted === 1 ? "" : "s"} (splines and ellipses) converted into arcs and lines.`;
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
  Dimension: "Dimension",
};
/** One-key shortcuts for the drawing tools (no Ctrl/Alt). */
const TOOL_KEYS: Record<string, DrawTool> = {
  l: "Line",
  a: "Arc",
  c: "Circle",
  r: "Rectangle",
  p: "Polyline",
  ".": "Points",
  d: "Dimension",
};
const TOOL_KEY_LABELS: Record<string, string> = {
  Line: "L",
  Arc: "A",
  Circle: "C",
  Rectangle: "R",
  Polyline: "P",
  Point: ".",
  Dimension: "D",
};

const buttonForTool = (tool: DrawTool) => Object.keys(TOOL_BUTTONS).find((b) => TOOL_BUTTONS[b] === tool) ?? null;

const emptyDraft = (tool: DrawTool): Draft => ({ tool, points: [], arcs: [], arc_next: false, targets: [] });

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
  } else if (d.tool === "Dimension") {
    const kinds = d.targets.map((t) => t?.type ?? "").join(",");
    const hints: Record<string, string> = {
      "": "Click a circle, an arc, a line, or a shape's end or centre",
      Circle: "Click where the value goes",
      Line: "Click another line for the angle (or distance if parallel), or where the length goes",
      "Line,Line": "Click where the dimension goes (for an angle: inside the angle to measure)",
      Point: "Click a second end, midpoint or centre",
      "Point,Point": "Click where the dimension line goes",
    };
    hint = hints[kinds] ?? "";
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

/** Stable empty list, so the drawing area doesn't redraw toolpaths for nothing. */
const EMPTY_TOOLPATHS: ToolpathDisplay[] = [];
const EMPTY_CUT_WIDTHS: CutWidthStep[] = [];
const EMPTY_TOOLING: Tooling = { tools: [], libraries: [] };
const EMPTY_MACHINES: Machines = { machines: [] };

const plural = (n: number, what: string) => `${n} ${what}${n === 1 ? "" : "s"}`;

/** Cheap fingerprint of a playback, to tell when the toolpaths have changed. */
function playbackSignature(pb: Playback): string {
  let sum = 0;
  for (const st of pb.steps) {
    const p = st.points[st.points.length - 1];
    sum += st.operation_id + p[0] * 1.3 + p[1] * 1.7 + p[2] * 1.9 + st.tool_diameter;
  }
  // A changed cutter (e.g. another shank) with the same moves still counts as new.
  const tools = JSON.stringify(pb.tools);
  return `${pb.steps.length}|${pb.total_s}|${sum}|${tools}`;
}

const samePoint = (a: Point | undefined, b: Point) => a !== undefined && a.x === b.x && a.y === b.y;

/** Until the first view arrives (the engine's defaults). */
const DEFAULT_SNAP: SnapOptions = { grid: true, objects: true };
const DEFAULT_GRID_AREA: GridArea = { x_min: -1000, x_max: 1000, y_min: -1000, y_max: 1000 };

export default function App() {
  const notify = useNotify();
  const [view, setView] = useState<View | null>(null);
  const [selection, setSelection] = useState<Selection>(NOTHING);
  const [fitRequest, setFitRequest] = useState(0);
  const [choice, setChoice] = useState<(Choice & { resolve: (a: string | null) => void }) | null>(null);
  const [currentLayer, setCurrentLayer] = useState<string | null>(null);
  const [viewMode, setViewMode] = useState<ViewMode>("2D");
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  /** The open menu-bar heading, if any. */
  const [barMenu, setBarMenu] = useState<number | null>(null);
  const [numbers, setNumbers] = useState<(NumberQuestion & { resolve: (v: number[] | null) => void }) | null>(
    null,
  );
  const [hasClipboard, setHasClipboard] = useState(false);
  /** Which tab of the left panel is shown (view state only). */
  const [leftTab, setLeftTab] = useState<"project" | "setup">("project");
  const [showToolList, setShowToolList] = useState(false);
  // Toolpath display and tool playback (view state only: not saved, not undo steps).
  const [showToolpaths, setShowToolpaths] = useState(true);
  // Shaded cutter width along the toolpaths (off by default; fetched only while it's on).
  const [showCutWidth, setShowCutWidth] = useState(false);
  const showCutWidthRef = useRef(false);
  const [cutWidths, setCutWidths] = useState<CutWidthStep[]>(EMPTY_CUT_WIDTHS);
  const [showPlaybackBar, setShowPlaybackBar] = useState(true);
  const [playback, setPlayback] = useState<Playback | null>(null);
  const [playTime, setPlayTime] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [speed, setSpeed] = useState(10);
  const playbackSig = useRef("");
  // Last values typed into the transform dialogs, offered again next time.
  const lastValues = useRef({ dx: 0, dy: 0, degrees: 90, factor: 1, tolerance: 0.01, curveTolerance: 0.01 });

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
    // New or changed toolpaths: playback starts again from the beginning.
    const pb = await engine.getPlayback();
    const sig = playbackSignature(pb);
    if (sig !== playbackSig.current) {
      playbackSig.current = sig;
      setPlayback(pb);
      setPlayTime(0);
      setPlaying(false);
      // The cut widths go with the playback steps, so they are fetched again with them.
      setCutWidths(showCutWidthRef.current ? await engine.getCutWidths() : EMPTY_CUT_WIDTHS);
    }
    const existing = new Set(v.display.shapes.map((s) => s.id));
    setSelection((sel) =>
      sel.kind === "objects" ? { kind: "objects", ids: sel.ids.filter((id) => existing.has(id)) } : sel,
    );
    return v;
  }, []);

  useEffect(() => {
    refresh().catch((err) => notify("error", `Could not reach the CAM engine: ${String(err)}`));
    engine.hasClipboard().then(setHasClipboard, () => {});
    engine.settingsProblems().then((ps) => ps.forEach((p) => notify("error", p)), () => {});
  }, [refresh, notify]);

  /**
   * Asks how closely curves (splines, ellipses, Béziers) should become arcs and lines, when the
   * file has any. Resolves the tolerance in mm (remembered), or null if cancelled.
   */
  const askCurveTolerance = useCallback(
    async (count: number, what: string): Promise<number | null> => {
      const last = lastValues.current;
      if (count === 0) return last.curveTolerance;
      const v = await askNumbers({
        title: `Convert ${count} curve${count === 1 ? "" : "s"} (${what}) into arcs and lines`,
        fields: [{ label: "Stay within (mm) of the curve", value: last.curveTolerance }],
      });
      if (!v) return null;
      if (!(v[0] > 0)) {
        notify("error", "The tolerance must be more than 0.");
        return null;
      }
      last.curveTolerance = v[0];
      return v[0];
    },
    [askNumbers, notify],
  );

  /** After an import: show it all, select nothing, and say what came in. */
  const afterImport = useCallback(
    async (message: string) => {
      await refresh();
      setSelection(NOTHING);
      setFitRequest((n) => n + 1);
      notify("info", message);
    },
    [notify, refresh],
  );

  const importDxf = useCallback(
    async (path: string) => {
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
      // Splines and ellipses become arcs and lines: ask how closely, only when there are some.
      const tolerance = await askCurveTolerance(info.curve_count, "splines and ellipses");
      if (tolerance === null) return;
      const report = await engine.importDxf(path, scale, tolerance);
      await afterImport(importMessage(report, scale !== 1 ? info.units.name : null));
    },
    [ask, askCurveTolerance, afterImport],
  );

  const importSvg = useCallback(
    async (path: string) => {
      const info = await engine.svgInfo(path);
      // Files that don't give their real size: ask how big one SVG unit is.
      let mmPerUnit = 1;
      let unitsName: string | null = null;
      if (!info.size_known) {
        const choices: [string, number, string][] = [
          ["96 per inch: px, as Inkscape and browsers (1 unit = 0.2646 mm)", 25.4 / 96, "px (96 per inch)"],
          ["72 per inch: pt, as Illustrator (1 unit = 0.3528 mm)", 25.4 / 72, "pt (72 per inch)"],
          ["1 unit = 1 mm", 1, ""],
        ];
        const answer = await ask({
          title: "SVG units",
          message: "This SVG doesn't say how big it is. How large is one unit in it?",
          options: choices.map((c) => c[0]),
        });
        if (answer === null) return;
        const chosen = choices.find((c) => c[0] === answer)!;
        mmPerUnit = chosen[1];
        unitsName = chosen[2] || null;
      }
      const tolerance = await askCurveTolerance(info.curve_count, "curves and ellipses");
      if (tolerance === null) return;
      // Shapes outside Inkscape layers go on a layer named after the file.
      const name = path.split(/[\\/]/).pop()!.replace(/\.svg$/i, "");
      const report = await engine.importSvg(path, mmPerUnit, tolerance, name);
      await afterImport(importMessage(report, unitsName));
    },
    [ask, askCurveTolerance, afterImport],
  );

  /** Import…: a DXF or SVG drawing, by its file extension. */
  const importDrawing = useCallback(async () => {
    const path = await engine.chooseImportFile();
    if (!path) return;
    if (/\.svg$/i.test(path)) await importSvg(path);
    else await importDxf(path);
  }, [importDxf, importSvg]);

  // --- Project files ---------------------------------------------------------------------------

  /** Save As…: ask where, then save. Resolves false if cancelled. */
  const saveAs = useCallback(async () => {
    const v = await engine.getView();
    const path = await engine.chooseSavePath(v.fileName ?? `${v.document.name}.whamcam`);
    if (!path) return false;
    await engine.saveProject(path);
    const after = await refresh();
    notify("info", `Saved ${after.fileName ?? "the project"}.`);
    return true;
  }, [refresh, notify]);

  /** Save: to the current file, or Save As… if it has never been saved. Resolves false if cancelled. */
  const save = useCallback(async () => {
    const v = await engine.getView();
    if (!v.fileName) return saveAs();
    await engine.saveProject(null);
    await refresh();
    notify("info", `Saved ${v.fileName}.`);
    return true;
  }, [saveAs, refresh, notify]);

  /**
   * With unsaved changes, asks Save / Don't save / Cancel. Resolves true if it's fine to carry
   * on (saved, or the changes may be dropped), false if cancelled.
   */
  const confirmDiscard = useCallback(async () => {
    const v = await engine.getView();
    if (!v.modified) return true;
    const answer = await ask({
      title: "Unsaved changes",
      message: `Save the changes to ${v.fileName ?? v.document.name} first?`,
      options: ["Save", "Don't save"],
    });
    if (answer === null) return false;
    return answer === "Save" ? save() : true;
  }, [ask, save]);

  /** After a different project is loaded: nothing selected, nothing being drawn or placed. */
  const resetForNewDocument = useCallback(() => {
    setDraft(null);
    setSpecial(null);
    setSelection(NOTHING);
    setCurrentLayer(null);
  }, [setDraft, setSpecial]);

  const newProject = useCallback(async () => {
    if (!(await confirmDiscard())) return;
    await engine.newProject();
    resetForNewDocument();
    await refresh();
  }, [confirmDiscard, resetForNewDocument, refresh]);

  const openProject = useCallback(async () => {
    if (!(await confirmDiscard())) return;
    const path = await engine.chooseProjectFile();
    if (!path) return;
    await engine.openProject(path);
    resetForNewDocument();
    await refresh();
    setFitRequest((n) => n + 1);
  }, [confirmDiscard, resetForNewDocument, refresh]);

  // The title bar: file name, with * while there are unsaved changes.
  const title = `WHAM_CAM - ${view?.fileName ?? view?.document.name ?? "Untitled"}${view?.modified ? " *" : ""}`;
  useEffect(() => {
    engine.setWindowTitle(title).catch(() => {});
  }, [title]);

  // Closing the window with unsaved changes asks first.
  const confirmDiscardRef = useRef(confirmDiscard);
  confirmDiscardRef.current = confirmDiscard;
  useEffect(() => {
    const stop = engine.onCloseRequested(() =>
      confirmDiscardRef.current().catch((err) => {
        notify("error", String(err));
        return false;
      }),
    );
    return () => {
      stop.then((f) => f()).catch(() => {});
    };
  }, [notify]);

  // --- Operations ----------------------------------------------------------------------------

  /** Pocket: adds a Pocket of the selected shapes and selects it. */
  const addPocket = useCallback(async () => {
    const shapes = selectedIds(selection);
    if (shapes.length === 0) {
      notify("info", "Select the closed shapes to pocket first (the outline and any islands).");
      return;
    }
    const id = await engine.addPocket(shapes);
    await refresh();
    setSelection({ kind: "operation", id });
    notify("info", "Pocket added. Set its target depth in Properties, then Generate toolpaths.");
  }, [selection, refresh, notify]);

  /** Profile: adds a Profile of the selected shapes and selects it. */
  const addProfile = useCallback(async () => {
    const shapes = selectedIds(selection);
    if (shapes.length === 0) {
      notify("info", "Select the shapes to profile first.");
      return;
    }
    const id = await engine.addProfile(shapes);
    await refresh();
    setSelection({ kind: "operation", id });
    notify("info", "Profile added. Set its target depth in Properties, then Generate toolpaths.");
  }, [selection, refresh, notify]);

  /** Drill: adds a Drill of the selected point lists, circles and arcs and selects it. */
  const addDrill = useCallback(async () => {
    const shapes = selectedIds(selection);
    if (shapes.length === 0) {
      notify("info", "Select the point lists, circles or arcs to drill first.");
      return;
    }
    const id = await engine.addDrill(shapes);
    await refresh();
    setSelection({ kind: "operation", id });
    notify("info", "Drill added. Set its target depth in Properties, then Generate toolpaths.");
  }, [selection, refresh, notify]);

  /** Generate toolpaths: for the given operations, or all enabled ones. */
  const generateToolpaths = useCallback(
    async (ids: number[] = []) => {
      const report = await engine.generateToolpaths(ids);
      await refresh();
      for (const w of report.warnings) notify("warning", w);
      if (report.errors.length > 0) {
        const list = report.errors.map(([name, err]) => `${name}: ${err}`).join(" ");
        notify("error", `${plural(report.errors.length, "operation")} failed. ${list}`);
      } else if (report.generated === 0) {
        notify("info", "Nothing to generate: add an operation (and make sure it's enabled).");
      } else {
        notify("info", `Generated ${plural(report.generated, "toolpath")}.`);
      }
    },
    [refresh, notify],
  );

  /** Writes the job's G-code to a file the user picks (refused with the reason if not ready). */
  const produceGcode = useCallback(async () => {
    const now = new Date();
    const pad = (n: number) => String(n).padStart(2, "0");
    const date = `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())} ${pad(now.getHours())}:${pad(now.getMinutes())}`;
    let suggested: string;
    try {
      suggested = await engine.checkGcode(date);
    } catch (err) {
      notify("error", String(err));
      return;
    }
    const path = await engine.chooseGcodePath(suggested);
    if (!path) return;
    try {
      const name = await engine.saveGcode(path, date);
      notify("info", `Saved ${name}. Check it in a simulator and with an air cut before cutting material.`);
    } catch (err) {
      notify("error", String(err));
    }
  }, [notify]);

  const operationById = useCallback(
    (id: number) => view?.document.parts.flatMap((p) => p.operations).find((o) => o.id === id),
    [view],
  );

  const deleteOperation = useCallback(
    async (id: number) => {
      await engine.deleteOperation(id);
      setSelection((sel) => (sel.kind === "operation" && sel.id === id ? NOTHING : sel));
      await refresh();
    },
    [refresh],
  );

  /** The right-click menu of an operation in the tree. */
  const operationMenu = (id: number): MenuItem[] => {
    const op = operationById(id);
    if (!op) return [];
    const shapes = selectedIds(selection);
    return [
      {
        label: "Use selected shapes",
        disabled: shapes.length === 0,
        action: () =>
          run(async () => {
            await engine.setOperationShapes(id, shapes);
            await refresh();
            notify("info", `${op.name} now uses ${plural(shapes.length, "shape")}.`);
          }),
      },
      { label: "Select its shapes", action: () => setSelection({ kind: "objects", ids: op.shape_ids }) },
      {
        label: op.enabled ? "Disable" : "Enable",
        separatorBefore: true,
        action: () => void setOperation({ ...op, enabled: !op.enabled }),
      },
      { label: "Generate toolpath", action: () => run(() => generateToolpaths([id])) },
      { label: "Delete", separatorBefore: true, action: () => run(() => deleteOperation(id)) },
    ];
  };

  /** Deletes the selected objects, or the selected layer (with everything on it). */
  const deleteSelected = useCallback(async () => {
    if (selection.kind === "operation") {
      await deleteOperation(selection.id);
      return;
    }
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
  }, [selection, view, refresh, deleteOperation]);

  const hiddenLayers = useMemo(() => new Set(view?.hiddenLayers ?? []), [view]);

  /** Shows or hides a layer, then redraws. */
  const setLayerVisible = useCallback(
    async (name: string, visible: boolean) => {
      await engine.setLayerVisible(name, visible);
      await refresh();
    },
    [refresh],
  );

  /** Adds a new empty layer, selects it and makes it the current layer. */
  const addLayer = useCallback(async () => {
    const name = await engine.addLayer();
    await refresh();
    const doc = await engine.getView();
    const index = doc.document.layers.findIndex((l) => l.name === name);
    setSelection({ kind: "layer", layer: index });
    setCurrentLayer(name);
    return name;
  }, [refresh]);

  /** Deletes a layer (with everything on it). */
  const deleteLayer = useCallback(
    async (name: string) => {
      await engine.deleteLayer(name);
      setSelection(NOTHING);
      await refresh();
    },
    [refresh],
  );

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
  const setOperation = useCallback((op: Operation) => tryEdit(() => engine.setOperation(op)), [tryEdit]);
  const setOperationShapes = useCallback(
    (id: number, shapeIds: number[]) => tryEdit(() => engine.setOperationShapes(id, shapeIds)),
    [tryEdit],
  );

  // --- Tooling (this computer's tool list and libraries; saved straight away) ---------------------

  /** Runs a tooling change; resolves to an error message, or null if it worked. */
  const toolingEdit = useCallback(
    async (change: () => Promise<unknown>): Promise<string | null> => {
      try {
        await change();
        await refresh();
        return null;
      } catch (err) {
        return String(err);
      }
    },
    [refresh],
  );
  const toolingTry = useCallback(
    async (change: () => Promise<unknown>) => {
      const error = await toolingEdit(change);
      if (error) notify("error", error);
      return error === null;
    },
    [toolingEdit, notify],
  );

  const deleteTool = useCallback(
    async (tool: Tool) => {
      const libs = (view?.tooling.libraries ?? []).filter((l) => l.tools.some((t) => t.tool_id === tool.id));
      const where =
        libs.length === 0
          ? ""
          : ` It will also be taken out of ${libs.length === 1 ? "the library" : "the libraries"} ${libs
              .map((l) => `"${l.name}"`)
              .join(", ")}.`;
      const answer = await ask({
        title: "Delete tool",
        message: `Delete "${tool.name}" from the master tool list?${where} Operations that use it keep their tool values. This can't be undone.`,
        options: ["Delete"],
      });
      if (answer === "Delete") await toolingTry(() => engine.deleteTool(tool.id));
    },
    [view, ask, toolingTry],
  );

  const renameLibrary = useCallback(
    async (oldName: string, newName: string) => {
      let stored = newName.trim();
      const ok = await toolingTry(async () => {
        stored = await engine.renameLibrary(oldName, newName);
      });
      if (ok) setSelection((sel) => (sel.kind === "library" && sel.name === oldName ? { kind: "library", name: stored } : sel));
      return ok;
    },
    [toolingTry],
  );

  const deleteLibrary = useCallback(
    async (name: string) => {
      const answer = await ask({
        title: "Delete tool library",
        message: `Delete the tool library "${name}"? Its tools stay in the master tool list, and operations keep their tool values. This can't be undone.`,
        options: ["Delete"],
      });
      if (answer !== "Delete") return;
      if (await toolingTry(() => engine.deleteLibrary(name)))
        setSelection((sel) => (sel.kind === "library" && sel.name === name ? NOTHING : sel));
    },
    [ask, toolingTry],
  );

  const renameMachine = useCallback(
    async (oldName: string, newName: string) => {
      let stored = newName.trim();
      const ok = await toolingTry(async () => {
        stored = await engine.renameMachine(oldName, newName);
      });
      if (ok) setSelection((sel) => (sel.kind === "machine" && sel.name === oldName ? { kind: "machine", name: stored } : sel));
      return ok;
    },
    [toolingTry],
  );

  const renameTemplate = useCallback(
    async (oldName: string, newName: string) => {
      let stored = newName.trim();
      const ok = await toolingTry(async () => {
        stored = await engine.renameTemplate(oldName, newName);
      });
      if (ok)
        setSelection((sel) => (sel.kind === "template" && sel.name === oldName ? { kind: "template", name: stored } : sel));
      return ok;
    },
    [toolingTry],
  );

  const deleteTemplate = useCallback(
    async (name: string) => {
      const answer = await ask({
        title: "Delete project template",
        message: `Delete the project template "${name}"? Projects that used it keep their own grid and snap settings. This can't be undone.`,
        options: ["Delete"],
      });
      if (answer !== "Delete") return;
      if (await toolingTry(() => engine.deleteTemplate(name)))
        setSelection((sel) => (sel.kind === "template" && sel.name === name ? NOTHING : sel));
    },
    [ask, toolingTry],
  );

  const deleteMachine = useCallback(
    async (name: string) => {
      const answer = await ask({
        title: "Delete machine",
        message: `Delete the machine "${name}"? Projects that use it will show it as not on this computer. This can't be undone.`,
        options: ["Delete"],
      });
      if (answer !== "Delete") return;
      if (await toolingTry(() => engine.deleteMachine(name)))
        setSelection((sel) => (sel.kind === "machine" && sel.name === name ? NOTHING : sel));
    },
    [ask, toolingTry],
  );

  const addLibrary = useCallback(async () => {
    const name = await engine.addLibrary();
    await refresh();
    return name;
  }, [refresh]);

  const setActiveLibrary = useCallback(
    (name: string | null) => tryEdit(() => engine.setActiveLibrary(name)),
    [tryEdit],
  );
  const pickTool = useCallback(
    (id: number, tool: engine.OperationTool) => tryEdit(() => engine.setOperationTool(id, tool)),
    [tryEdit],
  );

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
    if (!d || CLICKS_NEEDED[d.tool] !== null || d.tool === "Dimension") return;
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

  // --- Edit tools -----------------------------------------------------------------------------

  /** Join…: ask for the tolerance, then join the selected lines, arcs and open polylines. */
  const joinSelected = useCallback(async () => {
    const targets = ids;
    if (targets.length === 0) return;
    const last = lastValues.current;
    const v = await askNumbers({
      title: `Join ${plural(targets.length, "object")}`,
      fields: [{ label: "Join ends closer than (mm)", value: last.tolerance }],
    });
    if (!v) return;
    last.tolerance = v[0];
    const r = await engine.join(targets, v[0]);
    await refresh();
    if (r.ids.length === 0) {
      notify("info", "Nothing to join: no line, arc or open polyline ends meet within that distance.");
      return;
    }
    setSelection({ kind: "objects", ids: r.ids });
    const open = r.ids.length - r.closed;
    const made = [r.closed > 0 ? plural(r.closed, "closed polyline") : "", open > 0 ? plural(open, "open polyline") : ""]
      .filter(Boolean)
      .join(" and ");
    const left = r.skipped > 0 ? ` ${plural(r.skipped, "shape")} couldn't be joined.` : "";
    notify("info", `Joined ${plural(r.joined, "shape")} into ${made}.${left}`);
  }, [ids, askNumbers, refresh, notify]);

  const explodeSelected = useCallback(async () => {
    if (ids.length === 0) return;
    const pieces = await engine.explode(ids);
    if (pieces.length === 0) {
      notify("info", "Nothing to explode: only polylines and rectangles can be exploded.");
      return;
    }
    await refresh();
    setSelection({ kind: "objects", ids: pieces });
    notify("info", `Exploded into ${plural(pieces.length, "piece")}.`);
  }, [ids, refresh, notify]);

  const closeSelected = useCallback(async () => {
    if (ids.length === 0) return;
    const n = await engine.closePolylines(ids);
    if (n === 0) {
      notify("info", "Nothing to close: select open polylines with at least 3 points.");
      return;
    }
    await refresh();
    notify("info", `Closed ${plural(n, "polyline")}.`);
  }, [ids, refresh, notify]);

  const breakSelected = useCallback(async () => {
    if (ids.length < 2) {
      notify("info", "Select at least 2 shapes that cross to break them.");
      return;
    }
    const r = await engine.breakAtIntersections(ids);
    if (r.ids.length === 0) {
      notify("info", "Nothing to break: the selected shapes don't cross or touch.");
      return;
    }
    await refresh();
    setSelection({ kind: "objects", ids: r.ids });
    notify(
      "info",
      `Broke ${plural(r.broken, "shape")} into ${plural(r.ids.length, "piece")} at ${plural(r.crossings, "crossing")}.`,
    );
  }, [ids, refresh, notify]);

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
    (p: Point, click: engine.DimensionClick | null) => {
      const d = draftRef.current;
      if (d?.tool === "Dimension" && click) {
        // The engine said what this click is (a point, or a picked circle/arc/line) and whether
        // it completes the dimension.
        if (click.problem) {
          notify("info", click.problem);
          return;
        }
        const next: Draft = { ...d, points: [...d.points, click.point], targets: [...d.targets, click.target] };
        if (click.finishes) void commitDraft(next);
        else setDraft(next);
        return;
      }
      if (d) return addDrawPoint(p);
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
    [addDrawPoint, applyTransform, commitDraft, notify, refresh, run, setDraft, setSpecial],
  );

  const toggleCutWidth = () => {
    const on = !showCutWidthRef.current;
    showCutWidthRef.current = on;
    setShowCutWidth(on);
    if (on) engine.getCutWidths().then(setCutWidths, (err) => notify("error", String(err)));
    else setCutWidths(EMPTY_CUT_WIDTHS);
  };

  const toggleToolpaths = () => {
    setPlaying(false);
    setShowToolpaths((v) => !v);
  };

  const onAction = (name: string) => {
    const tool = TOOL_BUTTONS[name];
    if (tool) {
      // Pressing the active tool again stops drawing.
      if (draftRef.current?.tool === tool) stopPlacing();
      else setDraft(emptyDraft(tool));
      return;
    }
    switch (name) {
      case "New":
        return run(newProject);
      case "Open":
        return run(openProject);
      case "Save":
        return run(save);
      case "Import":
        return run(importDrawing);
      case "Undo":
        return run(() => undoRedo("undo"));
      case "Redo":
        return run(() => undoRedo("redo"));
      case "Delete":
        return run(deleteSelected);
      case "Zoom to fit":
        return setFitRequest((n) => n + 1);
      case "Profile":
        return run(addProfile);
      case "Pocket":
        return run(addPocket);
      case "Drill":
        return run(addDrill);
      case "Generate toolpaths":
        return run(() => generateToolpaths());
      case "Show toolpaths":
        return toggleToolpaths();
      case "Show cut width":
        return toggleCutWidth();
      case "Produce G-code":
        return void produceGcode();
      default:
        notify("info", `${name} is not built yet.`);
    }
  };

  /**
   * Up/Down arrows: select the previous/next shape in tree order (layer by layer), stopping at
   * the ends. With nothing selected Down starts at the first shape and Up at the last; with a
   * layer selected, at that layer's first or last shape.
   */
  const stepSelection = (dir: 1 | -1) => {
    const order = layers.flatMap((l) => l.objects.map((o) => o.id));
    if (order.length === 0) return;
    const at = ids.map((id) => order.indexOf(id)).filter((i) => i >= 0);
    let next: number;
    if (at.length > 0) {
      next = dir > 0 ? Math.max(...at) + 1 : Math.min(...at) - 1;
    } else if (selection.kind === "layer" && layers[selection.layer]?.objects.length) {
      const objs = layers[selection.layer].objects;
      next = order.indexOf(objs[dir > 0 ? 0 : objs.length - 1].id);
    } else {
      next = dir > 0 ? 0 : order.length - 1;
    }
    next = Math.min(Math.max(next, 0), order.length - 1);
    setSelection({ kind: "objects", ids: [order[next]] });
  };

  // Keyboard shortcuts (ignored while typing in a text box or while a dialog is open).
  const keyHandler = useRef<(e: KeyboardEvent) => void>(() => {});
  keyHandler.current = (e: KeyboardEvent) => {
    if (choice || numbers) return;
    const key = e.key.toLowerCase();
    if (menu || barMenu !== null) {
      if (key === "escape") {
        setMenu(null);
        setBarMenu(null);
        e.preventDefault();
      }
      return;
    }
    const target = e.target as HTMLElement | null;
    if (target && ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName)) return;
    // Keys pressed inside a pop-up (e.g. the points table) belong to the pop-up.
    if (target?.closest?.(".modal-backdrop")) return;
    const ctrl = e.ctrlKey || e.metaKey;
    const d = draftRef.current;
    if (key === "escape") {
      // Esc: cancel a move/paste or the shape in progress, then stop drawing, then clear the selection.
      if (specialRef.current) setSpecial(null);
      else if (d && d.points.length > 0) setDraft(emptyDraft(d.tool));
      else if (d) setDraft(null);
      else setSelection(NOTHING);
    } else if (ctrl && key === "n") run(newProject);
    else if (ctrl && key === "o") run(openProject);
    else if (ctrl && e.shiftKey && key === "s") run(saveAs);
    else if (ctrl && key === "s") run(save);
    else if (ctrl && e.shiftKey && key === "a") run(() => selectAllOnLayer(drawLayer));
    else if (ctrl && key === "a") run(selectAll);
    else if (ctrl && key === "c") run(copySelected);
    else if (ctrl && key === "x") run(cutSelected);
    else if (ctrl && key === "v") startPaste();
    else if (d && key === "enter") finishDraft();
    else if (d && key === "backspace") {
      const points = d.points.slice(0, -1);
      setDraft({ ...d, points, arcs: d.arcs.slice(0, Math.max(0, points.length - 1)) });
    } else if (d?.tool === "Polyline" && d.points.length > 0 && !ctrl && !e.altKey && (key === "a" || key === "l")) {
      // While a polyline is being drawn, A and L switch its next segment between arc and line.
      setDraft({ ...d, arc_next: key === "a" });
    } else if (!ctrl && !e.altKey && TOOL_KEYS[key]) {
      // Drawing tool shortcuts (L, A, C, R, P, full stop for points). The tool already in use carries on as it is.
      if (d?.tool !== TOOL_KEYS[key]) setDraft(emptyDraft(TOOL_KEYS[key]));
    } else if (!ctrl && (key === "arrowdown" || key === "arrowup")) stepSelection(key === "arrowdown" ? 1 : -1);
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

  const selectedSet = useMemo(() => new Set(ids), [ids]);
  const operationNames = useMemo(
    () => new Map((view?.document.parts ?? []).flatMap((p) => p.operations.map((o) => [o.id, o.name] as const))),
    [view],
  );
  const onSelect = (sel: Selection) => {
    setSelection(sel);
    if (sel.kind === "layer" && layers[sel.layer]) {
      const name = layers[sel.layer].name;
      setCurrentLayer(name);
      // The current layer can't be hidden: making a hidden layer current shows it.
      if (hiddenLayers.has(name)) run(() => setLayerVisible(name, true));
    }
  };

  const disabled = new Set<string>();
  if (!view?.canUndo) disabled.add("Undo");
  if (!showToolpaths) disabled.add("Show cut width");
  if (!view?.canRedo) disabled.add("Redo");
  if (ids.length === 0 && selection.kind !== "layer" && selection.kind !== "operation") disabled.add("Delete");

  const placing = useMemo<Placing | null>(
    () => (draft ? { kind: "draw", draft } : special),
    [draft, special],
  );

  // The commands shared by the menu bar and the right-click menu. Items that can't be used right
  // now are greyed; ones not built yet are always greyed.
  const commands = () => {
    const none = ids.length === 0;
    const later = (label: string): MenuItem => ({ label, disabled: true });
    const sep = (item: MenuItem): MenuItem => ({ ...item, separatorBefore: true });
    const draw = (label: string): MenuItem => ({ label, shortcut: TOOL_KEY_LABELS[label], action: () => onAction(label) });
    return {
      later,
      sep,
      view: [
        { label: "Zoom to fit", action: () => setFitRequest((n) => n + 1) },
        { label: "2D view", disabled: viewMode === "2D", action: () => setViewMode("2D") },
        { label: "3D view", disabled: viewMode === "3D", action: () => setViewMode("3D") },
        { label: "Show toolpaths", checked: showToolpaths, action: toggleToolpaths, separatorBefore: true },
        { label: "Show cut width", checked: showCutWidth, disabled: !showToolpaths, action: toggleCutWidth },
        {
          label: "Toolpath playback",
          checked: showPlaybackBar,
          disabled: !showToolpaths,
          action: () => {
            setPlaying(false);
            setShowPlaybackBar((v) => !v);
          },
        },
      ] as MenuItem[],
      draw: [
        ...["Line", "Arc", "Circle", "Rectangle", "Polyline", "Point"].map(draw),
        sep(draw("Dimension")),
      ],
      transform: {
        label: "Transform",
        disabled: none,
        items: [
          { label: "Move…", action: () => run(() => transformDialog("Move")) },
          { label: "Move by clicking", action: startMoveByClicking },
          { label: "Rotate…", action: () => run(() => transformDialog("Rotate")) },
          { label: "Rotate by clicking", action: () => run(startRotateByClicking) },
          { label: "Scale…", action: () => run(() => transformDialog("Scale")) },
          sep({ label: "Mirror left–right", action: () => run(() => applyTransform(ids, { type: "MirrorLeftRight" })) }),
          { label: "Mirror top–bottom", action: () => run(() => applyTransform(ids, { type: "MirrorTopBottom" })) },
        ],
      } as MenuItem,
      del: {
        label: "Delete",
        shortcut: "Del",
        disabled: disabled.has("Delete"),
        action: () => run(deleteSelected),
      } as MenuItem,
      editTools: [
        sep({ label: "Join…", disabled: none, action: () => run(joinSelected) }),
        { label: "Explode", disabled: none, action: () => run(explodeSelected) },
        { label: "Break at intersections", disabled: ids.length < 2, action: () => run(breakSelected) },
        { label: "Close polyline", disabled: none, action: () => run(closeSelected) },
      ] as MenuItem[],
      select: [
        { label: "Select All", shortcut: "Ctrl+A", action: () => run(selectAll) },
        {
          label: `Select All On Layer "${drawLayer}"`,
          shortcut: "Ctrl+Shift+A",
          action: () => run(() => selectAllOnLayer(drawLayer)),
        },
      ] as MenuItem[],
      clipboard: [
        { label: "Cut", shortcut: "Ctrl+X", disabled: none, action: () => run(cutSelected) },
        { label: "Copy", shortcut: "Ctrl+C", disabled: none, action: () => run(copySelected) },
        { label: "Paste", shortcut: "Ctrl+V", disabled: !hasClipboard, action: startPaste },
      ] as MenuItem[],
      undo: [
        { label: "Undo", shortcut: "Ctrl+Z", disabled: !view?.canUndo, action: () => run(() => undoRedo("undo")) },
        { label: "Redo", shortcut: "Ctrl+Y", disabled: !view?.canRedo, action: () => run(() => undoRedo("redo")) },
      ] as MenuItem[],
    };
  };

  // The right-click menu in the drawing area.
  const menuItems = (): MenuItem[] => {
    const c = commands();
    const [first, ...restSelect] = c.select;
    const [cut, ...restClipboard] = c.clipboard;
    const [undo, ...restUndo] = c.undo;
    return [
      { label: "View", items: c.view },
      { label: "Draw", items: c.draw },
      c.transform,
      { label: "Edit", items: [c.del, ...c.editTools] },
      c.sep(first),
      ...restSelect,
      c.sep(cut),
      ...restClipboard,
      c.sep(undo),
      ...restUndo,
    ];
  };

  // The menu bar across the top.
  const menuBarItems = (): MenuItem[] => {
    const c = commands();
    const [first, ...restSelect] = c.select;
    const [cut, ...restClipboard] = c.clipboard;
    return [
      {
        label: "File",
        items: [
          { label: "New", shortcut: "Ctrl+N", action: () => run(newProject) },
          { label: "Open…", shortcut: "Ctrl+O", action: () => run(openProject) },
          { label: "Save", shortcut: "Ctrl+S", action: () => run(save) },
          { label: "Save As…", shortcut: "Ctrl+Shift+S", action: () => run(saveAs) },
          c.sep({ label: "Import DXF or SVG…", action: () => run(importDrawing) }),
        ],
      },
      { label: "View", items: c.view },
      {
        label: "Edit",
        items: [
          ...c.undo,
          c.sep(cut),
          ...restClipboard,
          c.del,
          c.sep(first),
          ...restSelect,
          c.sep(c.transform),
          ...c.editTools,
        ],
      },
      { label: "Draw", items: c.draw },
      {
        label: "Machining",
        items: [
          { label: "Profile", action: () => run(addProfile) },
          { label: "Pocket", action: () => run(addPocket) },
          { label: "Drill", action: () => run(addDrill) },
          c.sep({ label: "Master tool list…", action: () => setShowToolList(true) }),
          {
            label: "Tool library",
            items:
              (view?.tooling.libraries.length ?? 0) === 0
                ? [{ label: "No libraries yet (Setup tab)", disabled: true }]
                : [
                    {
                      label: "None",
                      checked: view?.document.active_library === null,
                      action: () => void setActiveLibrary(null),
                    },
                    ...(view?.tooling.libraries ?? []).map((l) => ({
                      label: l.name,
                      checked: view?.document.active_library === l.name,
                      action: () => void setActiveLibrary(l.name),
                    })),
                  ],
          },
          c.sep({ label: "Generate toolpaths", action: () => run(() => generateToolpaths()) }),
          { label: "Produce G-code", action: () => void produceGcode() },
        ],
      },
    ];
  };

  // The playback, when toolpaths and the playback bar are shown and there is something to play.
  const toolbarOn = useMemo(() => {
    const on = new Set<string>();
    if (showToolpaths) on.add("Show toolpaths");
    if (showCutWidth) on.add("Show cut width");
    return on;
  }, [showToolpaths, showCutWidth]);

  const playbackShown = showToolpaths && showPlaybackBar && playback && playback.steps.length > 0 ? playback : null;

  return (
    <div className="app">
      <MenuBar menus={menuBarItems()} open={barMenu} onOpen={setBarMenu} />
      <Toolbar
        onAction={onAction}
        disabled={disabled}
        active={draft ? buttonForTool(draft.tool) : null}
        on={toolbarOn}
      />
      <div className="side">
        <section className="panel tree-panel">
          <div className="panel-tabs" role="tablist">
            <button
              type="button"
              role="tab"
              aria-selected={leftTab === "project"}
              className={leftTab === "project" ? "active" : ""}
              onClick={() => setLeftTab("project")}
            >
              Project
            </button>
            <button
              type="button"
              role="tab"
              aria-selected={leftTab === "setup"}
              className={leftTab === "setup" ? "active" : ""}
              onClick={() => setLeftTab("setup")}
            >
              Setup
            </button>
          </div>
          {leftTab === "setup" ? (
            <SetupTree
              tooling={view?.tooling ?? null}
              selection={selection}
              onSelect={onSelect}
              onOpenToolList={() => setShowToolList(true)}
              onAddLibrary={addLibrary}
              onRenameLibrary={renameLibrary}
              onDeleteLibrary={(name) => void deleteLibrary(name)}
              machines={view?.machines ?? null}
              onAddMachine={async () => {
                const name = await engine.addMachine();
                await refresh();
                return name;
              }}
              onRenameMachine={renameMachine}
              onDeleteMachine={(name) => void deleteMachine(name)}
              templates={view?.templates ?? null}
              onAddTemplate={async () => {
                const name = await engine.addTemplate();
                await refresh();
                return name;
              }}
              onRenameTemplate={renameTemplate}
              onDeleteTemplate={(name) => void deleteTemplate(name)}
              onSetTemplateForNewProjects={(name) => void toolingTry(() => engine.setTemplateForNewProjects(name))}
            />
          ) : (
          <Tree
            doc={view?.document ?? null}
            selection={selection}
            onSelect={onSelect}
            onRenameLayer={renameLayer}
            onSetLayerColor={setLayerColor}
            hiddenLayers={hiddenLayers}
            onSetLayerVisible={(name, visible) => run(() => setLayerVisible(name, visible))}
            dimensions={view?.dimensions ?? []}
            dimensionsVisible={view?.dimensionsVisible ?? true}
            onSetDimensionsVisible={(visible) => run(async () => {
              await engine.setDimensionsVisible(visible);
              await refresh();
            })}
            currentLayer={drawLayer}
            onAddLayer={addLayer}
            operationStates={view?.operations ?? []}
            operationMenu={operationMenu}
            onRenameOperation={async (id, name) => {
              const op = operationById(id);
              return op ? setOperation({ ...op, name }) : false;
            }}
            onDeleteLayer={(name) => run(() => deleteLayer(name))}
          />
          )}
        </section>
        <section className="panel props-panel">
          <h2>Properties</h2>
          <Properties
            doc={view?.document ?? null}
            display={view?.display ?? null}
            selection={selection}
            dimensions={view?.dimensions ?? []}
            onRenameLayer={renameLayer}
            onSetLayerColor={setLayerColor}
            onSetShape={setShape}
            onSetGridSpacing={setGridSpacing}
            onSetGridArea={(area) => tryEdit(() => engine.setGridArea(area))}
            onSetSnap={(snap) => tryEdit(() => engine.setSnap(snap))}
            templates={view?.templates ?? null}
            onApplyTemplate={(name) => tryEdit(() => engine.applyProjectTemplate(name))}
            onRenameTemplate={renameTemplate}
            onSetTemplateGridSpacing={(name, mm) => toolingTry(() => engine.setTemplateGridSpacing(name, mm))}
            onSetTemplateGridArea={(name, area) => toolingTry(() => engine.setTemplateGridArea(name, area))}
            onSetTemplateSnap={(name, snap) => toolingTry(() => engine.setTemplateSnap(name, snap))}
            onSetOperation={setOperation}
            onSetOperationShapes={setOperationShapes}
            operationStates={view?.operations ?? []}
            tooling={view?.tooling ?? EMPTY_TOOLING}
            onPickTool={pickTool}
            onSetMachiningTool={(n) => tryEdit(() => engine.setMachiningTool(n))}
            onSetPartTool={(part, n) => tryEdit(() => engine.setPartTool(part, n))}
            machines={view?.machines ?? EMPTY_MACHINES}
            onRenameMachine={renameMachine}
            onSetMachinePost={(name, post) => toolingTry(() => engine.setMachinePost(name, post))}
            onSetProjectMachine={(name) => tryEdit(() => engine.setProjectMachine(name))}
            onSetActiveLibrary={setActiveLibrary}
            onRenameLibrary={renameLibrary}
            onAddLibraryTool={(library, toolId, number) =>
              toolingTry(() => engine.addLibraryTool(library, toolId, number))
            }
            onSetLibraryToolNumber={(library, oldNumber, newNumber) =>
              toolingTry(() => engine.setLibraryToolNumber(library, oldNumber, newNumber))
            }
            onRemoveLibraryTool={(library, number) => toolingTry(() => engine.removeLibraryTool(library, number))}
          />
        </section>
      </div>
      <Viewport
        display={view?.display ?? null}
        toolpaths={showToolpaths ? (view?.toolpaths ?? EMPTY_TOOLPATHS) : EMPTY_TOOLPATHS}
        cutWidths={showToolpaths && showCutWidth ? cutWidths : EMPTY_CUT_WIDTHS}
        playback={playbackShown}
        playTime={playTime}
        selectedIds={selectedSet}
        fitRequest={fitRequest}
        gridSpacing={view?.document.grid_spacing ?? 10}
        gridArea={view?.document.grid_area ?? DEFAULT_GRID_AREA}
        mode={viewMode}
        onMode={setViewMode}
        placing={placing}
        hint={draft ? drawHint(draft) : special ? specialHint(special) : null}
        snapOptions={view?.document.snap ?? DEFAULT_SNAP}
        onSnapOptions={(snap) => void tryEdit(() => engine.setSnap(snap))}
        onPick={onPick}
        onBox={onBox}
        onPlacePoint={placePoint}
        onScale={() => run(refresh)}
        onRepositionDimension={(id, p) => run(async () => {
          await engine.repositionDimension(id, p.x, p.y);
          await refresh();
          setSelection({ kind: "objects", ids: [id] });
        })}
        drawColor={layers.find((l) => l.name === drawLayer)?.color ?? "#ffffff"}
        onFinishDraft={finishDraft}
        onContextMenu={(x, y) => {
          setBarMenu(null);
          setMenu({ x, y });
        }}
      >
        {playbackShown && (
          <PlaybackBar
            playback={playbackShown}
            time={playTime}
            onTime={setPlayTime}
            playing={playing}
            onPlaying={setPlaying}
            speed={speed}
            onSpeed={setSpeed}
            names={operationNames}
          />
        )}
      </Viewport>
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
      {showToolList && view && (
        <ToolListDialog
          tooling={view.tooling}
          newTool={engine.newTool}
          preview={engine.toolPreview}
          onAdd={(tool) => toolingEdit(() => engine.addTool(tool))}
          onEdit={(tool) => toolingEdit(() => engine.setTool(tool))}
          onDelete={(tool) => void deleteTool(tool)}
          onClose={() => setShowToolList(false)}
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
