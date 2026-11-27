import { useEffect, useRef, useState, type ReactNode } from "react";
import type { DimensionInfo, Document, Layer, OperationState, Shape } from "../engine";
import { selectedIds, selectObjects, type Selection } from "../selection";
import ColorSwatch from "./ColorSwatch";
import ContextMenu, { type MenuItem } from "./ContextMenu";
import { EyeIcon } from "./Icons";

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
  /** Layers hidden from view. */
  hiddenLayers: Set<string>;
  onSetLayerVisible: (name: string, visible: boolean) => void;
  /** The dimensions as they are now, and whether they are shown. */
  dimensions: DimensionInfo[];
  dimensionsVisible: boolean;
  onSetDimensionsVisible: (visible: boolean) => void;
  /** Name of the layer new drawings go onto. */
  currentLayer: string | null;
  /** Adds a new empty layer (selected and made current); resolves to its name. */
  onAddLayer: () => Promise<string>;
  onDeleteLayer: (name: string) => void;
  /** Toolpath state of each operation (for the out-of-date / error markers). */
  operationStates: OperationState[];
  /** The right-click menu for an operation. */
  operationMenu: (id: number) => MenuItem[];
  /** Resolves false if the new name was rejected. */
  onRenameOperation: (id: number, name: string) => Promise<boolean>;
  /** The part new operations go into (highlighted like the current layer). */
  currentPart: string | null;
  /** Adds a part (selected and made current); resolves to its index. */
  onNewPart: () => Promise<number>;
  /** Resolves false if the new name was rejected. */
  onRenamePart: (part: number, name: string) => Promise<boolean>;
  /** Deletes a part and its operations. */
  onDeletePart: (part: number) => void;
}

type ContextEvent = React.MouseEvent<HTMLElement>;

function Item({
  label,
  selected,
  onClick,
  onContextMenu,
  children,
}: {
  label: ReactNode;
  selected?: boolean;
  onClick?: (e: React.MouseEvent) => void;
  onContextMenu?: (e: ContextEvent) => void;
  children?: ReactNode;
}) {
  return (
    <li>
      <div className={`tree-item${selected ? " selected" : ""}`} onClick={onClick} onContextMenu={onContextMenu}>
        {label}
      </div>
      {children && <ul>{children}</ul>}
    </li>
  );
}

/**
 * An operation or part in the tree. Click: select. Double-click (or Rename in its menu): rename
 * in place (Enter or clicking away saves, Esc cancels). A part shows its operations below.
 */
