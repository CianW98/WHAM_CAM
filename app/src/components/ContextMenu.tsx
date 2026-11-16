import { useLayoutEffect, useRef, useState } from "react";

export interface MenuItem {
  label: string;
  /** Shown on the right, e.g. "Ctrl+C". */
  shortcut?: string;
  disabled?: boolean;
  /** An on/off item: shows a tick when true. */
  checked?: boolean;
  action?: () => void;
  /** A submenu, opened by hovering. */
  items?: MenuItem[];
  /** Draw a dividing line above this item. */
  separatorBefore?: boolean;
}

/** One menu panel; positioned so it stays on screen. */
export function Panel({
  items,
  x,
  y,
  onClose,
  alignRightOf,
}: {
  items: MenuItem[];
  x: number;
  y: number;
  onClose: () => void;
  /** For a submenu: the parent row's left edge, used to flip to the left side if needed. */
  alignRightOf?: number;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ left: x, top: y });
  const [open, setOpen] = useState<{ index: number; x: number; y: number; left: number } | null>(null);

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const { width, height } = el.getBoundingClientRect();
    let left = x;
    if (left + width > window.innerWidth - 4) {
      left = alignRightOf !== undefined ? alignRightOf - width : window.innerWidth - width - 4;
    }
    const top = Math.min(y, window.innerHeight - height - 4);
    setPos({ left: Math.max(4, left), top: Math.max(4, top) });
  }, [x, y, alignRightOf]);

  return (
    <div className="context-menu" ref={ref} style={pos} role="menu" onContextMenu={(e) => e.preventDefault()}>
      {items.map((item, i) => (
        <div key={item.label}>
          {item.separatorBefore && <div className="menu-separator" />}
          <div
            role="menuitem"
            aria-disabled={item.disabled}
            className={`menu-item${item.disabled ? " disabled" : ""}${open?.index === i ? " open" : ""}`}
            onMouseEnter={(e) => {
              const r = e.currentTarget.getBoundingClientRect();
              setOpen(item.items && !item.disabled ? { index: i, x: r.right, y: r.top - 4, left: r.left } : null);
            }}
            onClick={() => {
              if (item.disabled || item.items || !item.action) return;
              onClose();
              item.action();
            }}
          >
            <span>
              {item.checked !== undefined && <span className="menu-check">{item.checked ? "✓" : ""}</span>}
              {item.label}
            </span>
            <span className="menu-shortcut">{item.items ? "▸" : item.shortcut ?? ""}</span>
          </div>
        </div>
      ))}
      {open && items[open.index]?.items && (
        <Panel items={items[open.index].items!} x={open.x} y={open.y} alignRightOf={open.left} onClose={onClose} />
      )}
    </div>
  );
}

/** A right-click menu at (x, y) (page pixels). Clicking outside or Esc (handled by the caller) closes it. */
export default function ContextMenu({
  items,
  x,
  y,
  onClose,
}: {
  items: MenuItem[];
  x: number;
  y: number;
  onClose: () => void;
}) {
  return (
    <div
      className="menu-backdrop"
      onPointerDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
      onContextMenu={(e) => {
        e.preventDefault();
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <Panel items={items} x={x} y={y} onClose={onClose} />
    </div>
  );
}
