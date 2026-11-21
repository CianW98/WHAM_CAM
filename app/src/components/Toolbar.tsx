import { ActionIcon } from "./Icons";

/** The toolbar's groups of actions, each shown under its heading. */
const GROUPS: { heading: string; actions: string[] }[] = [
  { heading: "File", actions: ["New", "Open", "Save", "Import"] },
  { heading: "Edit", actions: ["Undo", "Redo", "Delete"] },
  { heading: "Draw", actions: ["Line", "Arc", "Circle", "Rectangle", "Polyline", "Point", "Dimension"] },
  { heading: "Machining", actions: ["Profile", "Pocket", "Drill"] },
  { heading: "Toolpaths", actions: ["Generate toolpaths", "Show toolpaths", "Show cut width", "Produce G-code"] },
];

interface Props {
  onAction: (name: string) => void;
  /** Buttons that are currently unavailable (e.g. Undo with nothing to undo). */
  disabled: Set<string>;
  /** The button shown pressed (the active drawing tool), if any. */
  active: string | null;
  /** On/off buttons that are currently on (e.g. Show toolpaths). */
  on: Set<string>;
}

export default function Toolbar({ onAction, disabled, active, on }: Props) {
  return (
    <header className="toolbar">
      <span className="brand">WHAM_CAM</span>
      {GROUPS.map(({ heading, actions }) => (
        <div className="tool-group" key={heading}>
          <div className="tool-group-heading">{heading}</div>
          <div className="tool-group-buttons">
            {actions.map((name) => (
              <button
                key={name}
                aria-label={name}
                className={`tool-button${active === name || on.has(name) ? " active" : ""}`}
                disabled={disabled.has(name)}
                onClick={() => onAction(name)}
              >
                <ActionIcon name={name} />
              </button>
            ))}
          </div>
        </div>
      ))}
    </header>
  );
}
