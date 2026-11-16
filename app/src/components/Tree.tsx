import { useRef, useState, type ReactNode } from "react";
import type { Document, Layer, Shape } from "../engine";
import { selectedIds, selectObjects, type Selection } from "../selection";
import ColorSwatch from "./ColorSwatch";

/** Tree label for a shape type, using common CAM names. */
export function shapeLabel(shape: Shape): string {
  if (shape.type === "Points") return "PointList";
  if (shape.type === "Rectangle") return "PolyRectangle";
  return shape.type;
}

interface Props {
  doc: Document | null;
  selection: Selection;
  onSelect: (sel: Selection) => void;
  /** Resolves false if the rename was rejected. */
  onRenameLayer: (oldName: string, newName: string) => Promise<boolean>;
  onSetLayerColor: (name: string, color: string) => void;
  /** Name of the layer new drawings go onto. */
  currentLayer: string | null;
}

function Item({
  label,
  selected,
  onClick,
  children,
}: {
  label: ReactNode;
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

/**
 * A layer in the tree. Click: select and open/close. Double-click: rename in place
 * (Enter or clicking away saves, Esc cancels). Click the colour square: pick a colour.
 */
function LayerItem({
  layer,
  open,
  selected,
  current,
  onClick,
  onRename,
  onColor,
  children,
}: {
  layer: Layer;
  open: boolean;
  selected: boolean;
  current: boolean;
  onClick: () => void;
  onRename: (newName: string) => Promise<boolean>;
  onColor: (color: string) => void;
  children: ReactNode;
}) {
  const [editing, setEditing] = useState<string | null>(null);
  // Enter/Esc close the box, which can also fire a blur: only the first finish counts.
  const finished = useRef(true);

  const start = () => {
    finished.current = false;
    setEditing(layer.name);
  };
  const finish = async (save: boolean) => {
    if (finished.current || editing === null) return;
    finished.current = true;
    const name = editing;
    setEditing(null);
    if (save && name.trim() !== layer.name) await onRename(name);
  };

  return (
    <li>
      <div
        className={`tree-item${selected ? " selected" : ""}`}
        onClick={onClick}
        onDoubleClick={start}
      >
        <span className="chevron">{open ? "▾" : "▸"}</span>
        <ColorSwatch color={layer.color} onPick={onColor} title={`Colour of layer ${layer.name}`} />
        {editing === null ? (
          <>
            {`${layer.name} (${layer.objects.length})`}
            {current && (
              <span className="current-layer" title="Current layer: new drawings go here">
                ✎
              </span>
            )}
          </>
        ) : (
          <input
            className="tree-rename"
            value={editing}
            autoFocus
            onFocus={(e) => e.currentTarget.select()}
            onChange={(e) => setEditing(e.target.value)}
            onClick={(e) => e.stopPropagation()}
            onDoubleClick={(e) => e.stopPropagation()}
            onBlur={() => finish(true)}
            onKeyDown={(e) => {
              if (e.key === "Enter") finish(true);
              if (e.key === "Escape") finish(false);
            }}
          />
        )}
      </div>
      {open && layer.objects.length > 0 && <ul>{children}</ul>}
    </li>
  );
}

export default function Tree({ doc, selection, onSelect, onRenameLayer, onSetLayerColor, currentLayer }: Props) {
  // Which layers are closed (by name). Display-only state; all layers start open.
  const [closed, setClosed] = useState<Set<string>>(new Set());
  if (!doc) return <div className="empty">No project loaded.</div>;
  const ids = new Set(selectedIds(selection));
  const is = (s: Selection) => JSON.stringify(s) === JSON.stringify(selection);

  const toggle = (name: string) =>
    setClosed((c) => {
      const next = new Set(c);
      if (next.has(name)) next.delete(name);
      else next.add(name);
      return next;
    });

  // Layers row: close every layer if any is open, otherwise open them all.
  const anyOpen = doc.layers.some((l) => !closed.has(l.name));
  const toggleAll = () => setClosed(anyOpen ? new Set(doc.layers.map((l) => l.name)) : new Set());

  const rename = async (oldName: string, newName: string) => {
    const ok = await onRenameLayer(oldName, newName);
    // Keep a renamed layer open/closed as it was.
    if (ok && closed.has(oldName)) {
      setClosed((c) => {
        const next = new Set(c);
        next.delete(oldName);
        next.add(newName.trim());
        return next;
      });
    }
    return ok;
  };

  return (
    <ul className="tree">
      <Item label={doc.name} selected={is({ kind: "project" })} onClick={() => onSelect({ kind: "project" })}>
        <Item
          label={
            <span title={anyOpen ? "Close all layers" : "Open all layers"}>
              <span className="chevron">{anyOpen ? "▾" : "▸"}</span>
              Layers
            </span>
          }
          onClick={toggleAll}
        >
          {doc.layers.map((layer, li) => (
            <LayerItem
              key={layer.name}
              layer={layer}
              open={!closed.has(layer.name)}
              selected={is({ kind: "layer", layer: li })}
              current={layer.name === currentLayer}
              onClick={() => {
                onSelect({ kind: "layer", layer: li });
                toggle(layer.name);
              }}
              onRename={(newName) => rename(layer.name, newName)}
              onColor={(color) => onSetLayerColor(layer.name, color)}
            >
              {layer.objects.map((obj) => (
                <Item
                  key={obj.id}
                  label={`${shapeLabel(obj.shape)} (${obj.id})`}
                  selected={ids.has(obj.id)}
                  onClick={(e) => onSelect(selectObjects(selection, [obj.id], e.shiftKey || e.ctrlKey))}
                />
              ))}
            </LayerItem>
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
