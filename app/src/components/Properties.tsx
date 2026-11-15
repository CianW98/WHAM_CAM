import type { Document, Shape } from "../engine";
import type { Selection } from "../selection";

type Row = [string, string];

/** Numbers are shown as the engine sent them, with trailing zeros trimmed. */
const num = (n: number) => String(Number(n.toFixed(4)));

function shapeRows(shape: Shape): Row[] {
  switch (shape.type) {
    case "Polyline": {
      const segments = shape.closed ? shape.vertices.length : shape.vertices.length - 1;
      return [
        ["Closed", shape.closed ? "True" : "False"],
        ["Number of segments", String(segments)],
        ["Points", shape.vertices.map((v) => `(${num(v.x)}, ${num(v.y)})`).join(" ")],
      ];
    }
    case "Circle":
      return [
        ["Center", `(${num(shape.center.x)}, ${num(shape.center.y)})`],
        ["Radius", num(shape.radius)],
      ];
  }
}

function rowsFor(doc: Document, sel: Selection): Row[] {
  if (!sel) return [];
  switch (sel.kind) {
    case "project":
      return [["Name", doc.name]];
    case "layer": {
      const layer = doc.layers[sel.layer];
      return [
        ["Name", layer.name],
        ["Objects", String(layer.objects.length)],
      ];
    }
    case "object": {
      const layer = doc.layers[sel.layer];
      const obj = layer.objects.find((o) => o.id === sel.id);
      if (!obj) return [];
      return [["ID", String(obj.id)], ["Layer", layer.name], ["Type", obj.shape.type], ...shapeRows(obj.shape)];
    }
    case "part":
      return [["Name", doc.parts[sel.part].name]];
  }
}

export default function Properties({ doc, selection }: { doc: Document | null; selection: Selection }) {
  const rows = doc ? rowsFor(doc, selection) : [];
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
