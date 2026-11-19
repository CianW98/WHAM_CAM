// The properties panel: the settings of whatever is selected, editable in place.
// Values only change through the engine.

import { useEffect, useRef, useState, type ReactNode } from "react";
import type {
  CutOrdering,
  DisplayData,
  DrillMethod,
  DrillSettings,
  Document,
  EndType,
  FluteType,
  Layer,
  MillingDirection,
  Operation,
  OperationKind,
  OperationState,
  PocketSettings,
  Point,
  ProfileSettings,
  ProfileSide,
  Shape,
  OperationTool,
  PickedTool,
  Tool,
  Tooling,
} from "../engine";
import type { Selection } from "../selection";
import { shapeLabel } from "./Tree";
import ColorSwatch from "./ColorSwatch";
import PointsDialog, { type PointsEdit } from "./PointsDialog";
import { END_TYPES, FLUTE_TYPES, endTypeLabel } from "./ToolListDialog";

/** Numbers as the engine sent them, rounded for display with trailing zeros trimmed. */
const num = (n: number) => String(Number(n.toFixed(4)));
const pointText = (p: Point) => `${num(p.x)},${num(p.y)}`;

/** Reads "x,y" (a comma and/or spaces between). Null if not two numbers. */
function parsePoint(text: string): Point | null {
  const parts = text.trim().split(/\s*[,;\s]\s*/);
  if (parts.length !== 2 || parts.some((p) => p === "")) return null;
  const [x, y] = parts.map(Number);
  return Number.isFinite(x) && Number.isFinite(y) ? { x, y } : null;
}

/** Sends a changed shape to the engine. Resolves false if the engine refused it. */
type EditShape = (shape: Shape) => Promise<boolean>;

// --- Grid building blocks ---------------------------------------------------------------------

function Grid({ children }: { children: ReactNode }) {
  return (
    <table className="props">
      <tbody>{children}</tbody>
    </table>
  );
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <tr>
      <th>{label}</th>
      <td>{children}</td>
    </tr>
  );
}

/**
 * A text box in the grid. Enter or clicking away saves, Esc cancels. Unchanged text is not
 * saved, so a value shown rounded is never overwritten by its rounded form. `parse` returns
 * null for text that isn't valid.
 */
function TextField<T>({
  shown,
  parse,
  onSave,
}: {
  shown: string;
  parse: (text: string) => T | null;
  onSave: (v: T) => Promise<boolean>;
}) {
  const [text, setText] = useState(shown);
  const cancelled = useRef(false);
  useEffect(() => setText(shown), [shown]);

  const save = async () => {
    if (cancelled.current) {
      cancelled.current = false;
      return;
    }
    if (text.trim() === shown) return;
    const v = parse(text);
    if (v === null || !(await onSave(v))) setText(shown);
  };

  return (
    <input
      className="prop-input"
      value={text}
      spellCheck={false}
      onChange={(e) => setText(e.target.value)}
      onBlur={save}
      onKeyDown={(e) => {
        if (e.key === "Enter") e.currentTarget.blur();
        if (e.key === "Escape") {
          cancelled.current = true;
          setText(shown);
          e.currentTarget.blur();
        }
      }}
    />
  );
}

function NumberField({ value, onSave }: { value: number; onSave: (v: number) => Promise<boolean> }) {
  return (
    <TextField
      shown={num(value)}
      parse={(t) => (t.trim() !== "" && Number.isFinite(Number(t)) ? Number(t) : null)}
      onSave={onSave}
    />
  );
}

/** A point as one "x,y" box. */
function PointField({ value, onSave }: { value: Point; onSave: (p: Point) => Promise<boolean> }) {
  return <TextField shown={pointText(value)} parse={parsePoint} onSave={onSave} />;
}

function BoolField({ value, onSave }: { value: boolean; onSave: (v: boolean) => void }) {
  return <input type="checkbox" checked={value} onChange={(e) => onSave(e.target.checked)} />;
}

/** A drop-down of fixed choices (value → label). */
function ChoiceField<T extends string>({
  value,
  choices,
  onSave,
}: {
  value: T;
  choices: [T, string][];
  onSave: (v: T) => void;
}) {
  return (
    <select className="prop-input" value={value} onChange={(e) => onSave(e.target.value as T)}>
      {choices.map(([v, label]) => (
        <option key={v} value={v}>
          {label}
        </option>
      ))}
    </select>
  );
}

/** A heading row splitting the grid into groups. */
function GroupRow({ label }: { label: string }) {
  return (
    <tr className="props-group">
      <th colSpan={2}>{label}</th>
    </tr>
  );
}

