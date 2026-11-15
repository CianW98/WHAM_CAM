import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

/**
 * Preset colours, spread evenly around the colour wheel and readable on the dark drawing
 * area. Row 1: bright hues in wheel order. Row 2: darker versions of the same hues.
 * Row 3: extra blues/violets, magenta, pink, brown and neutrals. No yellow (the selection
 * highlight) and no transparency: layer colours are plain #rrggbb.
 */
const PRESETS = [
  ["#ff0000", "#ff8000", "#80ff00", "#00ff00", "#00ff99", "#00ffff", "#00b0ff", "#0055ff"],
  ["#b00000", "#a05000", "#4a9000", "#008000", "#00995c", "#008b8b", "#006699", "#2040c0"],
  ["#6495ed", "#8040ff", "#ff00ff", "#ff60b0", "#a0522d", "#ffffff", "#c0c0c0", "#808080"],
];

const HEX = /^#?([0-9a-f]{6})$/i;

interface Props {
  /** Current colour, "#rrggbb". */
  color: string;
  /** Called once when a colour is chosen. */
  onPick: (color: string) => void;
  size?: "small" | "large";
  title?: string;
}

/** A colour square that opens WHAM_CAM's colour picker (presets + hex box) when clicked. */
export default function ColorSwatch({ color, onPick, size = "small", title = "Change colour" }: Props) {
  const anchor = useRef<HTMLSpanElement>(null);
  const [open, setOpen] = useState(false);

  const choose = (c: string) => {
    setOpen(false);
    if (c.toLowerCase() !== color.toLowerCase()) onPick(c.toLowerCase());
  };

  return (
    <>
      <span
        ref={anchor}
        className={`swatch-picker ${size}`}
        title={title}
        onClick={(e) => {
          e.stopPropagation(); // don't also select / open-close the tree item
          setOpen((o) => !o);
        }}
        onDoubleClick={(e) => e.stopPropagation()}
      >
        <span className="swatch" style={{ background: color }} />
      </span>
      {open && anchor.current && (
        <ColorPopover anchor={anchor.current} color={color} onChoose={choose} onClose={() => setOpen(false)} />
      )}
    </>
  );
}

function ColorPopover({
  anchor,
  color,
  onChoose,
  onClose,
}: {
  anchor: HTMLElement;
  color: string;
  onChoose: (c: string) => void;
  onClose: () => void;
}) {
  const box = useRef<HTMLDivElement>(null);
  const [hex, setHex] = useState(color);
  const [pos, setPos] = useState({ left: 0, top: 0 });
  const valid = HEX.test(hex.trim());

  // Place below the swatch, kept inside the window.
  useLayoutEffect(() => {
    const a = anchor.getBoundingClientRect();
    const w = box.current?.offsetWidth ?? 220;
    const h = box.current?.offsetHeight ?? 140;
    setPos({
      left: Math.max(4, Math.min(a.left, window.innerWidth - w - 4)),
      top: a.bottom + 4 + h > window.innerHeight ? Math.max(4, a.top - h - 4) : a.bottom + 4,
    });
  }, [anchor]);

  // Close on Esc or a click outside. Capture phase, so the app's own Esc handling doesn't also run.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        onClose();
      }
    };
    const onDown = (e: MouseEvent) => {
      const t = e.target as Node;
      if (!box.current?.contains(t) && !anchor.contains(t)) onClose();
    };
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("mousedown", onDown, true);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("mousedown", onDown, true);
    };
  }, [anchor, onClose]);

  const applyHex = () => {
    const m = HEX.exec(hex.trim());
    if (m) onChoose(`#${m[1]}`);
  };

  // React events bubble through portals to the tree item, so stop them here.
  const stop = (e: React.SyntheticEvent) => e.stopPropagation();

  return createPortal(
    <div
      ref={box}
      className="color-popover"
      style={{ left: pos.left, top: pos.top }}
      onClick={stop}
      onDoubleClick={stop}
      onMouseDown={stop}
      onKeyDown={stop}
    >
      <div className="color-grid">
        {PRESETS.flat().map((c) => (
          <button
            key={c}
            className={`color-cell${c === color.toLowerCase() ? " current" : ""}`}
            style={{ background: c }}
            title={c}
            aria-label={c}
            onClick={() => onChoose(c)}
          />
        ))}
      </div>
      <div className="color-hex">
        <span className="swatch" style={{ background: valid ? `#${HEX.exec(hex.trim())![1]}` : "transparent" }} />
        <input
          className={`prop-input${valid ? "" : " invalid"}`}
          value={hex}
          spellCheck={false}
          autoFocus
          onFocus={(e) => e.currentTarget.select()}
          onChange={(e) => setHex(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && applyHex()}
        />
        <button className="primary" disabled={!valid} onClick={applyHex}>
          OK
        </button>
      </div>
    </div>,
    document.body,
  );
}
