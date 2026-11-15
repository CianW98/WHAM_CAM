import type { ReactNode } from "react";
import type { Document, Shape } from "../engine";
import { selectedIds, selectObjects, type Selection } from "../selection";

/** Tree label for a shape type, using the reference program's names. */
export function shapeLabel(shape: Shape): string {
  return shape.type === "Points" ? "PointList" : shape.type;
}

interface Props {
  doc: Document | null;
  selection: Selection;
  onSelect: (sel: Selection) => void;
}

function Item({
  label,
  selected,
  onClick,
  children,
}: {
  label: string;
  selected?: boolean;
  onClick?: (e: React.MouseEvent) => void;
  children?: ReactNode;
}) {
  return (
    <li>
      <div className={`tree-item${selected ? " selected" : ""}`} onClick={onClick}>
        {label}
      </div>
      {children && <ul>{children}</ul>}
    </li>
  );
}

export default function Tree({ doc, selection, onSelect }: Props) {
  if (!doc) return <div className="empty">No project loaded.</div>;
  const ids = new Set(selectedIds(selection));
  const is = (s: Selection) => JSON.stringify(s) === JSON.stringify(selection);

  return (
    <ul className="tree">
      <Item label={doc.name} selected={is({ kind: "project" })} onClick={() => onSelect({ kind: "project" })}>
        <Item label="Layers">
          {doc.layers.map((layer, li) => (
            <Item
              key={layer.name}
              label={`${layer.name} (${layer.objects.length})`}
              selected={is({ kind: "layer", layer: li })}
              onClick={() => onSelect({ kind: "layer", layer: li })}
            >
              {layer.objects.length > 0 &&
                layer.objects.map((obj) => (
                  <Item
                    key={obj.id}
                    label={`${shapeLabel(obj.shape)} (${obj.id})`}
                    selected={ids.has(obj.id)}
                    onClick={(e) => onSelect(selectObjects(selection, [obj.id], e.shiftKey || e.ctrlKey))}
                  />
                ))}
            </Item>
          ))}
        </Item>
        <Item label="Machining">
          {doc.parts.map((part, pi) => (
            <Item
              key={part.name}
              label={part.name}
              selected={is({ kind: "part", part: pi })}
              onClick={() => onSelect({ kind: "part", part: pi })}
            />
          ))}
        </Item>
      </Item>
    </ul>
  );
}
