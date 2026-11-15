import { useState } from "react";
import type { Document, Layer, Point, Shape } from "../engine";
import type { Selection } from "../selection";
import { shapeLabel } from "./Tree";
import ColorSwatch from "./ColorSwatch";

type Row = [string, string];

/** Numbers as the engine sent them, rounded for display with trailing zeros trimmed. */
const num = (n: number) => String(Number(n.toFixed(4)));
const pt = (p: Point) => `(${num(p.x)}, ${num(p.y)})`;

function shapeRows(shape: Shape): Row[] {
  switch (shape.type) {
    case "Line":
      return [
        ["Start", pt(shape.start)],
        ["End", pt(shape.end)],
      ];
    case "Arc":
      return [
        ["Center", pt(shape.center)],
        ["Radius", num(shape.radius)],
        ["Start angle", num(shape.start_angle)],
        ["End angle", num(shape.end_angle)],
      ];
    case "Circle":
      return [
        ["Center", pt(shape.center)],
        ["Radius", num(shape.radius)],
      ];
    case "Polyline": {
      const segments = shape.closed ? shape.vertices.length : shape.vertices.length - 1;
      return [
        ["Closed", shape.closed ? "True" : "False"],
        ["Number of segments", String(segments)],
        ["Points", shape.vertices.map((v) => pt(v)).join(" ")],
      ];
    }
    case "Points":
      return [
        ["Number of points", String(shape.points.length)],
        ["Points", shape.points.map(pt).join(" ")],
      ];
    case "Spline":
      return [
        ["Degree", String(shape.degree)],
        ["Control points", String(shape.control_points.length)],
        ["Rational", shape.weights.length > 0 ? "True" : "False"],
        ["Closed", shape.closed ? "True" : "False"],
      ];
    case "Ellipse":
      return [
        ["Center", pt(shape.center)],
        ["Major axis", pt(shape.major_axis)],
        ["Ratio", num(shape.ratio)],
        ["Start parameter", num(shape.start_param)],
        ["End parameter", num(shape.end_param)],
      ];
  }
}

function rowsFor(doc: Document, sel: Selection): Row[] | string {
  switch (sel.kind) {
    case "project":
      return [["Name", doc.name]];
    case "layer":
      return []; // shown by LayerProperties
    case "part":
      return doc.parts[sel.part] ? [["Name", doc.parts[sel.part].name]] : [];
    case "objects": {
      if (sel.ids.length > 1) return `${sel.ids.length} objects selected.`;
      if (sel.ids.length === 0) return [];
      for (const layer of doc.layers) {
        const obj = layer.objects.find((o) => o.id === sel.ids[0]);
        if (obj) {
          return [["ID", String(obj.id)], ["Layer", layer.name], ["Type", shapeLabel(obj.shape)], ...shapeRows(obj.shape)];
        }
      }
      return [];
    }
  }
}

interface LayerEdits {
  /** Resolves false if the rename was rejected (the box then shows the old name again). */
  onRenameLayer: (oldName: string, newName: string) => Promise<boolean>;
  onSetLayerColor: (name: string, color: string) => void;
}

/** Editable layer properties. Name: Enter or leaving the box saves, Esc cancels. */
function LayerProperties({ layer, onRenameLayer, onSetLayerColor }: { layer: Layer } & LayerEdits) {
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

export default function Properties({
  doc,
  selection,
  ...edits
}: { doc: Document | null; selection: Selection } & LayerEdits) {
  if (doc && selection.kind === "layer" && doc.layers[selection.layer]) {
    const layer = doc.layers[selection.layer];
    // Keyed by name and colour so the boxes reset after a rename, undo or failed edit.
    return <LayerProperties key={`${layer.name}|${layer.color}`} layer={layer} {...edits} />;
  }
  const rows = doc ? rowsFor(doc, selection) : [];
  if (typeof rows === "string") return <div className="empty">{rows}</div>;
  if (rows.length === 0) return <div className="empty">Select an item to see its properties.</div>;
  return (
    <table className="props">
      <tbody>
        {rows.map(([k, v]) => (
          <tr key={k}>
            <th>{k}</th>
            <td>{v}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