/** Reads a list of shape ids like "1, 2, 3" (commas and/or spaces). Null if not whole numbers. */
function parseIds(text: string): number[] | null {
  const parts = text.split(/[\s,;]+/).filter((p) => p !== "");
  if (parts.length === 0 || !parts.every((p) => /^\d+$/.test(p))) return null;
  return [...new Set(parts.map(Number))];
}

const parseNumber = (t: string) => (t.trim() !== "" && Number.isFinite(Number(t)) ? Number(t) : null);

/** "(Collection)" with a … button that opens the points pop-up. */
function CollectionField({ onOpen }: { onOpen: () => void }) {
  return (
    <span className="collection">
      <span className="muted">(Collection)</span>
      <button type="button" title="Edit the points" onClick={onOpen}>
        …
      </button>
    </span>
  );
}

// --- Shapes ---------------------------------------------------------------------------------

/** The rows for a shape's own values; editable except for splines and ellipses. */
function ShapeRows({
  shape,
  direction,
  edit,
  openPoints,
}: {
  shape: Shape;
  direction: string | null;
  edit: EditShape;
  openPoints: (e: PointsEdit, save: (pts: { x: number; y: number; bulge: number }[]) => Promise<boolean>) => void;
}) {
  switch (shape.type) {
    case "Line":
      return (
        <>
          <Row label="Start">
            <PointField value={shape.start} onSave={(start) => edit({ ...shape, start })} />
          </Row>
          <Row label="End">
            <PointField value={shape.end} onSave={(end) => edit({ ...shape, end })} />
          </Row>
        </>
      );
    case "Arc":
      return (
        <>
          <Row label="Center">
            <PointField value={shape.center} onSave={(center) => edit({ ...shape, center })} />
          </Row>
          <Row label="Radius">
            <NumberField value={shape.radius} onSave={(radius) => edit({ ...shape, radius })} />
          </Row>
          <Row label="Start angle">
            <NumberField value={shape.start_angle} onSave={(start_angle) => edit({ ...shape, start_angle })} />
          </Row>
          <Row label="End angle">
            <NumberField value={shape.end_angle} onSave={(end_angle) => edit({ ...shape, end_angle })} />
          </Row>
        </>
      );
    case "Circle":
      return (
        <>
          <Row label="Center">
            <PointField value={shape.center} onSave={(center) => edit({ ...shape, center })} />
          </Row>
          <Row label="Radius">
            <NumberField value={shape.radius} onSave={(radius) => edit({ ...shape, radius })} />
          </Row>
        </>
      );
    case "Polyline": {
      const segments = shape.closed ? shape.vertices.length : shape.vertices.length - 1;
      return (
        <>
          <Row label="Closed">
            <BoolField value={shape.closed} onSave={(closed) => void edit({ ...shape, closed })} />
          </Row>
          <Row label="Direction">
            {direction ?? ""}
          </Row>
          <Row label="Number of segments">
            {segments}
          </Row>
          <Row label="Points">
            <CollectionField
              onOpen={() =>
                openPoints({ title: "Polyline points", withBulge: true, points: shape.vertices }, (pts) =>
                  edit({ ...shape, vertices: pts }),
                )
              }
            />
          </Row>
        </>
      );
    }
    case "Rectangle":
      return (
        <>
          <>
            <Row label="Direction">
              {direction ?? ""}
            </Row>
            <Row label="Number of segments">
              4
            </Row>
          </>
          <>
            <Row label="Lower left">
              <PointField value={shape.lower_left} onSave={(lower_left) => edit({ ...shape, lower_left })} />
            </Row>
            <Row label="Width">
              <NumberField value={shape.width} onSave={(width) => edit({ ...shape, width })} />
            </Row>
            <Row label="Height">
              <NumberField value={shape.height} onSave={(height) => edit({ ...shape, height })} />
            </Row>
            <Row label="Rotation">
              <NumberField value={shape.rotation} onSave={(rotation) => edit({ ...shape, rotation })} />
            </Row>
          </>
        </>
      );
    case "Points":
      return (
        <>
          <Row label="Number of points">
            {shape.points.length}
          </Row>
          <Row label="Points">
            <CollectionField
              onOpen={() =>
                openPoints({ title: "Point list", withBulge: false, points: shape.points }, (pts) =>
                  edit({ ...shape, points: pts.map(({ x, y }) => ({ x, y })) }),
                )
              }
            />
          </Row>
        </>
      );
    case "Spline":
      return (
        <>
          <Row label="Degree">
            {shape.degree}
          </Row>
          <Row label="Control points">
            {shape.control_points.length}
          </Row>
          <Row label="Rational">
            {shape.weights.length > 0 ? "True" : "False"}
          </Row>
          <Row label="Closed">
            {shape.closed ? "True" : "False"}
          </Row>
        </>
      );
    case "Ellipse":
      return (
        <>
          <Row label="Center">
            {pointText(shape.center)}
          </Row>
          <Row label="Major axis">
            {pointText(shape.major_axis)}
          </Row>
          <Row label="Ratio">
            {num(shape.ratio)}
          </Row>
          <Row label="Start parameter">
            {num(shape.start_param)}
          </Row>
          <Row label="End parameter">
            {num(shape.end_param)}
          </Row>
        </>
      );
  }
}

