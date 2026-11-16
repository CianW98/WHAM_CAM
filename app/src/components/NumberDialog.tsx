import { useState } from "react";

export interface NumberQuestion {
  title: string;
  fields: { label: string; value: number }[];
}

/** A small modal asking for one or more numbers. Enter / OK answers, Esc / Cancel answers null. */
export default function NumberDialog({
  question,
  onAnswer,
}: {
  question: NumberQuestion;
  onAnswer: (values: number[] | null) => void;
}) {
  const [texts, setTexts] = useState(question.fields.map((f) => String(f.value)));
  const values = texts.map((t) => (t.trim() === "" ? NaN : Number(t)));
  const ok = values.every(Number.isFinite);

  return (
    <div className="modal-backdrop" onKeyDown={(e) => e.key === "Escape" && onAnswer(null)}>
      <form
        className="modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="number-title"
        onSubmit={(e) => {
          e.preventDefault();
          if (ok) onAnswer(values);
        }}
      >
        <h3 id="number-title">{question.title}</h3>
        <table className="number-fields">
          <tbody>
            {question.fields.map((f, i) => (
              <tr key={f.label}>
                <th>
                  <label htmlFor={`number-${i}`}>{f.label}</label>
                </th>
                <td>
                  <input
                    id={`number-${i}`}
                    className={`prop-input num${Number.isFinite(values[i]) ? "" : " bad"}`}
                    value={texts[i]}
                    autoFocus={i === 0}
                    onFocus={(e) => e.currentTarget.select()}
                    spellCheck={false}
                    onChange={(e) => setTexts((t) => t.map((v, j) => (j === i ? e.target.value : v)))}
                  />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        <div className="modal-buttons">
          <button type="submit" className="primary" disabled={!ok}>
            OK
          </button>
          <button type="button" onClick={() => onAnswer(null)}>
            Cancel
          </button>
        </div>
      </form>
    </div>
  );
}
