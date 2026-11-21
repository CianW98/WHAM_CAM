// The Setup tab of the left panel: the Master tool list, the Tool libraries, the Machines and the
// Project templates (this computer's, shared by every project).

import { useEffect, useRef, useState } from "react";
import type { Machines, ProjectTemplates, Tooling } from "../engine";
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
  machines: Machines | null;
  /** Adds a machine; resolves to its name. */
  onAddMachine: () => Promise<string>;
  /** Resolves false if the new name was rejected. */
  onRenameMachine: (oldName: string, newName: string) => Promise<boolean>;
  onDeleteMachine: (name: string) => void;
  templates: ProjectTemplates | null;
  /** Adds a project template; resolves to its name. */
  onAddTemplate: () => Promise<string>;
  /** Resolves false if the new name was rejected. */
  onRenameTemplate: (oldName: string, newName: string) => Promise<boolean>;
  onDeleteTemplate: (name: string) => void;
  onSetTemplateForNewProjects: (name: string) => void;
}

/** A library, machine or template row: click selects, double-click renames in place (Enter saves,
 * Esc cancels). */
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
  /** Shown after the name in brackets (e.g. a library's tool count), if given. */
  count?: number | string;
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
          count === undefined ? name : `${name} (${count})`
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
  machines,
  onAddMachine,
  onRenameMachine,
  onDeleteMachine,
  templates,
  onAddTemplate,
  onRenameTemplate,
  onDeleteTemplate,
  onSetTemplateForNewProjects,
}: Props) {
  type Group = "library" | "machine" | "template";
  // The right-click menu: on a heading (name null) or on a library, machine or template.
  const [menu, setMenu] = useState<{ x: number; y: number; group: Group; name: string | null } | null>(null);
  // Which row to start renaming in place, as "group:name".
  const [renaming, setRenaming] = useState<string | null>(null);
  const [open, setOpen] = useState(true);
  const [machinesOpen, setMachinesOpen] = useState(true);
  const [templatesOpen, setTemplatesOpen] = useState(true);
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

  const openMenu = (e: React.MouseEvent, group: Group, name: string | null) => {
    e.preventDefault();
    e.stopPropagation();
    setMenu({ x: e.clientX, y: e.clientY, group, name });
  };

  const newLibrary = async () => {
    const name = await onAddLibrary();
    setOpen(true);
    onSelect({ kind: "library", name });
    setRenaming(`library:${name}`);
  };

  const newMachine = async () => {
    const name = await onAddMachine();
    setMachinesOpen(true);
    onSelect({ kind: "machine", name });
    setRenaming(`machine:${name}`);
  };

  const newTemplate = async () => {
    const name = await onAddTemplate();
    setTemplatesOpen(true);
    onSelect({ kind: "template", name });
    setRenaming(`template:${name}`);
  };

  const menuItems = (group: Group, name: string | null): MenuItem[] => {
    const items: MenuItem[] = [
      group === "library"
        ? { label: "New library", action: () => void newLibrary() }
        : group === "machine"
          ? { label: "New machine", action: () => void newMachine() }
          : { label: "New template", action: () => void newTemplate() },
    ];
    if (name !== null) {
      items.push({ label: "Rename", action: () => setRenaming(`${group}:${name}`) });
      if (group === "template") {
        items.push({
          label: "Use for new projects",
          disabled: templates?.for_new_projects === name,
          action: () => onSetTemplateForNewProjects(name),
        });
      }
      items.push({
        label: group === "library" ? "Delete library" : group === "machine" ? "Delete machine" : "Delete template",
        separatorBefore: true,
        action: () =>
          group === "library" ? onDeleteLibrary(name) : group === "machine" ? onDeleteMachine(name) : onDeleteTemplate(name),
      });
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
            onContextMenu={(e) => openMenu(e, "library", null)}
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
                  onContextMenu={(e) => openMenu(e, "library", lib.name)}
                  onRename={async (name) => {
                    const ok = await onRenameLibrary(lib.name, name);
                    if (ok) onSelect({ kind: "library", name: name.trim() });
                    return ok;
                  }}
                  renameNow={renaming === `library:${lib.name}`}
                  onRenameStarted={() => setRenaming(null)}
                />
              ))}
            </ul>
          )}
        </li>
        <li>
          <div
            className="tree-item"
            title="Right-click for New machine"
            onClick={() => setMachinesOpen((o) => !o)}
            onContextMenu={(e) => openMenu(e, "machine", null)}
          >
            <span className="chevron">{machinesOpen ? "▾" : "▸"}</span>
            Machines
          </div>
          {machinesOpen && machines && machines.machines.length > 0 && (
            <ul>
              {machines.machines.map((m) => (
                <LibraryItem
                  key={m.name}
                  name={m.name}
                  selected={selection.kind === "machine" && selection.name === m.name}
                  onClick={() => onSelect({ kind: "machine", name: m.name })}
                  onContextMenu={(e) => openMenu(e, "machine", m.name)}
                  onRename={async (name) => {
                    const ok = await onRenameMachine(m.name, name);
                    if (ok) onSelect({ kind: "machine", name: name.trim() });
                    return ok;
                  }}
                  renameNow={renaming === `machine:${m.name}`}
                  onRenameStarted={() => setRenaming(null)}
                />
              ))}
            </ul>
          )}
        </li>
        <li>
          <div
            className="tree-item"
            title="Right-click for New template"
            onClick={() => setTemplatesOpen((o) => !o)}
            onContextMenu={(e) => openMenu(e, "template", null)}
          >
            <span className="chevron">{templatesOpen ? "▾" : "▸"}</span>
            Project templates
          </div>
          {templatesOpen && templates && (
            <ul>
              {templates.templates.map((t) => (
                <LibraryItem
                  key={t.name}
                  name={t.name}
                  count={templates.for_new_projects === t.name ? "new projects" : undefined}
                  selected={selection.kind === "template" && selection.name === t.name}
                  onClick={() => onSelect({ kind: "template", name: t.name })}
                  onContextMenu={(e) => openMenu(e, "template", t.name)}
                  onRename={async (name) => {
                    const ok = await onRenameTemplate(t.name, name);
                    if (ok) onSelect({ kind: "template", name: name.trim() });
                    return ok;
                  }}
                  renameNow={renaming === `template:${t.name}`}
                  onRenameStarted={() => setRenaming(null)}
                />
              ))}
            </ul>
          )}
        </li>
      </ul>
      {menu && (
        <ContextMenu items={menuItems(menu.group, menu.name)} x={menu.x} y={menu.y} onClose={() => setMenu(null)} />
      )}
    </>
  );
}
