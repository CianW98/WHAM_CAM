// The properties panel: a property grid with collapsible sections, in the style of classic
// simple CAM programs. Greyed rows are read-only. Values only change through the engine.

import { createContext, useContext, useEffect, useRef, useState, type ReactNode } from "react";
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

/** Names of the collapsed sections, shared so a section stays closed as the selection changes. */
const Collapsed = createContext<{ closed: Set<string>; toggle: (title: string) => void }>({
  closed: new Set(),
  toggle: () => {},
});

function Grid({ children }: { children: ReactNode }) {
  return (
    <table className="props">
      {children}
    </table>
  );
}

/** A collapsible group of rows with a header bar. */
function Section({ title, children }: { title: string; children: ReactNode }) {
  const { closed, toggle } = useContext(Collapsed);
  const open = !closed.has(title);
  return (
    <tbody>
      <tr className="prop-section" onClick={() => toggle(title)}>
        <td colSpan={2}>
          <span className="prop-toggle">{open ? "⊟" : "⊞"}</span>
          {title}
        </td>
      </tr>
      {open && children}
    </tbody>
  );
}

/** One property. Read-only rows are greyed. */
function Row({ label, readOnly, children }: { label: string; readOnly?: boolean; children: ReactNode }) {
  return (
    <tr className={readOnly ? "read-only" : ""}>
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
  return (
    <select className="prop-input" value={value ? "True" : "False"} onChange={(e) => onSave(e.target.value === "True")}>
      <option>True</option>
      <option>False</option>
    </select>
  );
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

/** The sections for a shape's own values; editable except for splines and ellipses. */
function ShapeSections({
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
        <Section title="Line">
          <Row label="Start Point">
            <PointField value={shape.start} onSave={(start) => edit({ ...shape, start })} />
          </Row>
          <Row label="End Point">
            <PointField value={shape.end} onSave={(end) => edit({ ...shape, end })} />
          </Row>
        </Section>
      );
    case "Arc":
      return (
        <Section title="Arc">
          <Row label="Centre">
            <PointField value={shape.center} onSave={(center) => edit({ ...shape, center })} />
          </Row>
          <Row label="Radius">
            <NumberField value={shape.radius} onSave={(radius) => edit({ ...shape, radius })} />
          </Row>
          <Row label="Start Angle">
            <NumberField value={shape.start_angle} onSave={(start_angle) => edit({ ...shape, start_angle })} />
          </Row>
          <Row label="End Angle">
            <NumberField value={shape.end_angle} onSave={(end_angle) => edit({ ...shape, end_angle })} />
          </Row>
        </Section>
      );
    case "Circle":
      return (
        <Section title="Circle">
          <Row label="Centre">
            <PointField value={shape.center} onSave={(center) => edit({ ...shape, center })} />
          </Row>
          <Row label="Radius">
            <NumberField value={shape.radius} onSave={(radius) => edit({ ...shape, radius })} />
          </Row>
        </Section>
      );
    case "Polyline": {
      const segments = shape.closed ? shape.vertices.length : shape.vertices.length - 1;
      return (
        <Section title="Polyline">
          <Row label="Closed">
            <BoolField value={shape.closed} onSave={(closed) => void edit({ ...shape, closed })} />
          </Row>
          <Row label="Direction" readOnly>
            {direction ?? ""}
          </Row>
          <Row label="Number of Segments" readOnly>
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
        </Section>
      );
    }
    case "Rectangle":
      return (
        <>
          <Section title="Polyline">
            <Row label="Direction" readOnly>
              {direction ?? ""}
            </Row>
            <Row label="Number of Segments" readOnly>
              4
            </Row>
          </Section>
          <Section title="Rectangle">
            <Row label="Lower Left">
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
          </Section>
        </>
      );
    case "Points":
      return (
        <Section title="Point List">
          <Row label="Number of Points" readOnly>
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
        </Section>
      );
    case "Spline":
      return (
        <Section title="Spline">
          <Row label="Degree" readOnly>
            {shape.degree}
          </Row>
          <Row label="Control Points" readOnly>
            {shape.control_points.length}
          </Row>
          <Row label="Rational" readOnly>
            {shape.weights.length > 0 ? "True" : "False"}
          </Row>
          <Row label="Closed" readOnly>
            {shape.closed ? "True" : "False"}
          </Row>
        </Section>
      );
    case "Ellipse":
      return (
        <Section title="Ellipse">
          <Row label="Centre" readOnly>
            {pointText(shape.center)}
          </Row>
          <Row label="Major Axis" readOnly>
            {pointText(shape.major_axis)}
          </Row>
          <Row label="Ratio" readOnly>
            {num(shape.ratio)}
          </Row>
          <Row label="Start Parameter" readOnly>
            {num(shape.start_param)}
          </Row>
          <Row label="End Parameter" readOnly>
            {num(shape.end_param)}
          </Row>
        </Section>
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

function LayerSections({ layer, onRenameLayer, onSetLayerColor }: { layer: Layer } & Edits) {
  return (
    <Section title="(General)">
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
      <Row label="Objects" readOnly>
        {layer.objects.length}
      </Row>
    </Section>
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
          <LayerSections layer={layer} {...edits} />
        </Grid>
      ) : (
        nothing
      );
    }
    case "project":
      return (
        <Grid>
          <Section title="(General)">
            <Row label="Name" readOnly>
              {doc.name}
            </Row>
          </Section>
          <Section title="Drawing">
            <Row label="Grid Spacing (mm)">
              <NumberField value={doc.grid_spacing} onSave={edits.onSetGridSpacing} />
            </Row>
          </Section>
        </Grid>
      );
    case "part": {
      const part = doc.parts[selection.part];
      return part ? (
        <Grid>
          <Section title="(General)">
            <Row label="Name" readOnly>
              {part.name}
            </Row>
          </Section>
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
            <Section title="(General)">
              <Row label="ID" readOnly>
                {obj.id}
              </Row>
              <Row label="Layer" readOnly>
                {layer.name}
              </Row>
              <Row label="Primitive Type" readOnly>
                {shapeLabel(obj.shape)}
              </Row>
            </Section>
            <ShapeSections
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
  const [closed, setClosed] = useState<Set<string>>(new Set());
  const toggle = (title: string) =>
    setClosed((c) => {
      const next = new Set(c);
      if (next.has(title)) next.delete(title);
      else next.add(title);
      return next;
    });
  const [points, setPoints] = useState<{
    edit: PointsEdit;
    save: (pts: { x: number; y: number; bulge: number }[]) => Promise<boolean>;
  } | null>(null);

  if (!doc) return <div className="empty">Select an item to see its properties.</div>;
  return (
    <Collapsed.Provider value={{ closed, toggle }}>
      <Content
        doc={doc}
        display={display}
        selection={selection}
        edits={edits}
        openPoints={(edit, save) => setPoints({ edit, save })}
      />
      {points && <PointsDialog edit={points.edit} onSave={points.save} onClose={() => setPoints(null)} />}
    </Collapsed.Provider>
  );
}
