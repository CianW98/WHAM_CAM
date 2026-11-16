// The properties panel: the settings of whatever is selected, editable in place.
// Values only change through the engine.

import { useEffect, useRef, useState, type ReactNode } from "react";
import type { DisplayData, Document, Layer, Point, Shape } from "../engine";
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

// --- Panel ----------------------------------------------------------------------------------

interface Edits {
  /** Resolves false if the rename was rejected (the box then shows the old name again). */
  onRenameLayer: (oldName: string, newName: string) => Promise<boolean>;
  onSetLayerColor: (name: string, color: string) => void;
  /** Resolves false if the engine refused the change. */
  onSetShape: (id: number, shape: Shape) => Promise<boolean>;
  onSetGridSpacing: (mm: number) => Promise<boolean>;
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