// --- Operations -----------------------------------------------------------------------------

/** "T3: 6mm 2F Spiral End Mill, 20mm flute (Ø6)" */
const toolText = (number: number, t: Tool) => `T${number}: ${t.name} (Ø${num(t.diameter)})`;

/** Same tool values (ignoring the master-list id, which differs between computers). */
const sameTool = (a: Tool, b: Tool) => JSON.stringify({ ...a, id: 0 }) === JSON.stringify({ ...b, id: 0 });

/** The active library's tools on this computer, with their numbers. */
function libraryEntries(tooling: Tooling, library: string | null) {
  const lib = library === null ? undefined : tooling.libraries.find((l) => l.name === library);
  const entries = (lib?.tools ?? []).flatMap((e) => {
    const t = tooling.tools.find((x) => x.id === e.tool_id);
    return t ? [{ number: e.number, tool: t }] : [];
  });
  return { lib, entries };
}

/**
 * A Tool drop-down: the `fixed` choices first (e.g. "Custom tool", "Use Machining tool …"), then
 * the active library's tools. `current` is the picked library tool (number and copy), or null
 * when a fixed choice is in use (`value` says which). A picked tool
 * no longer matching the library is still shown, as kept in the project. Only end mills can
 * be picked in v1.
 */
function ToolSelect({
  tooling,
  library,
  fixed,
  value,
  current,
  onPick,
}: {
  tooling: Tooling;
  library: string | null;
  /** [key, label] of the choices above the library tools. */
  fixed: [string, string][];
  /** One of the fixed keys, or "library". */
  value: string;
  current: { number: number; tool: Tool } | null;
  /** A fixed key, or a tool number. */
  onPick: (choice: string | number) => void;
}) {
  const { lib, entries } = libraryEntries(tooling, library);
  const match =
    value === "library" && current
      ? entries.find((e) => e.number === current.number && sameTool(e.tool, current.tool))
      : undefined;
  const selected = value !== "library" ? value : match ? String(match.number) : "kept";
  const changed = value === "library" && current && !match && entries.some((e) => e.number === current.number);
  return (
    <>
      <select
        className="prop-input"
        value={selected}
        onChange={(e) => {
          const v = e.target.value;
          if (v === "kept") return;
          onPick(/^\d+$/.test(v) ? Number(v) : v);
        }}
      >
        {fixed.map(([k, label]) => (
          <option key={k} value={k}>
            {label}
          </option>
        ))}
        {value === "library" && current && !match && (
          <option value="kept">{toolText(current.number, current.tool)} – kept in project</option>
        )}
        {library !== null && !lib && <option disabled>"{library}" not on this computer</option>}
        {entries.map((e) => (
          <option key={e.number} value={String(e.number)} disabled={e.tool.end_type !== "Flat"}>
            {toolText(e.number, e.tool)}
            {e.tool.end_type !== "Flat" ? ` – ${endTypeLabel(e.tool.end_type).toLowerCase()}, not supported in v1` : ""}
          </option>
        ))}
      </select>
      {changed && (
        <div className="muted prop-note">T{current!.number} has changed in the library: pick it again to update.</div>
      )}
    </>
  );
}

/** "T3: name (Ø6)" for a picked tool, or "no tool". */
const pickedText = (p: PickedTool | null) => (p ? toolText(p.number, p.tool) : "no tool");

/** The Machining or a part's Tool drop-down. */
function LevelToolSelect({
  doc,
  tooling,
  picked,
  first,
  onPick,
}: {
  doc: Document;
  tooling: Tooling;
  picked: PickedTool | null;
  first: string;
  onPick: (number: number | null) => void;
}) {
  return (
    <ToolSelect
      tooling={tooling}
      library={doc.active_library}
      fixed={[["first", first]]}
      value={picked ? "library" : "first"}
      current={picked}
      onPick={(c) => onPick(typeof c === "number" ? c : null)}
    />
  );
}

