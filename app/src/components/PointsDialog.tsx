import { useState } from "react";

/** One row of the table: X, Y and (for polylines) the bulge, as typed. */
type Row = { x: string; y: string; bulge: string };

export interface PointsEdit {
  title: string;
  /** Polylines have a bulge column (0 = straight segment to the next point). */
  withBulge: boolean;
  points: { x: number; y: number; bulge?: number }[];
}

const text = (n: number) => String(Number(n.toFixed(6)));
const parse = (t: string) => (t.trim() === "" ? NaN : Number(t));

/**
 * The points of a polyline or point list in a pop-up table, so they don't flood the
 * properties panel. Edit cells, add or remove points, then OK saves them as one change.
 */
export default function PointsDialog({
  edit,
  onSave,
  onClose,
}: {
  edit: PointsEdit;
  /** Resolves false if the engine refused the points (the dialog then stays open). */
  onSave: (points: { x: number; y: number; bulge: number }[]) => Promise<boolean>;
  onClose: () => void;
}) {
  const [rows, setRows] = useState<Row[]>(
    edit.points.map((p) => ({ x: text(p.x), y: text(p.y), bulge: text(p.bulge ?? 0) })),
  );
  const values = rows.map((r) => ({ x: parse(r.x), y: parse(r.y), bulge: edit.withBulge ? parse(r.bulge) : 0 }));
  const ok = rows.length > 0 && values.every((v) => Number.isFinite(v.x) && Number.isFinite(v.y) && Number.isFinite(v.bulge));
  const columns: (keyof Row)[] = edit.withBulge ? ["x", "y", "bulge"] : ["x", "y"];

  const set = (i: number, key: keyof Row, value: string) =>
    setRows((rs) => rs.map((r, j) => (j === i ? { ...r, [key]: value } : r)));

  return (
    <div className="modal-backdrop" onKeyDown={(e) => e.key === "Escape" && onClose()}>
      <form
        className="modal points-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="points-title"
        onSubmit={async (e) => {
          e.preventDefault();
          if (ok && (await onSave(values))) onClose();
        }}
      >
        <div className="points-header">
          <h3 id="points-title">{edit.title}</h3>
          <button type="button" className="close" title="Close without saving" onClick={onClose}>
            ✕
          </button>
        </div>
        <div className="points-scroll">
          <table className="points-table">
            <thead>
              <tr>
                <th />
                <th>X</th>
                <th>Y</th>
                {edit.withBulge && <th title="0 = straight; otherwise tan(arc angle / 4), + = anticlockwise">Bulge</th>}
                <th />
              </tr>
            </thead>
            <tbody>
              {rows.map((r, i) => (
                <tr key={i}>
                  <td className="muted">{i + 1}</td>
                  {columns.map((key) => (
                    <td key={key}>
                      <input
                        className={`cell${Number.isFinite(parse(r[key])) ? "" : " bad"}`}
                        value={r[key]}
                        autoFocus={i === 0 && key === "x"}
                        spellCheck={false}
                        onFocus={(e) => e.currentTarget.select()}
                        onChange={(e) => set(i, key, e.target.value)}
                      />
                    </td>
                  ))}
                  <td>
                    <button
                      type="button"
                      className="remove"
                      title="Remove this point"
                      onClick={() => setRows((rs) => rs.filter((_, j) => j !== i))}
                    >
                      ✕
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <div className="modal-buttons">
          <button
            type="button"
            className="add"
            onClick={() => {
              const last = rows[rows.length - 1];
              setRows((rs) => [...rs, { x: last?.x ?? "0", y: last?.y ?? "0", bulge: "0" }]);
            }}
          >
            Add point
          </button>
          <span className="spacer" />
          <button type="submit" className="primary" disabled={!ok}>
            OK
          </button>
          <button type="button" onClick={onClose}>
            Cancel
          </button>
        </div>
      </form>
    </div>
  );
}
