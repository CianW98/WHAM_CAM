// The Master tool list window: every physical tool on this computer, with Add / Edit / Delete.
// The engine checks and stores the values; this only shows them and collects the form.

import { useState } from "react";
import type { EndType, Tool, Tooling } from "../engine";

export const END_TYPES: [EndType, string][] = [
  ["Flat", "Flat end mill"],
  ["Ball", "Ball nose"],
  ["VBit", "V-bit"],
  ["Drill", "Drill"],
];

export const endTypeLabel = (t: EndType) => END_TYPES.find(([v]) => v === t)?.[1] ?? t;

const num = (n: number) => String(Number(n.toFixed(4)));

/** Resolves to an error message, or null if it was saved. */
type Save = (tool: Tool) => Promise<string | null>;

interface Props {
  tooling: Tooling;
  /** Starting values for a new tool. */
  newTool: () => Promise<Tool>;
  onAdd: Save;
  onEdit: Save;
  onDelete: (tool: Tool) => void;
  onClose: () => void;
}

export default function ToolListDialog({ tooling, newTool, onAdd, onEdit, onDelete, onClose }: Props) {
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
                    <td>{t.flutes}</td>
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

/** The Add tool / Edit tool form. Number boxes are kept as text until OK. */
function ToolForm({
  tool,
  isNew,
  onSave,
  onCancel,
}: {
  tool: Tool;
  isNew: boolean;
  onSave: Save;
  onCancel: () => void;
}) {
  const [name, setName] = useState(tool.name);
  const [endType, setEndType] = useState<EndType>(tool.end_type);
  const [text, setText] = useState({
    diameter: num(tool.diameter),
    shank: tool.shank_diameter === null ? "" : num(tool.shank_diameter),
    flutes: String(tool.flutes),
    flute_length: num(tool.flute_length),
    overall_length: num(tool.overall_length),
    v_angle: num(tool.v_angle),
  });
  const [error, setError] = useState<string | null>(null);
  type Key = keyof typeof text;
  const parse = (s: string) => (s.trim() !== "" && Number.isFinite(Number(s)) ? Number(s) : NaN);
  const bad = (k: Key) => {
    if (k === "shank" && text.shank.trim() === "") return false;
    if (k === "v_angle" && endType !== "VBit") return false;
    if (k === "flutes") return !/^\d+$/.test(text.flutes.trim());
    return Number.isNaN(parse(text[k]));
  };
  const keys: Key[] = ["diameter", "shank", "flutes", "flute_length", "overall_length", "v_angle"];
  const ok = name.trim() !== "" && !keys.some(bad);

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
          if (!ok) return;
          setError(
            await onSave({
              ...tool,
              name: name.trim(),
              end_type: endType,
              diameter: parse(text.diameter),
              shank_diameter: text.shank.trim() === "" ? null : parse(text.shank),
              flutes: Number(text.flutes),
              flute_length: parse(text.flute_length),
              overall_length: parse(text.overall_length),
              v_angle: endType === "VBit" ? parse(text.v_angle) : tool.v_angle,
            }),
          );
        }}
      >
        <h3 id="tool-form-title">{isNew ? "Add tool" : `Edit tool: ${tool.name}`}</h3>
        <table className="props">
          <tbody>
            <tr>
              <th>Name</th>
              <td>
                <input
                  className={`prop-input${name.trim() === "" ? " bad" : ""}`}
                  value={name}
                  autoFocus
                  onFocus={(e) => e.currentTarget.select()}
                  onChange={(e) => setName(e.target.value)}
                />
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
            {field("shank", "Shank diameter (mm)", `same as diameter`)}
            {field("flutes", "Flutes")}
            {field("flute_length", "Flute length (mm)")}
            {field("overall_length", "Overall length (mm)")}
          </tbody>
        </table>
        {endType !== "Flat" && (
          <p className="muted tool-note">
            v1 toolpaths are for flat end mills only: this tool can be kept in the list and libraries, but not picked
            for an operation yet.
          </p>
        )}
        {error && <p className="prop-error">{error}</p>}
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
