import type { ReactNode } from "react";
import type { Document } from "../engine";
import type { Selection } from "../selection";

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
  onClick?: () => void;
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
  const is = (s: Selection) => JSON.stringify(s) === JSON.stringify(selection);

  return (
    <ul className="tree">
      <Item label={doc.name} selected={is({ kind: "project" })} onClick={() => onSelect({ kind: "project" })}>
        <Item label="Layers">
          {doc.layers.map((layer, li) => (
            <Item
              key={layer.name}
              label={layer.name}
              selected={is({ kind: "layer", layer: li })}
              onClick={() => onSelect({ kind: "layer", layer: li })}
            >
              {layer.objects.map((obj) => (
                <Item
                  key={obj.id}
                  label={`${obj.shape.type} (${obj.id})`}
                  selected={is({ kind: "object", layer: li, id: obj.id })}
                  onClick={() => onSelect({ kind: "object", layer: li, id: obj.id })}
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