/** The operation's Tool drop-down: a custom tool, the part/Machining tool, or a library tool. */
function OperationToolSelect({
  op,
  doc,
  tooling,
  onPick,
}: {
  op: Operation;
  doc: Document;
  tooling: Tooling;
  onPick: (id: number, tool: OperationTool) => Promise<boolean>;
}) {
  const c = op.common;
  const part = doc.parts.find((p) => p.operations.some((o) => o.id === op.id));
  const inherited = part?.tool
    ? `Use part tool (${pickedText(part.tool)})`
    : doc.machining_tool
      ? `Use Machining tool (${pickedText(doc.machining_tool)})`
      : doc.active_library === null
        ? "No library selected"
        : "No tool selected";
  const value = c.tool_source === "Inherit" ? "inherit" : c.tool_source === "Custom" ? "custom" : "library";
  return (
    <ToolSelect
      tooling={tooling}
      library={doc.active_library}
      fixed={[
        ["custom", "Custom tool"],
        ["inherit", inherited],
      ]}
      value={value}
      current={c.tool_source === "Library" && c.tool ? { number: c.tool_number, tool: c.tool } : null}
      onPick={(choice) =>
        void onPick(op.id, choice === "inherit" ? "Inherit" : choice === "custom" ? "Custom" : { Library: choice as number })
      }
    />
  );
}

/**
 * A custom tool's settings (as in the Add tool window, without the name: a custom tool isn't
 * saved to a library), edited in place.
 */
function CustomToolRows({ op, onSet }: { op: Operation; onSet: (op: Operation) => Promise<boolean> }) {
  const c = op.common;
  const t = c.tool;
  if (!t) return null;
  const set = (patch: Partial<Tool>) => onSet({ ...op, common: { ...c, tool: { ...t, ...patch } } });
  const n = (value: number, save: (v: number) => Promise<boolean>) => <NumberField value={value} onSave={save} />;
  const whole = (text: string) => (/^\d+$/.test(text.trim()) ? Number(text) : null);
  return (
    <>
      <Row label="Tool number">
        <TextField
          shown={String(c.tool_number)}
          parse={whole}
          onSave={(tool_number) => onSet({ ...op, common: { ...c, tool_number } })}
        />
      </Row>
      <Row label="End type">
        <select className="prop-input" value={t.end_type} onChange={(e) => void set({ end_type: e.target.value as EndType })}>
          {END_TYPES.map(([v, label]) => (
            <option key={v} value={v} disabled={v !== "Flat"}>
              {label}
              {v !== "Flat" ? " – not supported in v1" : ""}
            </option>
          ))}
        </select>
      </Row>
      {t.end_type !== "Drill" && (
        <Row label="Flute type">
          <ChoiceField<FluteType> value={t.flute_type} choices={FLUTE_TYPES} onSave={(flute_type) => void set({ flute_type })} />
        </Row>
      )}
      {t.flute_type !== "Straight" && t.end_type !== "Drill" && (
        <Row label="Helix angle (°)">{n(t.helix_angle, (helix_angle) => set({ helix_angle }))}</Row>
      )}
      {t.end_type === "VBit" && <Row label="V angle (°)">{n(t.v_angle, (v_angle) => set({ v_angle }))}</Row>}
      <Row label="Diameter">{n(t.diameter, (diameter) => set({ diameter }))}</Row>
      <Row label="Shank diameter">
        <TextField
          shown={t.shank_diameter === null ? "" : String(Number(t.shank_diameter.toFixed(4)))}
          parse={(text) => (text.trim() === "" ? "same" : parseNumber(text))}
          onSave={(v) => set({ shank_diameter: v === "same" ? null : (v as number) })}
        />
      </Row>
      <Row label="Flutes">
        <TextField shown={String(t.flutes)} parse={whole} onSave={(flutes) => set({ flutes })} />
      </Row>
      <Row label="Flute length">{n(t.flute_length, (flute_length) => set({ flute_length }))}</Row>
      <Row label="Overall length">{n(t.overall_length, (overall_length) => set({ overall_length }))}</Row>
    </>
  );
}

