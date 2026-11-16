// The properties panel: the settings of whatever is selected, editable in place.
// Values only change through the engine.

import { useEffect, useRef, useState, type ReactNode } from "react";
import type {
  CutOrdering,
  DisplayData,
  Document,
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
} from "../engine";
import type { Selection } from "../selection";
import { shapeLabel } from "./Tree";
import ColorSwatch from "./ColorSwatch";
import PointsDialog, { type PointsEdit } from "./PointsDialog";

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

function OperationRows({
  op,
  state,
  onSet,
  onSetShapes,
}: {
  op: Operation;
  state: OperationState | undefined;
  onSet: (op: Operation) => Promise<boolean>;
  onSetShapes: (id: number, shapeIds: number[]) => Promise<boolean>;
}) {
  const c = op.common;
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
      <Row label="Tool number">
        <TextField
          shown={String(c.tool_number)}
          parse={(t) => (/^\d+$/.test(t.trim()) ? Number(t) : null)}
          onSave={(tool_number) => common({ tool_number })}
        />
      </Row>
      <Row label="Tool diameter">{num(c.tool_diameter, (tool_diameter) => common({ tool_diameter }))}</Row>
      <Row label="Stock surface">{num(c.stock_surface, (stock_surface) => common({ stock_surface }))}</Row>
      <Row label="Target depth">
        <TextField
          shown={c.target_depth === null ? "" : String(Number(c.target_depth.toFixed(4)))}
          parse={parseNumber}
          onSave={(target_depth) => common({ target_depth })}
        />
      </Row>
      <Row label="Depth increment">{num(c.depth_increment, (depth_increment) => common({ depth_increment }))}</Row>
      <Row label="Final depth increment">
        {num(c.final_depth_increment, (final_depth_increment) => common({ final_depth_increment }))}
      </Row>
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
      <Row label="Roughing clearance">
        {num(c.roughing_clearance, (roughing_clearance) => common({ roughing_clearance }))}
      </Row>
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
  const kind = (patch: Partial<ProfileSettings> | Partial<PocketSettings>) =>
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
  const ordering = (
    <Row label="Cut ordering">
      <ChoiceField value={k.cut_ordering} choices={CUT_ORDERINGS} onSave={(cut_ordering) => void kind({ cut_ordering })} />
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
          {ordering}
        </>
      );
    }
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
          {ordering}
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
    case "part": {
      const part = doc.parts[selection.part];
      return part ? (
        <Grid>
          <>
            <Row label="Name">
              {part.name}
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
          />
        </Grid>
      ) : (
        nothing
      );
    }
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
