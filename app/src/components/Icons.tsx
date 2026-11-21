import type { ReactNode } from "react";

/** Simple line icons, drawn on a 16×16 grid in the current text colour. */
function Svg({ children, size = 24 }: { children: ReactNode; size?: number }) {
  return (
    <svg
      viewBox="0 0 16 16"
      width={size}
      height={size}
      aria-hidden="true"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.3"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      {children}
    </svg>
  );
}

/** The layer/part show-hide eye; crossed out when hidden. */
export function EyeIcon({ shown }: { shown: boolean }) {
  return (
    <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
      <path
        d="M1 8c1.8-3.2 4.2-4.8 7-4.8s5.2 1.6 7 4.8c-1.8 3.2-4.2 4.8-7 4.8S2.8 11.2 1 8z"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.3"
      />
      <circle cx="8" cy="8" r="2.2" fill="currentColor" />
      {!shown && <path d="M2 14L14 2" stroke="currentColor" strokeWidth="1.5" />}
    </svg>
  );
}

const dot = (x: number, y: number, r = 1.1) => <circle cx={x} cy={y} r={r} fill="currentColor" stroke="none" />;

/** The path drawn by the Show toolpaths icon, also used by Show cut width. */
const TOOLPATH = "M2 13V4.5a2 2 0 012-2h4a2 2 0 012 2v7a2 2 0 002 2h2";

/** One icon per toolbar action, by the action's name. */
const ICONS: Record<string, ReactNode> = {
  New: (
    <>
      <path d="M3.5 1.5h6l3 3v10h-9z" />
      <path d="M9.5 1.5v3h3" />
    </>
  ),
  Open: (
    <>
      <path d="M1.5 13.5v-10h4l1.5 1.5h5.5v2" />
      <path d="M1.5 13.5l2.2-6h11l-2.2 6z" />
    </>
  ),
  Save: (
    <>
      <path d="M2 2h9.5L14 4.5V14H2z" />
      <path d="M4.5 2v3.5h6V2" />
      <rect x="4.5" y="9" width="7" height="5" />
    </>
  ),
  Import: (
    <>
      <path d="M8 1.5v8" />
      <path d="M4.8 6.5L8 9.7l3.2-3.2" />
      <path d="M2 10.5v3.5h12v-3.5" />
    </>
  ),
  Undo: (
    <>
      <path d="M5.5 3L2.5 6l3 3" />
      <path d="M2.5 6h7a4 4 0 010 8H6" />
    </>
  ),
  Redo: (
    <>
      <path d="M10.5 3l3 3-3 3" />
      <path d="M13.5 6h-7a4 4 0 000 8H10" />
    </>
  ),
  Delete: (
    <>
      <path d="M2.5 4h11" />
      <path d="M6 4V2.5h4V4" />
      <path d="M3.8 4l.8 10h6.8l.8-10" />
      <path d="M6.5 6.5v5M9.5 6.5v5" />
    </>
  ),
  Line: (
    <>
      <path d="M3 13L13 3" />
      {dot(3, 13, 1.5)}
      {dot(13, 3, 1.5)}
    </>
  ),
  Arc: (
    <>
      <path d="M2.5 13a10.5 10.5 0 0111-9.5" />
      {dot(2.5, 13, 1.5)}
      {dot(13.5, 3.5, 1.5)}
    </>
  ),
  Circle: (
    <>
      <circle cx="8" cy="8" r="5.5" />
      {dot(8, 8)}
    </>
  ),
  Rectangle: <rect x="2" y="4" width="12" height="8" />,
  Polyline: (
    <>
      <path d="M2 12.5L5.5 4l4 6.5L14 3" />
      {dot(2, 12.5)}
      {dot(5.5, 4)}
      {dot(9.5, 10.5)}
      {dot(14, 3)}
    </>
  ),
  Point: (
    <>
      <path d="M8 3.5v9M3.5 8h9" />
      {dot(8, 8, 1.5)}
    </>
  ),
  Dimension: (
    <>
      <path d="M2 5v7M14 5v7" />
      <path d="M2 8.5h12" />
      <path d="M4 7l-2 1.5L4 10M12 7l2 1.5L12 10" />
    </>
  ),
  // Profile: a shape with the cut path round it and the cutter on the path.
  Profile: (
    <>
      <rect x="4.5" y="4.5" width="7" height="7" />
      <rect x="2" y="2" width="12" height="12" rx="2.5" strokeWidth="1" />
      {dot(14, 8, 1.9)}
    </>
  ),
  // Pocket: an outline with offset loops inside.
  Pocket: (
    <>
      <rect x="2" y="2" width="12" height="12" rx="1" />
      <rect x="4.5" y="4.5" width="7" height="7" rx="0.6" strokeWidth="1" />
      <rect x="7" y="7" width="2" height="2" strokeWidth="1" />
    </>
  ),
  // Drill: a drill bit going down into a hole.
  Drill: (
    <>
      <path d="M6 1.5h4v6.5L8 11 6 8z" />
      <path d="M6 4l4 1.5M6 6.5L10 8" strokeWidth="1" />
      <path d="M2 11h3.5M10.5 11H14M5.5 11v3.5h5V11" />
    </>
  ),
  // Generate toolpaths: circular arrows (recalculate).
  "Generate toolpaths": (
    <>
      <path d="M13 6.5A5.2 5.2 0 003.6 4.6" />
      <path d="M3.3 2v2.9h2.9" />
      <path d="M3 9.5a5.2 5.2 0 009.4 1.9" />
      <path d="M12.7 14v-2.9H9.8" />
    </>
  ),
  // Show toolpaths: a path with an arrow at its end.
  "Show toolpaths": (
    <>
      <path d={TOOLPATH} />
      <path d="M12.3 11.8l1.7 1.7-1.7 1.7" />
      {dot(2, 13, 1.3)}
    </>
  ),
  // Show cut width: the Show toolpaths path with the cut width shaded along it.
  "Show cut width": (
    <>
      <path d={TOOLPATH} strokeWidth="3.6" opacity="0.35" />
      <path d={TOOLPATH} />
    </>
  ),
  // Produce G-code: an NC file.
  "Produce G-code": (
    <>
      <path d="M3 1.5h7l3 3v10H3z" />
      <path d="M10 1.5v3h3" />
      <path d="M5 12V8l2 4V8" strokeWidth="1.1" />
      <path d="M11 8.6a1.6 1.6 0 100 2.8" strokeWidth="1.1" />
    </>
  ),
};

/** The icon for a toolbar action, or nothing if it has none. */
export function ActionIcon({ name }: { name: string }) {
  const icon = ICONS[name];
  return icon ? <Svg>{icon}</Svg> : null;
}