function OperationRows({
  op,
  state,
  onSet,
  onSetShapes,
  doc,
  tooling,
  onPickTool,
}: {
  op: Operation;
  state: OperationState | undefined;
  onSet: (op: Operation) => Promise<boolean>;
  onSetShapes: (id: number, shapeIds: number[]) => Promise<boolean>;
  doc: Document;
  tooling: Tooling;
  onPickTool: (id: number, tool: OperationTool) => Promise<boolean>;
}) {
  const c = op.common;
  const drill = op.kind.type === "Drill";
  const common = (patch: Partial<Operation["common"]>) => onSet({ ...op, common: { ...c, ...patch } });
  const num = (value: number, save: (v: number) => Promise<boolean>) => <NumberField value={value} onSave={save} />;
  return (
    <>
      <Row label="Name">
        <TextField shown={op.name} parse={(t) => (t.trim() ? t.trim() : null)} onSave={(name) => onSet({ ...op, name })} />
      </Row>
      <Row label="Type">{op.kind.type}</Row>
      <Row label="Enabled">
        <BoolField value={op.enabled} onSave={(enabled) => void onSet({ ...op, enabled })} />
      </Row>
      <Row label="Show toolpath">
        <BoolField value={op.show_toolpath} onSave={(show_toolpath) => void onSet({ ...op, show_toolpath })} />
      </Row>
      <Row label="Shapes">
        <TextField
          shown={op.shape_ids.join(", ")}
          parse={parseIds}
          onSave={(ids) => onSetShapes(op.id, ids)}
        />
      </Row>
      {state?.message && (
        <Row label={state.status === "outdated" ? "Last error" : "Error"}>
          <span className="prop-error">{state.message}</span>
        </Row>
      )}
      <GroupRow label="Common" />
      <Row label="Tool">
        <OperationToolSelect op={op} doc={doc} tooling={tooling} onPick={onPickTool} />
      </Row>
      {c.tool_source === "Custom" ? (
        <CustomToolRows op={op} onSet={onSet} />
      ) : (
        <>
          {/* From the library, the part or Machining: shown, not editable here. */}
          <Row label="Tool number">
            <span className="muted">{state?.tool_number ?? "–"}</span>
          </Row>
          <Row label="Tool diameter">
            <span className="muted">{state?.tool_diameter != null ? String(Number(state.tool_diameter.toFixed(4))) : "–"}</span>
          </Row>
        </>
      )}
      <Row label="Stock surface">{num(c.stock_surface, (stock_surface) => common({ stock_surface }))}</Row>
      <Row label="Target depth">
        <TextField
          shown={c.target_depth === null ? "" : String(Number(c.target_depth.toFixed(4)))}
          parse={parseNumber}
          onSave={(target_depth) => common({ target_depth })}
        />
      </Row>
      {/* Drilling has no depth passes: Standard and Peck don't use these; Spiral uses the depth
          increment as its depth per turn. */}
      {(op.kind.type !== "Drill" || op.kind.method === "Spiral") && (
        <Row label={drill ? "Depth per turn" : "Depth increment"}>
          {num(c.depth_increment, (depth_increment) => common({ depth_increment }))}
        </Row>
      )}
      {!drill && (
        <Row label="Final depth increment">
          {num(c.final_depth_increment, (final_depth_increment) => common({ final_depth_increment }))}
        </Row>
      )}
      <Row label="Clearance plane">{num(c.clearance_plane, (clearance_plane) => common({ clearance_plane }))}</Row>
      <Row label="Fast plunge height">
        {num(c.fast_plunge_height, (fast_plunge_height) => common({ fast_plunge_height }))}
      </Row>
      <Row label="Cut feed rate">{num(c.cut_feedrate, (cut_feedrate) => common({ cut_feedrate }))}</Row>
      <Row label="Plunge feed rate">{num(c.plunge_feedrate, (plunge_feedrate) => common({ plunge_feedrate }))}</Row>
      <Row label="Spindle speed">{num(c.spindle_speed, (spindle_speed) => common({ spindle_speed }))}</Row>
      <Row label="Spindle direction">
        <ChoiceField
          value={c.spindle_direction}
          choices={[
            ["CW", "CW"],
            ["CCW", "CCW"],
            ["Off", "Off"],
          ]}
          onSave={(spindle_direction) => void common({ spindle_direction })}
        />
      </Row>
      {!drill && (
        <Row label="Roughing clearance">
          {num(c.roughing_clearance, (roughing_clearance) => common({ roughing_clearance }))}
        </Row>
      )}
      <KindRows op={op} state={state} onSet={onSet} />
    </>
  );
}

const MILLING_DIRECTIONS: [MillingDirection, string][] = [
  ["Conventional", "Conventional"],
  ["Climb", "Climb"],
];
const CUT_ORDERINGS: [CutOrdering, string][] = [
  ["DepthFirst", "Depth first"],
  ["LevelFirst", "Level first"],
];

