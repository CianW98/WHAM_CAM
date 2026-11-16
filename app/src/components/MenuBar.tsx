// The menu bar across the top of the window (File, View, Edit, ...).
// Click a heading to open its menu; while one is open, hovering another heading switches to it.
// Clicking outside or Esc (handled by the caller) closes it.

import { useRef } from "react";
import { Panel, type MenuItem } from "./ContextMenu";

interface Props {
  /** The headings; each one's `items` is its drop-down menu. */
  menus: MenuItem[];
  /** The index of the open heading, or null. */
  open: number | null;
  onOpen: (index: number | null) => void;
}

export default function MenuBar({ menus, open, onOpen }: Props) {
  const headings = useRef<(HTMLButtonElement | null)[]>([]);
  const rect = open !== null ? headings.current[open]?.getBoundingClientRect() : undefined;

  return (
    <>
      <nav className={`menubar${open !== null ? " active" : ""}`} onContextMenu={(e) => e.preventDefault()}>
        {menus.map((menu, i) => (
          <button
            key={menu.label}
            ref={(el) => {
              headings.current[i] = el;
            }}
            type="button"
            className={open === i ? "open" : ""}
            onPointerDown={(e) => {
              if (e.button !== 0) return;
              onOpen(open === i ? null : i);
            }}
            onMouseEnter={() => {
              if (open !== null && open !== i) onOpen(i);
            }}
          >
            {menu.label}
          </button>
        ))}
      </nav>
      {open !== null && rect && menus[open]?.items && (
        <div
          className="menu-backdrop"
          onPointerDown={(e) => {
            if (e.target === e.currentTarget) onOpen(null);
          }}
          onContextMenu={(e) => {
            e.preventDefault();
            if (e.target === e.currentTarget) onOpen(null);
          }}
        >
          <Panel key={open} items={menus[open].items!} x={rect.left} y={rect.bottom} onClose={() => onOpen(null)} />
        </div>
      )}
    </>
  );
}
