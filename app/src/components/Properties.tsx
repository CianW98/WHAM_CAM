import type { Document, Point, Shape } from "../engine";
import type { Selection } from "../selection";
import { shapeLabel } from "./Tree";

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
    case "layer": {
      const layer = doc.layers[sel.layer];
      if (!layer) return [];
      return [
        ["Name", layer.name],
        ["Objects", String(layer.objects.length)],
      ];
    }
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

export default function Properties({ doc, selection }: { doc: Document | null; selection: Selection }) {
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
