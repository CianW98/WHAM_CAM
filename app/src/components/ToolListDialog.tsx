// The Master tool list window: every physical tool on this computer, with Add / Edit / Delete.
// The engine checks and stores the values; this only shows them and collects the form.

import { useEffect, useRef, useState } from "react";
import type { EndType, FluteType, Tool, ToolDrawing, Tooling, ToolPreview } from "../engine";

export const END_TYPES: [EndType, string][] = [
  ["Flat", "End mill"],
  ["Ball", "Ball nose"],
  ["VBit", "V-bit"],
  ["Drill", "Drill"],
];

export const FLUTE_TYPES: [FluteType, string][] = [
  ["Spiral", "Spiral (upcut)"],
  ["Straight", "Straight"],
  ["Downcut", "Downcut (reverse spiral)"],
];

/** Spiral and downcut flutes have a helix angle. Drills have no flute type (or helix angle) in v1. */
const usesHelix = (flute: FluteType, end: EndType) => flute !== "Straight" && end !== "Drill";

export const endTypeLabel = (t: EndType) => END_TYPES.find(([v]) => v === t)?.[1] ?? t;

const num = (n: number) => String(Number(n.toFixed(4)));

/** Resolves to an error message, or null if it was saved. */
type Save = (tool: Tool) => Promise<string | null>;

interface Props {
  tooling: Tooling;
  /** Starting values for a new tool. */
  newTool: () => Promise<Tool>;
  /** The name and side view a tool would get (from the engine). */
  preview: (tool: Tool) => Promise<ToolPreview>;
  onAdd: Save;
  onEdit: Save;
  onDelete: (tool: Tool) => void;
  onClose: () => void;
}

