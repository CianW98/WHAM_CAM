const GROUPS: string[][] = [
  ["New", "Open", "Save", "Import"],
  ["Undo", "Redo", "Delete"],
  ["Line", "Arc", "Circle", "Rectangle", "Polyline", "Point"],
  ["Profile", "Pocket", "Drill"],
  ["Generate toolpaths", "Show toolpaths", "Produce G-code"],
  ["Zoom to fit"],
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
      {GROUPS.map((group, i) => (
        <div className="tool-group" key={i}>
          {group.map((name) => (
            <button
              key={name}
              className={active === name || on.has(name) ? "active" : ""}
              disabled={disabled.has(name)}
              onClick={() => onAction(name)}
            >
              {name}
            </button>
          ))}
        </div>
      ))}
    </header>
  );
}