function OperationItem({
  name,
  enabled,
  marker,
  selected,
  onClick,
  onContextMenu,
  onRename,
  renameNow,
  onRenameStarted,
  children,
  current = false,
}: {
  children?: ReactNode;
  /** The current part: highlighted like the current layer. */
  current?: boolean;
  name: string;
  enabled: boolean;
  marker: ReactNode;
  selected: boolean;
  onClick: () => void;
  onContextMenu: (e: ContextEvent) => void;
  onRename: (name: string) => Promise<boolean>;
  renameNow: boolean;
  onRenameStarted: () => void;
}) {
  const [editing, setEditing] = useState<string | null>(null);
  // Enter/Esc close the box, which can also fire a blur: only the first finish counts.
  const finished = useRef(true);
  const start = () => {
    finished.current = false;
    setEditing(name);
  };
  const finish = async (save: boolean) => {
    if (finished.current || editing === null) return;
    finished.current = true;
    const newName = editing.trim();
    setEditing(null);
    if (save && newName !== "" && newName !== name) await onRename(newName);
  };
  useEffect(() => {
    if (!renameNow) return;
    start();
    onRenameStarted();
  }, [renameNow]);

  return (
    <li>
      <div
        className={`tree-item${selected ? " selected" : ""}${current ? " current-layer" : ""}`}
        onClick={onClick}
        onDoubleClick={start}
        onContextMenu={onContextMenu}
      >
        {editing === null ? (
          <span className={enabled ? "" : "muted"}>
            {name}
            {marker}
          </span>
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
  hidden,
  onToggleVisible,
  onClick,
  onRename,
  onColor,
  onContextMenu,
  renameNow,
  onRenameStarted,
  children,
}: {
  layer: Layer;
  open: boolean;
  selected: boolean;
  current: boolean;
  /** Hidden from view. */
  hidden: boolean;
  onToggleVisible: () => void;
  onClick: () => void;
  onRename: (newName: string) => Promise<boolean>;
  onColor: (color: string) => void;
  onContextMenu: (e: ContextEvent) => void;
  /** Start renaming in place (e.g. a layer just added from the menu). */
  renameNow: boolean;
  onRenameStarted: () => void;
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

  useEffect(() => {
    if (!renameNow) return;
    start();
    onRenameStarted();
  }, [renameNow]);

  return (
    <li>
      <div
        className={`tree-item${selected ? " selected" : ""}${current ? " current-layer" : ""}${hidden ? " hidden-layer" : ""}`}
        onClick={onClick}
        onDoubleClick={start}
        onContextMenu={onContextMenu}
      >
        <span className="chevron">{open ? "▾" : "▸"}</span>
        <button
          type="button"
          className="layer-eye"
          disabled={current}
          title={
            current
              ? "The current layer can't be hidden"
              : hidden
                ? `Show layer ${layer.name}`
                : `Hide layer ${layer.name}`
          }
          onClick={(e) => {
            e.stopPropagation();
            onToggleVisible();
          }}
          onDoubleClick={(e) => e.stopPropagation()}
        >
          <EyeIcon shown={!hidden} />
        </button>
        <ColorSwatch color={layer.color} onPick={onColor} title={`Colour of layer ${layer.name}`} />
        {editing === null ? (
          `${layer.name} (${layer.objects.length})`
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

/** The Dimensions heading (with one eye icon for all of them) and the dimensions under it. */
function DimensionsItem({
  dimensions,
  visible,
  onToggleVisible,
  isSelected,
  onSelect,
}: {
  dimensions: DimensionInfo[];
  visible: boolean;
  onToggleVisible: () => void;
  isSelected: (id: number) => boolean;
  onSelect: (id: number, add: boolean) => void;
}) {
  const [open, setOpen] = useState(true);
  return (
    <li>
      <div className={`tree-item${visible ? "" : " hidden-layer"}`} onClick={() => setOpen((o) => !o)}>
        <span className="chevron">{open ? "▾" : "▸"}</span>
        <button
          type="button"
          className="layer-eye"
          title={visible ? "Hide dimensions" : "Show dimensions"}
          onClick={(e) => {
            e.stopPropagation();
            onToggleVisible();
          }}
        >
          <EyeIcon shown={visible} />
        </button>
        {`Dimensions (${dimensions.length})`}
      </div>
      {open && dimensions.length > 0 && (
        <ul>
          {dimensions.map((d) => (
            <Item
              key={d.id}
              label={`${d.kind} dimension (${d.id}): ${d.text}`}
              selected={isSelected(d.id)}
              onClick={(e) => onSelect(d.id, e.shiftKey || e.ctrlKey)}
            />
          ))}
        </ul>
      )}
    </li>
  );
}

export default function Tree({
  doc,
  selection,
  onSelect,
  onRenameLayer,
  onSetLayerColor,
  hiddenLayers,
  onSetLayerVisible,
  dimensions,
  dimensionsVisible,
  onSetDimensionsVisible,
  currentLayer,
  onAddLayer,
  onDeleteLayer,
  operationStates,
  operationMenu,
  onRenameOperation,
  currentPart,
  onNewPart,
  onRenamePart,
  onDeletePart,
}: Props) {
  // The right-click menu: on a layer (its name), the Layers heading (null), an operation,
  // Machining, or a part (its index).
  const [menu, setMenu] = useState<
    | { x: number; y: number; layer: string | null }
    | { x: number; y: number; operation: number }
    | { x: number; y: number; machining: true }
    | { x: number; y: number; part: number }
    | null
  >(null);
  // A part to start renaming in place (its index).
  const [renamingPart, setRenamingPart] = useState<number | null>(null);
  // A layer to start renaming in place.
  const [renaming, setRenaming] = useState<string | null>(null);
  // An operation to start renaming in place (from its menu).
  const [renamingOp, setRenamingOp] = useState<number | null>(null);
  // Esc closes the menu (in the capture phase, before the app's own Esc handling).
  useEffect(() => {
    if (!menu) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.stopPropagation();
      setMenu(null);
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [menu]);
  // Which layers are closed (by name). Display-only state; all layers start open.
  const [closed, setClosed] = useState<Set<string>>(new Set());
  // Keep a single selected shape in view (e.g. when stepping with the arrow keys).
  const single = selection.kind === "objects" && selection.ids.length === 1 ? selection.ids[0] : null;
  useEffect(() => {
    if (single !== null) document.querySelector(".tree-panel .tree-item.selected")?.scrollIntoView({ block: "nearest" });
  }, [single]);
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

  const openMenu = (e: ContextEvent, layer: string | null) => {
    e.preventDefault();
    e.stopPropagation();
    setMenu({ x: e.clientX, y: e.clientY, layer });
  };

  const newLayer = async () => {
    const name = await onAddLayer();
    setRenaming(name);
  };

  const menuItems = (layer: string | null): MenuItem[] => {
    const items: MenuItem[] = [{ label: "New layer", action: () => void newLayer() }];
    if (layer !== null) {
      items.push(
        { label: "Rename", action: () => setRenaming(layer) },
        { label: "Delete layer", separatorBefore: true, action: () => onDeleteLayer(layer) },
      );
    }
    return items;
  };

  const newPart = async () => {
    setRenamingPart(await onNewPart());
  };

  const machiningMenu = (part: number | null): MenuItem[] => {
    const items: MenuItem[] = [{ label: "New part", action: () => void newPart() }];
    if (part !== null) {
      items.push(
        { label: "Rename", action: () => setRenamingPart(part) },
        { label: "Delete part", separatorBefore: true, action: () => onDeletePart(part) },
      );
    }
    return items;
  };

  const openMachiningMenu = (e: ContextEvent, part: number | null) => {
    e.preventDefault();
    e.stopPropagation();
    setMenu(part === null ? { x: e.clientX, y: e.clientY, machining: true } : { x: e.clientX, y: e.clientY, part });
  };

  const openOperationMenu = (e: ContextEvent, id: number) => {
    e.preventDefault();
    e.stopPropagation();
    setMenu({ x: e.clientX, y: e.clientY, operation: id });
  };

  // Marker after an operation's name: out of date, or an error (with the message as a tooltip).
  const opMarker = (id: number) => {
    const st = operationStates.find((o) => o.id === id);
    if (st?.status === "error")
      return (
        <span className="op-marker error" title={st.message ?? "Error"}>
          !
        </span>
      );
    if (st?.status === "outdated")
      return (
        <span className="op-marker" title="Out of date: press Generate toolpaths">
          ⟳
        </span>
      );
    return null;
  };

  return (
    <>
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
          onContextMenu={(e) => openMenu(e, null)}
        >
          {doc.layers.map((layer, li) => (
            <LayerItem
              key={layer.name}
              layer={layer}
              open={!closed.has(layer.name)}
              selected={is({ kind: "layer", layer: li })}
              current={layer.name === currentLayer}
              hidden={hiddenLayers.has(layer.name)}
              onToggleVisible={() => onSetLayerVisible(layer.name, hiddenLayers.has(layer.name))}
              onClick={() => {
                onSelect({ kind: "layer", layer: li });
                toggle(layer.name);
              }}
              onRename={(newName) => rename(layer.name, newName)}
              onColor={(color) => onSetLayerColor(layer.name, color)}
              onContextMenu={(e) => openMenu(e, layer.name)}
              renameNow={renaming === layer.name}
              onRenameStarted={() => setRenaming(null)}
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
        <DimensionsItem
          dimensions={dimensions}
          visible={dimensionsVisible}
          onToggleVisible={() => onSetDimensionsVisible(!dimensionsVisible)}
          isSelected={(id) => ids.has(id)}
          onSelect={(id, add) => onSelect(selectObjects(selection, [id], add))}
        />
        <Item
          label="Machining"
          selected={is({ kind: "machining" })}
          onClick={() => onSelect({ kind: "machining" })}
          onContextMenu={(e) => openMachiningMenu(e, null)}
        >
          {doc.parts.map((part, pi) => (
            <OperationItem
              key={part.name}
              name={part.name}
              enabled
              marker={null}
              selected={is({ kind: "part", part: pi })}
              current={part.name === currentPart}
              onClick={() => onSelect({ kind: "part", part: pi })}
              onContextMenu={(e) => openMachiningMenu(e, pi)}
              onRename={(name) => onRenamePart(pi, name)}
              renameNow={renamingPart === pi}
              onRenameStarted={() => setRenamingPart(null)}
            >
              {part.operations.length > 0 &&
                part.operations.map((op) => (
                  <OperationItem
                    key={op.id}
                    name={op.name}
                    enabled={op.enabled}
                    marker={opMarker(op.id)}
                    selected={is({ kind: "operation", id: op.id })}
                    onClick={() => onSelect({ kind: "operation", id: op.id })}
                    onContextMenu={(e) => openOperationMenu(e, op.id)}
                    onRename={(name) => onRenameOperation(op.id, name)}
                    renameNow={renamingOp === op.id}
                    onRenameStarted={() => setRenamingOp(null)}
                  />
                ))}
            </OperationItem>
          ))}
        </Item>
      </Item>
    </ul>
    {menu && (
      <ContextMenu
        items={
          "operation" in menu
            ? [
                { label: "Rename", action: () => setRenamingOp(menu.operation) },
                ...operationMenu(menu.operation).map((item, i) => (i === 0 ? { ...item, separatorBefore: true } : item)),
              ]
            : "machining" in menu
              ? machiningMenu(null)
              : "part" in menu
                ? machiningMenu(menu.part)
                : menuItems(menu.layer)
        }
        x={menu.x}
        y={menu.y}
        onClose={() => setMenu(null)}
      />
    )}
    </>
  );
}
