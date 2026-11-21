import { useEffect, useLayoutEffect, useRef, useState } from "react";

/** How long the mouse rests on something before its description shows (ms). */
const DELAY = 500;

interface Tip {
  title: string;
  text: string;
  key: string;
  /** A second, muted line (e.g. "Changed in this project"). */
  note: string;
  /** The hovered element's box, in page pixels. */
  rect: DOMRect;
}

/** The attributes that give an element a hover description. */
export function tipProps(title: string, text?: string, key?: string, note?: string) {
  return { "data-tip-title": title, "data-tip": text ?? "", "data-tip-key": key ?? "", "data-tip-note": note ?? "" };
}

/**
 * The one hover description box for the whole program. Any element with `tipProps` shows it
 * after a short rest; it goes on leaving, clicking or pressing a key.
 */
export default function Tooltip() {
  const [tip, setTip] = useState<Tip | null>(null);
  const timer = useRef<number | undefined>(undefined);
  const over = useRef<HTMLElement | null>(null);

  useEffect(() => {
    const hide = () => {
      window.clearTimeout(timer.current);
      over.current = null;
      setTip(null);
    };
    const onOver = (e: PointerEvent) => {
      const el = (e.target as HTMLElement | null)?.closest?.<HTMLElement>("[data-tip-title]") ?? null;
      if (el === over.current) return;
      hide();
      if (!el) return;
      over.current = el;
      timer.current = window.setTimeout(() => {
        if (over.current !== el || !el.isConnected) return;
        setTip({
          title: el.dataset.tipTitle ?? "",
          text: el.dataset.tip ?? "",
          key: el.dataset.tipKey ?? "",
          note: el.dataset.tipNote ?? "",
          rect: el.getBoundingClientRect(),
        });
      }, DELAY);
    };
    const onLeaveWindow = (e: PointerEvent) => {
      if (!e.relatedTarget) hide();
    };
    window.addEventListener("pointerover", onOver);
    window.addEventListener("pointerout", onLeaveWindow);
    window.addEventListener("pointerdown", hide, true);
    window.addEventListener("keydown", hide, true);
    window.addEventListener("wheel", hide, true);
    window.addEventListener("blur", hide);
    return () => {
      hide();
      window.removeEventListener("pointerover", onOver);
      window.removeEventListener("pointerout", onLeaveWindow);
      window.removeEventListener("pointerdown", hide, true);
      window.removeEventListener("keydown", hide, true);
      window.removeEventListener("wheel", hide, true);
      window.removeEventListener("blur", hide);
    };
  }, []);

  return tip ? <TipBox tip={tip} /> : null;
}

/** The box, below the element (above it if there is no room), kept on screen. */
function TipBox({ tip }: { tip: Tip }) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ left: tip.rect.left, top: tip.rect.bottom + 6 });

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const { width, height } = el.getBoundingClientRect();
    const gap = 6;
    let top = tip.rect.bottom + gap;
    if (top + height > window.innerHeight - 4) top = tip.rect.top - height - gap;
    const left = Math.min(tip.rect.left, window.innerWidth - width - 4);
    setPos({ left: Math.max(4, left), top: Math.max(4, top) });
  }, [tip]);

  return (
    <div className="tooltip" ref={ref} style={pos} role="tooltip">
      <div className="tooltip-head">
        <span className="tooltip-title">{tip.title}</span>
        {tip.key && <span className="tooltip-key">{tip.key}</span>}
      </div>
      {tip.text && <div className="tooltip-text">{tip.text}</div>}
      {tip.note && <div className="tooltip-note">{tip.note}</div>}
    </div>
  );
}
