import { useEffect, useRef, useState, type ReactNode } from "react";
import type { Document, Layer, Point, Shape } from "../engine";
import type { Selection } from "../selection";
import { shapeLabel } from "./Tree";
import ColorSwatch from "./ColorSwatch";

/** Numbers as the engine sent them, rounded for display with trailing zeros trimmed. */
const num = (n: number) => String(Number(n.toFixed(4)));

/** Sends a changed shape to the engine. Resolves false if the engine refused it. */
type EditShape = (shape: Shape) => Promise<boolean>;

/**
 * A number box. Enter or clicking away saves, Esc cancels. Unchanged text is not saved, so a
 * value shown rounded is never overwritten by its rounded form.
 */
function NumberField({ value, onSave }: { value: number; onSave: (v: number) => Promise<boolean> }) {
  const [text, setText] = useState(num(value));
  const cancelled = useRef(false);
  useEffect(() => setText(num(value)), [value]);

  const save = async () => {
    if (cancelled.current) {
      cancelled.current = false;
      return;
    }
    const t = text.trim();
    if (t === num(value)) return;
    const v = Number(t);
    if (t === "" || !Number.isFinite(v) || !(await onSave(v))) setText(num(value));
  };

  return (
    <input
      className="prop-input num"
      value={text}
      spellCheck={false}
      onChange={(e) => setText(e.target.value)}
      onBlur={save}
      onKeyDown={(e) => {
        if (e.key === "Enter") e.currentTarget.blur();
        if (e.key === "Escape") {
          cancelled.current = true;
          setText(num(value));
          e.currentTarget.blur();
        }
      }}
    />
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

/** X and Y boxes for a point; `set` gets the point with one coordinate changed. */
function PointRows({ label, p, set }: { label: string; p: Point; set: (p: Point) => Promise<boolean> }) {
  return (
    <>
      <Row label={`${label} X`}>
        <NumberField value={p.x} onSave={(x) => set({ ...p, x })} />
      </Row>
      <Row label={`${label} Y`}>
        <NumberField value={p.y} onSave={(y) => set({ ...p, y })} />
      </Row>
    </>
  );
}

/** A table of X/Y boxes, one row per point of a point list. */
function PointTable({ points, set }: { points: Point[]; set: (i: number, p: Point) => Promise<boolean> }) {
  return (
    <tr>
      <td colSpan={2}>
        <table className="point-table">
          <thead>
            <tr>
              <th>#</th>
              <th>X</th>
              <th>Y</th>
            </tr>
          </thead>
          <tbody>
            {points.map((p, i) => (
              <tr key={i}>
                <td className="muted">{i + 1}</td>
                <td>
                  <NumberField value={p.x} onSave={(x) => set(i, { ...p, x })} />
                </td>
                <td>
                  <NumberField value={p.y} onSave={(y) => set(i, { ...p, y })} />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </td>
    </tr>
  );
}

/** The rows for a shape's own values; editable except for splines and ellipses. */
function ShapeRows({ shape, edit }: { shape: Shape; edit: EditShape }) {
  switch (shape.type) {
    case "Line":
      return (
        <>
          <PointRows label="Start" p={shape.start} set={(start) => edit({ ...shape, start })} />
          <PointRows label="End" p={shape.end} set={(end) => edit({ ...shape, end })} />
        </>
      );
    case "Arc":
      return (
        <>
          <PointRows label="Centre" p={shape.center} set={(center) => edit({ ...shape, center })} />
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
          <PointRows label="Centre" p={shape.center} set={(center) => edit({ ...shape, center })} />
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
            <input
              type="checkbox"
              checked={shape.closed}
              onChange={(e) => void edit({ ...shape, closed: e.target.checked })}
            />
          </Row>
          <Row label="Number of segments">{segments}</Row>
        </>
      );
    }
    case "Points":
      return (
        <>
          <Row label="Number of points">{shape.points.length}</Row>
          <PointTable
            points={shape.points}
            set={(i, p) => edit({ ...shape, points: shape.points.map((q, j) => (j === i ? p : q)) })}
          />
        </>
      );
    case "Spline":
      return (
        <>
          <Row label="Degree">{shape.degree}</Row>
          <Row label="Control points">{shape.control_points.length}</Row>
          <Row label="Rational">{shape.weights.length > 0 ? "True" : "False"}</Row>
          <Row label="Closed">{shape.closed ? "True" : "False"}</Row>
        </>
      );
    case "Ellipse":
      return (
        <>
          <Row label="Centre">{`(${num(shape.center.x)}, ${num(shape.center.y)})`}</Row>
          <Row label="Major axis">{`(${num(shape.major_axis.x)}, ${num(shape.major_axis.y)})`}</Row>
          <Row label="Ratio">{num(shape.ratio)}</Row>
          <Row label="Start parameter">{num(shape.start_param)}</Row>
          <Row label="End parameter">{num(shape.end_param)}</Row>
        </>
      );
  }
}

interface Edits {
  /** Resolves false if the rename was rejected (the box then shows the old name again). */
  onRenameLayer: (oldName: string, newName: string) => Promise<boolean>;
  onSetLayerColor: (name: string, color: string) => void;
  /** Resolves false if the engine refused the change. */
  onSetShape: (id: number, shape: Shape) => Promise<boolean>;
  onSetGridSpacing: (mm: number) => Promise<boolean>;
}

/** Editable layer properties. Name: Enter or leaving the box saves, Esc cancels. */
function LayerProperties({
  layer,
  onRenameLayer,
  onSetLayerColor,
}: { layer: Layer } & Pick<Edits, "onRenameLayer" | "onSetLayerColor">) {
  const [name, setName] = useState(layer.name);
  const save = async () => {
    if (name.trim() === layer.name || !(await onRenameLayer(layer.name, name))) setName(layer.name);
  };

  return (
    <table className="props">
      <tbody>
        <tr>
          <th>Name</th>
          <td>
            <input
              className="prop-input"
              value={name}
              onChange={(e) => setName(e.target.value)}
              onBlur={save}
              onKeyDown={(e) => {
                if (e.key === "Enter") e.currentTarget.blur();
                if (e.key === "Escape") {
                  setName(layer.name);
                  e.currentTarget.blur();
                }
              }}
            />
          </td>
        </tr>
        <tr>
          <th>Colour</th>
          <td>
            <ColorSwatch color={layer.color} onPick={(c) => onSetLayerColor(layer.name, c)} size="large" />{" "}
            <span className="muted">{layer.color}</span>
          </td>
        </tr>
        <tr>
          <th>Objects</th>
          <td>{layer.objects.length}</td>
        </tr>
      </tbody>
    </table>
  );
}

const Empty = ({ children }: { children: ReactNode }) => <div className="empty">{children}</div>;

export default function Properties({
  doc,
  selection,
  onRenameLayer,
  onSetLayerColor,
  onSetShape,
  onSetGridSpacing,
}: { doc: Document | null; selection: Selection } & Edits) {
  const nothing = <Empty>Select an item to see its properties.</Empty>;
  if (!doc) return nothing;
  switch (selection.kind) {
    case "layer": {
      const layer = doc.layers[selection.layer];
      if (!layer) return nothing;
      // Keyed by name and colour so the boxes reset after a rename, undo or failed edit.
      return (
        <LayerProperties
          key={`${layer.name}|${layer.color}`}
          layer={layer}
          onRenameLayer={onRenameLayer}
          onSetLayerColor={onSetLayerColor}
        />
      );
    }
    case "project":
      return (
        <table className="props">
          <tbody>
            <Row label="Name">{doc.name}</Row>
            <Row label="Grid spacing (mm)">
              <NumberField value={doc.grid_spacing} onSave={onSetGridSpacing} />
            </Row>
          </tbody>
        </table>
      );
    case "part": {
      const part = doc.parts[selection.part];
      if (!part) return nothing;
      return (
        <table className="props">
          <tbody>
            <Row label="Name">{part.name}</Row>
          </tbody>
        </table>
      );
    }
    case "objects": {
      if (selection.ids.length > 1) return <Empty>{selection.ids.length} objects selected.</Empty>;
      for (const layer of doc.layers) {
        const obj = layer.objects.find((o) => o.id === selection.ids[0]);
        if (!obj) continue;
        return (
          <table className="props">
            <tbody>
              <Row label="ID">{obj.id}</Row>
              <Row label="Layer">{layer.name}</Row>
              <Row label="Type">{shapeLabel(obj.shape)}</Row>
              <ShapeRows key={obj.id} shape={obj.shape} edit={(shape) => onSetShape(obj.id, shape)} />
            </tbody>
          </table>
        );
      }
      return nothing;
    }
  }
}