/** The settings of the operation's own type (Profile or Pocket). */
function KindRows({
  op,
  state,
  onSet,
}: {
  op: Operation;
  state: OperationState | undefined;
  onSet: (op: Operation) => Promise<boolean>;
}) {
  const k = op.kind;
  const kind = (patch: Partial<ProfileSettings> | Partial<PocketSettings> | Partial<DrillSettings>) =>
    onSet({ ...op, kind: { ...k, ...patch } as OperationKind });
  const num = (value: number, save: (v: number) => Promise<boolean>) => <NumberField value={value} onSave={save} />;
  const direction = (
    <Row label="Milling direction">
      <ChoiceField
        value={k.milling_direction}
        choices={MILLING_DIRECTIONS}
        onSave={(milling_direction) => void kind({ milling_direction })}
      />
    </Row>
  );
  const ordering = (value: CutOrdering) => (
    <Row label="Cut ordering">
      <ChoiceField value={value} choices={CUT_ORDERINGS} onSave={(cut_ordering) => void kind({ cut_ordering })} />
    </Row>
  );
  switch (k.type) {
    case "Profile": {
      // Open shapes can only be cut On line, so that is all that's offered for them.
      const open = state?.has_open_shapes ?? false;
      const sides: [ProfileSide, string][] = open
        ? [["OnLine", "On line"]]
        : [
            ["Outside", "Outside"],
            ["Inside", "Inside"],
            ["OnLine", "On line"],
          ];
      if (open && k.side !== "OnLine") sides.push([k.side, `${k.side} (open shapes need On line)`]);
      return (
        <>
          <GroupRow label="Profile" />
          <Row label="Side">
            <ChoiceField value={k.side} choices={sides} onSave={(side) => void kind({ side })} />
          </Row>
          {direction}
          {ordering(k.cut_ordering)}
        </>
      );
    }
    case "Drill":
      return (
        <>
          <GroupRow label="Drill" />
          <Row label="Method">
            <ChoiceField<DrillMethod>
              value={k.method}
              choices={[
                ["Standard", "Standard (straight plunge)"],
                ["Peck", "Peck"],
                ["Spiral", "Spiral"],
              ]}
              onSave={(method) => void kind({ method })}
            />
          </Row>
          <Row label="Retract height">{num(k.retract_height, (retract_height) => kind({ retract_height }))}</Row>
          {k.method === "Peck" && (
            <Row label="Peck distance">{num(k.peck_distance, (peck_distance) => kind({ peck_distance }))}</Row>
          )}
          {k.method === "Spiral" && (
            <>
              {/* Circles are cut to their own diameter; this is for point lists and arcs. */}
              <Row label="Hole diameter (points, arcs)">
                {num(k.hole_diameter, (hole_diameter) => kind({ hole_diameter }))}
              </Row>
              {direction}
            </>
          )}
        </>
      );
    case "Pocket":
      return (
        <>
          <GroupRow label="Pocket" />
          <Row label="Stepover">{num(k.stepover, (stepover) => kind({ stepover }))}</Row>
          <Row label="Stepover feed rate">
            <ChoiceField
              value={k.stepover_feed}
              choices={[
                ["Plunge", "Plunge feed rate"],
                ["Cut", "Cut feed rate"],
              ]}
              onSave={(stepover_feed) => void kind({ stepover_feed })}
            />
          </Row>
          {direction}
          {ordering(k.cut_ordering)}
        </>
      );
  }
}

// --- Panel ----------------------------------------------------------------------------------

interface Edits {
  /** Resolves false if the rename was rejected (the box then shows the old name again). */
  onRenameLayer: (oldName: string, newName: string) => Promise<boolean>;
  onSetLayerColor: (name: string, color: string) => void;
  /** Resolves false if the engine refused the change. */
  onSetShape: (id: number, shape: Shape) => Promise<boolean>;
  onSetGridSpacing: (mm: number) => Promise<boolean>;
  /** Resolves false if the engine refused the change. */
  onSetOperation: (op: Operation) => Promise<boolean>;
  /** Resolves false if the engine refused the shapes. */
  onSetOperationShapes: (id: number, shapeIds: number[]) => Promise<boolean>;
  operationStates: OperationState[];
  /** This computer's tools and libraries. */
  tooling: Tooling;
  /** Sets how an operation gets its tool. */
  onPickTool: (id: number, tool: OperationTool) => Promise<boolean>;
  /** Sets the Machining tool / a part's tool to a library tool number (null = none). */
  onSetMachiningTool: (number: number | null) => Promise<boolean>;
  onSetPartTool: (part: number, number: number | null) => Promise<boolean>;
  onSetActiveLibrary: (name: string | null) => Promise<boolean>;
  /** Resolves false if the rename was rejected. */
  onRenameLibrary: (oldName: string, newName: string) => Promise<boolean>;
  onAddLibraryTool: (library: string, toolId: number, number: number) => Promise<boolean>;
  onSetLibraryToolNumber: (library: string, oldNumber: number, newNumber: number) => Promise<boolean>;
  onRemoveLibraryTool: (library: string, number: number) => Promise<boolean>;
}

