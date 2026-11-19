// The Setup tab of the left panel: the Master tool list and the Tool libraries (this computer's,
// shared by every project).

import { useEffect, useRef, useState } from "react";
import type { Tooling } from "../engine";
import type { Selection } from "../selection";
import ContextMenu, { type MenuItem } from "./ContextMenu";

interface Props {
  tooling: Tooling | null;
  selection: Selection;
  onSelect: (sel: Selection) => void;
  onOpenToolList: () => void;
  /** Adds an empty library; resolves to its name. */
  onAddLibrary: () => Promise<string>;
  /** Resolves false if the new name was rejected. */
  onRenameLibrary: (oldName: string, newName: string) => Promise<boolean>;
  onDeleteLibrary: (name: string) => void;
}

/** A library row: click selects, double-click renames in place (Enter saves, Esc cancels). */
function LibraryItem({
  name,
  count,
  selected,
  onClick,
  onContextMenu,
  onRename,
  renameNow,
  onRenameStarted,
}: {
  name: string;
  count: number;
  selected: boolean;
  onClick: () => void;
  onContextMenu: (e: React.MouseEvent) => void;
  onRename: (name: string) => Promise<boolean>;
  renameNow: boolean;
  onRenameStarted: () => void;
}) {
  const [editing, setEditing] = useState<string | null>(null);
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
        className={`tree-item${selected ? " selected" : ""}`}
        onClick={onClick}
        onDoubleClick={start}
        onContextMenu={onContextMenu}
      >
        {editing === null ? (
          `${name} (${count})`
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
    </li>
  );
}

export default function SetupTree({
  tooling,
  selection,
  onSelect,
  onOpenToolList,
  onAddLibrary,
  onRenameLibrary,
  onDeleteLibrary,
}: Props) {
  const [menu, setMenu] = useState<{ x: number; y: number; library: string | null } | null>(null);
  const [renaming, setRenaming] = useState<string | null>(null);
  const [open, setOpen] = useState(true);
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
  if (!tooling) return <div className="empty">Loading…</div>;

  const openMenu = (e: React.MouseEvent, library: string | null) => {
    e.preventDefault();
    e.stopPropagation();
    setMenu({ x: e.clientX, y: e.clientY, library });
  };

  const newLibrary = async () => {
    const name = await onAddLibrary();
    setOpen(true);
    onSelect({ kind: "library", name });
    setRenaming(name);
  };

  const menuItems = (library: string | null): MenuItem[] => {
    const items: MenuItem[] = [{ label: "New library", action: () => void newLibrary() }];
    if (library !== null) {
      items.push(
        { label: "Rename", action: () => setRenaming(library) },
        { label: "Delete library", separatorBefore: true, action: () => onDeleteLibrary(library) },
      );
    }
    return items;
  };

  return (
    <>
      <ul className="tree">
        <li>
          <div
            className="tree-item"
            title="Every tool you have, on this computer"
            onClick={onOpenToolList}
          >
            Master tool list ({tooling.tools.length})
          </div>
        </li>
        <li>
          <div
            className="tree-item"
            title="Right-click for New library"
            onClick={() => setOpen((o) => !o)}
            onContextMenu={(e) => openMenu(e, null)}
          >
            <span className="chevron">{open ? "▾" : "▸"}</span>
            Tool libraries
          </div>
          {open && tooling.libraries.length > 0 && (
            <ul>
              {tooling.libraries.map((lib) => (
                <LibraryItem
                  key={lib.name}
                  name={lib.name}
                  count={lib.tools.length}
                  selected={selection.kind === "library" && selection.name === lib.name}
                  onClick={() => onSelect({ kind: "library", name: lib.name })}
                  onContextMenu={(e) => openMenu(e, lib.name)}
                  onRename={async (name) => {
                    const ok = await onRenameLibrary(lib.name, name);
                    if (ok) onSelect({ kind: "library", name: name.trim() });
                    return ok;
                  }}
                  renameNow={renaming === lib.name}
                  onRenameStarted={() => setRenaming(null)}
                />
              ))}
            </ul>
          )}
        </li>
      </ul>
      {menu && (
        <ContextMenu items={menuItems(menu.library)} x={menu.x} y={menu.y} onClose={() => setMenu(null)} />
      )}
    </>
  );
}
