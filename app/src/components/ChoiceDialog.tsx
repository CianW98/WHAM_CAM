export interface Choice {
  title: string;
  message: string;
  options: string[];
}

/** A small modal question with one button per option. Esc / Cancel answers null. */
export default function ChoiceDialog({ choice, onAnswer }: { choice: Choice; onAnswer: (a: string | null) => void }) {
  return (
    <div className="modal-backdrop" onKeyDown={(e) => e.key === "Escape" && onAnswer(null)}>
      <div className="modal" role="dialog" aria-modal="true" aria-labelledby="choice-title">
        <h3 id="choice-title">{choice.title}</h3>
        <p>{choice.message}</p>
        <div className="modal-buttons">
          {choice.options.map((o, i) => (
            <button key={o} className={i === 0 ? "primary" : ""} autoFocus={i === 0} onClick={() => onAnswer(o)}>
              {o}
            </button>
          ))}
          <button onClick={() => onAnswer(null)}>Cancel</button>
        </div>
      </div>
    </div>
  );
}