export default function ToolListDialog({ tooling, newTool, preview, onAdd, onEdit, onDelete, onClose }: Props) {
  const [selected, setSelected] = useState<number | null>(null);
  const [form, setForm] = useState<{ tool: Tool; isNew: boolean } | null>(null);
  const current = tooling.tools.find((t) => t.id === selected) ?? null;
  const libraries = (id: number) =>
    tooling.libraries
      .filter((l) => l.tools.some((t) => t.tool_id === id))
      .map((l) => l.name)
      .join(", ");

  if (form) {
    return (
      <ToolForm
        tool={form.tool}
        isNew={form.isNew}
        preview={preview}
        onSave={async (t) => {
          const error = await (form.isNew ? onAdd(t) : onEdit(t));
          if (error === null) setForm(null);
          return error;
        }}
        onCancel={() => setForm(null)}
      />
    );
  }

  return (
    <div className="modal-backdrop" onKeyDown={(e) => e.key === "Escape" && onClose()}>
      <div className="modal tool-list-dialog" role="dialog" aria-modal="true" aria-labelledby="tool-list-title">
        <div className="points-header">
          <h3 id="tool-list-title">Master tool list</h3>
          <button type="button" className="close" title="Close" onClick={onClose}>
            ✕
          </button>
        </div>
        <p className="muted">
          Every tool you have, on this computer. Add tools to a tool library (Setup tab) to give them tool numbers.
        </p>
        <div className="points-scroll">
          {tooling.tools.length === 0 ? (
            <div className="empty">No tools yet: press Add tool.</div>
          ) : (
            <table className="points-table tool-table">
              <thead>
                <tr>
                  <th>Name</th>
                  <th>End type</th>
                  <th>Diameter</th>
                  <th>Shank</th>
                  <th>Flutes</th>
                  <th>Flute length</th>
                  <th>Overall length</th>
                  <th>In libraries</th>
                </tr>
              </thead>
              <tbody>
                {tooling.tools.map((t) => (
                  <tr
                    key={t.id}
                    className={t.id === selected ? "selected" : ""}
                    onClick={() => setSelected(t.id)}
                    onDoubleClick={() => setForm({ tool: t, isNew: false })}
                  >
                    <td>{t.name}</td>
                    <td>
                      {endTypeLabel(t.end_type)}
                      {t.end_type === "VBit" ? ` ${num(t.v_angle)}°` : ""}
                    </td>
                    <td>{num(t.diameter)}</td>
                    <td>{num(t.shank_diameter ?? t.diameter)}</td>
                    <td>
                      {t.flutes}
                      {t.end_type !== "Drill" ? ` ${t.flute_type.toLowerCase()}` : ""}
                      {usesHelix(t.flute_type, t.end_type) ? ` ${num(t.helix_angle)}°` : ""}
                    </td>
                    <td>{num(t.flute_length)}</td>
                    <td>{num(t.overall_length)}</td>
                    <td className="muted">{libraries(t.id)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
        <div className="modal-buttons">
          <button type="button" className="primary" onClick={async () => setForm({ tool: await newTool(), isNew: true })}>
            Add tool
          </button>
          <button type="button" disabled={!current} onClick={() => current && setForm({ tool: current, isNew: false })}>
            Edit
          </button>
          <button type="button" disabled={!current} onClick={() => current && onDelete(current)}>
            Delete
          </button>
          <span className="spacer" />
          <button type="button" onClick={onClose}>
            Close
          </button>
        </div>
      </div>
    </div>
  );
}

/** The Add tool / Edit tool form, with a live side view of the tool on the right. */
function ToolForm({
  tool,
  isNew,
  preview,
  onSave,
  onCancel,
}: {
  tool: Tool;
  isNew: boolean;
  preview: (tool: Tool) => Promise<ToolPreview>;
  onSave: Save;
  onCancel: () => void;
}) {
  const [name, setName] = useState(tool.name);
  const [autoName, setAutoName] = useState(tool.auto_name);
  const [endType, setEndType] = useState<EndType>(tool.end_type);
  const [fluteType, setFluteType] = useState<FluteType>(tool.flute_type);
  const helix = usesHelix(fluteType, endType);
  const [text, setText] = useState({
    diameter: num(tool.diameter),
    shank: tool.shank_diameter === null ? "" : num(tool.shank_diameter),
    flutes: String(tool.flutes),
    flute_length: num(tool.flute_length),
    overall_length: num(tool.overall_length),
    v_angle: num(tool.v_angle),
    helix_angle: num(tool.helix_angle),
  });
  const [error, setError] = useState<string | null>(null);
  const [shown, setShown] = useState<ToolPreview | null>(null);
  type Key = keyof typeof text;
  const parse = (s: string) => (s.trim() !== "" && Number.isFinite(Number(s)) ? Number(s) : NaN);
  const bad = (k: Key) => {
    if (k === "shank" && text.shank.trim() === "") return false;
    if (k === "v_angle" && endType !== "VBit") return false;
    if (k === "helix_angle" && !helix) return false;
    if (k === "flutes") return !/^\d+$/.test(text.flutes.trim());
    return Number.isNaN(parse(text[k]));
  };
  const keys: Key[] = ["diameter", "shank", "flutes", "flute_length", "overall_length", "v_angle", "helix_angle"];
  const numbersOk = !keys.some(bad);

  /** The tool as the form stands, or null while a number box isn't a number. */
  const current = (): Tool | null =>
    numbersOk
      ? {
          ...tool,
          name: name.trim(),
          auto_name: autoName,
          end_type: endType,
          diameter: parse(text.diameter),
          shank_diameter: text.shank.trim() === "" ? null : parse(text.shank),
          flutes: Number(text.flutes),
          flute_length: parse(text.flute_length),
          overall_length: parse(text.overall_length),
          v_angle: endType === "VBit" ? parse(text.v_angle) : tool.v_angle,
          flute_type: fluteType,
          helix_angle: helix ? parse(text.helix_angle) : tool.helix_angle,
        }
      : null;

  // Ask the engine for the name and picture whenever a value changes (latest answer wins).
  const formKey = JSON.stringify([name, autoName, endType, fluteType, text]);
  const latest = useRef(formKey);
  useEffect(() => {
    latest.current = formKey;
    const t = current();
    if (!t) {
      setShown((p) => (p ? { ...p, drawing: null } : p));
      return;
    }
    preview(t).then(
      (p) => latest.current === formKey && setShown(p),
      () => {},
    );
  }, [formKey]);

  const nameShown = autoName ? (shown?.name ?? name) : name;
  const ok = numbersOk && (autoName || name.trim() !== "");

  const field = (k: Key, label: string, hint?: string) => (
    <tr>
      <th>{label}</th>
      <td>
        <input
          className={`prop-input${bad(k) ? " bad" : ""}`}
          value={text[k]}
          placeholder={hint}
          spellCheck={false}
          onChange={(e) => setText((t) => ({ ...t, [k]: e.target.value }))}
        />
      </td>
    </tr>
  );

  return (
    <div className="modal-backdrop" onKeyDown={(e) => e.key === "Escape" && onCancel()}>
      <form
        className="modal tool-form"
        role="dialog"
        aria-modal="true"
        aria-labelledby="tool-form-title"
        onSubmit={async (e) => {
          e.preventDefault();
          const t = current();
          if (!ok || !t) return;
          setError(await onSave(t));
        }}
      >
        <h3 id="tool-form-title">{isNew ? "Add tool" : `Edit tool: ${tool.name}`}</h3>
        <div className="tool-form-body">
          <div className="tool-form-fields">
            <table className="props">
              <tbody>
                <tr>
                  <th>Name</th>
                  <td>
                    <input
                      className={`prop-input${ok || autoName || name.trim() !== "" ? "" : " bad"}${autoName ? " auto" : ""}`}
                      value={nameShown}
                      title={autoName ? "Made from the values below. Type to give your own name." : ""}
                      onChange={(e) => {
                        setAutoName(false);
                        setName(e.target.value);
                      }}
                    />
                  </td>
                </tr>
                <tr>
                  <th />
                  <td>
                    <label className="auto-name">
                      <input
                        type="checkbox"
                        checked={autoName}
                        onChange={(e) => {
                          if (!e.target.checked && shown) setName(shown.name);
                          setAutoName(e.target.checked);
                        }}
                      />
                      Automatic name
                    </label>
                  </td>
                </tr>
                <tr>
                  <th>End type</th>
                  <td>
                    <select className="prop-input" value={endType} onChange={(e) => setEndType(e.target.value as EndType)}>
                      {END_TYPES.map(([v, label]) => (
                        <option key={v} value={v}>
                          {label}
                        </option>
                      ))}
                    </select>
                  </td>
                </tr>
                {endType === "VBit" && field("v_angle", "V angle (°)")}
                {field("diameter", "Diameter (mm)")}
                {field("shank", "Shank diameter (mm)", "same as diameter")}
                {field("flutes", "Flutes")}
                {endType !== "Drill" && (
                <tr>
                  <th>Flute type</th>
                  <td>
                    <select
                      className="prop-input"
                      value={fluteType}
                      onChange={(e) => setFluteType(e.target.value as FluteType)}
                    >
                      {FLUTE_TYPES.map(([v, label]) => (
                        <option key={v} value={v}>
                          {label}
                        </option>
                      ))}
                    </select>
                  </td>
                </tr>
                )}
                {helix && field("helix_angle", "Helix angle (°)")}
                {field("flute_length", "Flute length (mm)")}
                {field("overall_length", "Overall length (mm)")}
              </tbody>
            </table>
            {endType !== "Flat" && (
              <p className="muted tool-note">
                v1 toolpaths are for end mills only: this tool can be kept in the list and libraries, but not
                picked for an operation yet.
              </p>
            )}
            {error && <p className="prop-error">{error}</p>}
          </div>
          <ToolPicture drawing={shown?.drawing ?? null} tool={current()} />
        </div>
        <div className="modal-buttons">
          <button type="submit" className="primary" disabled={!ok}>
            OK
          </button>
          <button type="button" onClick={onCancel}>
            Cancel
          </button>
        </div>
      </form>
    </div>
  );
}

/** Size of the picture box in pixels. */
const PIC_W = 220;
const PIC_H = 330;

/**
 * The side view from the engine, scaled to fit, with the diameters and lengths marked.
 * Only display scaling here: the shape itself comes from the engine.
 */
function ToolPicture({ drawing, tool }: { drawing: ToolDrawing | null; tool: Tool | null }) {
  if (!drawing || !tool) {
    return (
      <div className="tool-picture empty-picture">
        <span className="muted">Enter valid values to see the tool.</span>
      </div>
    );
  }
  const halfWidth = Math.max(...drawing.outline.map((p) => Math.abs(p.x)));
  const length = Math.max(...drawing.outline.map((p) => p.y));
  // The tool is centred, with room on both sides for the length marks on the right, and
  // above and below for the diameter labels.
  const side = 62;
  const top = 22;
  const bottom = 24;
  const k = Math.min((PIC_H - top - bottom) / length, (PIC_W - 2 * side) / (2 * halfWidth));
  const cx = PIC_W / 2;
  const base = PIC_H - bottom;
  const X = (x: number) => cx + x * k;
  const Y = (y: number) => base - y * k;
  const path = drawing.outline.map((p, i) => `${i === 0 ? "M" : "L"}${X(p.x).toFixed(2)},${Y(p.y).toFixed(2)}`).join(" ") + " Z";
  const edge = X(halfWidth);
  const flute = Y(drawing.flute_top);
  const shank = tool.shank_diameter ?? tool.diameter;
  // A vertical length mark from the tip to `y`, at `x`, labelled.
  const mark = (x: number, yTop: number, label: string) => (
    <g className="dim">
      <line x1={x} y1={base} x2={x} y2={yTop} />
      <line x1={x - 4} y1={base} x2={x + 4} y2={base} />
      <line x1={x - 4} y1={yTop} x2={x + 4} y2={yTop} />
      <text x={x + 5} y={(base + yTop) / 2} dominantBaseline="middle">
        {label}
      </text>
    </g>
  );
  return (
    <svg className="tool-picture" width={PIC_W} height={PIC_H} viewBox={`0 0 ${PIC_W} ${PIC_H}`}>
      <path className="tool-body" d={path} />
      {drawing.flute_lines.map((line, i) => (
        <polyline
          key={i}
          className="flute-line"
          points={line.map((p) => `${X(p.x).toFixed(2)},${Y(p.y).toFixed(2)}`).join(" ")}
        />
      ))}
      <line
        className="flute-top"
        x1={X(-drawing.flute_top_radius)}
        y1={flute}
        x2={X(drawing.flute_top_radius)}
        y2={flute}
      />
      {mark(edge + 10, flute, num(tool.flute_length))}
      {mark(edge + 34, Y(length), num(tool.overall_length))}
      <text className="dim-text" x={cx} y={base + 16} textAnchor="middle">
        Ø{num(tool.diameter)}
        {tool.end_type === "VBit" ? ` ${num(tool.v_angle)}°` : ""}
      </text>
      <text className="dim-text" x={cx} y={Y(length) - 8} textAnchor="middle">
        Ø{num(shank)}
      </text>
    </svg>
  );
}