/** The project's tool library drop-down. */
function LibraryChoice({ doc, tooling, onSet }: { doc: Document; tooling: Tooling; onSet: (n: string | null) => void }) {
  const active = doc.active_library;
  const missing = active !== null && !tooling.libraries.some((l) => l.name === active);
  return (
    <>
      <select
        className="prop-input"
        value={active ?? ""}
        onChange={(e) => onSet(e.target.value === "" ? null : e.target.value)}
      >
        <option value="">No library selected</option>
        {missing && <option value={active!}>{active} – not on this computer</option>}
        {tooling.libraries.map((l) => (
          <option key={l.name} value={l.name}>
            {l.name}
          </option>
        ))}
      </select>
    </>
  );
}

/** A tool library: its numbered tools, and adding tools from the master list. */
function LibraryRows({ name, edits }: { name: string; edits: Edits }) {
  const { tooling } = edits;
  const lib = tooling.libraries.find((l) => l.name === name);
  const [toolId, setToolId] = useState<string>("");
  const [number, setNumber] = useState("");
  if (!lib) return <div className="empty">Select an item to see its properties.</div>;
  const nextFree = () => {
    let n = 1;
    while (lib.tools.some((t) => t.number === n)) n++;
    return n;
  };
  const numberText = number === "" ? String(nextFree()) : number;
  const numberOk = /^\d+$/.test(numberText.trim()) && Number(numberText) >= 1;
  const chosen = toolId === "" ? tooling.tools[0]?.id : Number(toolId);
  return (
    <>
      <Grid>
        <Row label="Name">
          <TextField
            shown={lib.name}
            parse={(t) => (t.trim() ? t : null)}
            onSave={(n) => edits.onRenameLibrary(lib.name, n)}
          />
        </Row>
      </Grid>
      <table className="points-table library-table">
        <thead>
          <tr>
            <th>No.</th>
            <th>Tool</th>
            <th>Ø</th>
            <th />
          </tr>
        </thead>
        <tbody>
          {lib.tools.map((e) => {
            const t = tooling.tools.find((x) => x.id === e.tool_id);
            return (
              <tr key={e.number}>
                <td>
                  <TextField
                    shown={String(e.number)}
                    parse={(s) => (/^\d+$/.test(s.trim()) ? Number(s) : null)}
                    onSave={(n) => edits.onSetLibraryToolNumber(lib.name, e.number, n)}
                  />
                </td>
                <td title={t ? endTypeLabel(t.end_type) : ""}>
                  {t?.name ?? "?"}
                  {t && t.end_type !== "Flat" && <span className="muted"> ({endTypeLabel(t.end_type).toLowerCase()})</span>}
                </td>
                <td>{t ? num(t.diameter) : ""}</td>
                <td>
                  <button
                    type="button"
                    className="remove"
                    title="Take this tool out of the library (it stays in the master tool list)"
                    onClick={() => void edits.onRemoveLibraryTool(lib.name, e.number)}
                  >
                    ✕
                  </button>
                </td>
              </tr>
            );
          })}
          {lib.tools.length === 0 && (
            <tr>
              <td colSpan={4} className="muted">
                No tools yet.
              </td>
            </tr>
          )}
        </tbody>
      </table>
      {tooling.tools.length === 0 ? (
        <div className="muted prop-note">Add tools to the Master tool list first.</div>
      ) : (
        <div className="library-add">
          <select className="prop-input" value={String(chosen)} onChange={(e) => setToolId(e.target.value)}>
            {tooling.tools.map((t) => (
              <option key={t.id} value={String(t.id)}>
                {t.name} (Ø{num(t.diameter)})
              </option>
            ))}
          </select>
          <input
            className={`prop-input number${numberOk ? "" : " bad"}`}
            title="Tool number"
            value={numberText}
            onChange={(e) => setNumber(e.target.value)}
          />
          <button
            type="button"
            disabled={!numberOk || chosen === undefined}
            onClick={async () => {
              if (await edits.onAddLibraryTool(lib.name, chosen!, Number(numberText))) setNumber("");
            }}
          >
            Add tool
          </button>
        </div>
      )}
    </>
  );
}

