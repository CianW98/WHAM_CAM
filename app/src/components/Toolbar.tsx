const GROUPS: string[][] = [
  ["New", "Open", "Save", "Import"],
  ["Line", "Arc", "Circle", "Rectangle", "Polyline", "Point"],
  ["Profile", "Pocket", "Drill"],
  ["Generate toolpaths", "Produce G-code"],
];

export default function Toolbar({ onAction }: { onAction: (name: string) => void }) {
  return (
    <header className="toolbar">
      <span className="brand">WHAM_CAM</span>
      {GROUPS.map((group, i) => (
        <div className="tool-group" key={i}>
          {group.map((name) => (
            <button key={name} onClick={() => onAction(name)}>
              {name}
            </button>
          ))}
        </div>
      ))}
    </header>
  );
}