function LayerRows({ layer, onRenameLayer, onSetLayerColor }: { layer: Layer } & Edits) {
  return (
    <>
      <Row label="Name">
        <TextField
          shown={layer.name}
          parse={(t) => t}
          onSave={(name) => onRenameLayer(layer.name, name)}
        />
      </Row>
      <Row label="Colour">
        <ColorSwatch color={layer.color} onPick={(c) => onSetLayerColor(layer.name, c)} size="large" />{" "}
        <span className="muted">{layer.color}</span>
      </Row>
      <Row label="Objects">
        {layer.objects.length}
      </Row>
    </>
  );
}

function Content({
  doc,
  display,
  selection,
  edits,
  openPoints,
}: {
  doc: Document;
  display: DisplayData | null;
  selection: Selection;
  edits: Edits;
  openPoints: (e: PointsEdit, save: (pts: { x: number; y: number; bulge: number }[]) => Promise<boolean>) => void;
}) {
  const nothing = <div className="empty">Select an item to see its properties.</div>;
  switch (selection.kind) {
    case "layer": {
      const layer = doc.layers[selection.layer];
      return layer ? (
        <Grid>
          <LayerRows layer={layer} {...edits} />
        </Grid>
      ) : (
        nothing
      );
    }
    case "project":
      return (
        <Grid>
          <>
            <Row label="Name">
              {doc.name}
            </Row>
          </>
          <>
            <Row label="Grid spacing (mm)">
              <NumberField value={doc.grid_spacing} onSave={edits.onSetGridSpacing} />
            </Row>
          </>
        </Grid>
      );
    case "machining":
      return (
        <Grid>
          <Row label="Tool library">
            <LibraryChoice doc={doc} tooling={edits.tooling} onSet={(n) => void edits.onSetActiveLibrary(n)} />
          </Row>
          <Row label="Tool">
            <LevelToolSelect
              doc={doc}
              tooling={edits.tooling}
              picked={doc.machining_tool}
              first={doc.active_library === null ? "No library selected" : "No tool selected"}
              onPick={(n) => void edits.onSetMachiningTool(n)}
            />
          </Row>
        </Grid>
      );
    case "part": {
      const part = doc.parts[selection.part];
      return part ? (
        <Grid>
          <>
            <Row label="Name">
              {part.name}
            </Row>
            <Row label="Tool">
              <LevelToolSelect
                doc={doc}
                tooling={edits.tooling}
                picked={part.tool}
                first={`Use Machining tool (${pickedText(doc.machining_tool)})`}
                onPick={(n) => void edits.onSetPartTool(selection.part, n)}
              />
            </Row>
          </>
        </Grid>
      ) : (
        nothing
      );
    }
    case "operation": {
      const op = doc.parts.flatMap((p) => p.operations).find((o) => o.id === selection.id);
      return op ? (
        <Grid>
          <OperationRows
            key={op.id}
            op={op}
            state={edits.operationStates.find((s) => s.id === op.id)}
            onSet={edits.onSetOperation}
            onSetShapes={edits.onSetOperationShapes}
            doc={doc}
            tooling={edits.tooling}
            onPickTool={edits.onPickTool}
          />
        </Grid>
      ) : (
        nothing
      );
    }
    case "library":
      return <LibraryRows key={selection.name} name={selection.name} edits={edits} />;
    case "objects": {
      if (selection.ids.length > 1) return <div className="empty">{selection.ids.length} objects selected.</div>;
      for (const layer of doc.layers) {
        const obj = layer.objects.find((o) => o.id === selection.ids[0]);
        if (!obj) continue;
        const direction = display?.shapes.find((s) => s.id === obj.id)?.direction ?? null;
        return (
          <Grid>
            <>
              <Row label="ID">
                {obj.id}
              </Row>
              <Row label="Layer">
                {layer.name}
              </Row>
              <Row label="Type">
                {shapeLabel(obj.shape)}
              </Row>
            </>
            <ShapeRows
              key={obj.id}
              shape={obj.shape}
              direction={direction}
              edit={(shape) => edits.onSetShape(obj.id, shape)}
              openPoints={openPoints}
            />
          </Grid>
        );
      }
      return nothing;
    }
  }
}

export default function Properties({
  doc,
  display,
  selection,
  ...edits
}: { doc: Document | null; display: DisplayData | null; selection: Selection } & Edits) {
  const [points, setPoints] = useState<{
    edit: PointsEdit;
    save: (pts: { x: number; y: number; bulge: number }[]) => Promise<boolean>;
  } | null>(null);

  if (!doc) return <div className="empty">Select an item to see its properties.</div>;
  return (
    <>
      <Content
        doc={doc}
        display={display}
        selection={selection}
        edits={edits}
        openPoints={(edit, save) => setPoints({ edit, save })}
      />
      {points && <PointsDialog edit={points.edit} onSave={points.save} onClose={() => setPoints(null)} />}
    </>
  );
}
