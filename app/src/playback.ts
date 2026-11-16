// Where the tool is at a given playback time. Display only: the points and times come from the
// engine; this just walks along them.
import type { Playback, PlaybackStep } from "./engine";

export interface ToolAt {
  /** Index of the current step in `playback.steps`. */
  step: number;
  /** Index of the segment within the step (points[seg] → points[seg + 1]). */
  seg: number;
  /** Tool position (tool tip centre), mm. */
  point: [number, number, number];
}

const dist = (a: number[], b: number[]) => Math.hypot(b[0] - a[0], b[1] - a[1], b[2] - a[2]);

/** The step running at time `t` (the last one that has started). */
export function stepAt(pb: Playback, t: number): number {
  let lo = 0;
  let hi = pb.steps.length - 1;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (pb.steps[mid].start_s <= t) lo = mid;
    else hi = mid - 1;
  }
  return lo;
}

/** Where the tool is at time `t` (seconds), moving at constant speed along each step. */
export function toolAt(pb: Playback, t: number): ToolAt | null {
  if (pb.steps.length === 0) return null;
  const i = stepAt(pb, t);
  const s: PlaybackStep = pb.steps[i];
  const f = s.duration_s > 0 ? Math.min(1, Math.max(0, (t - s.start_s) / s.duration_s)) : 1;
  const pts = s.points;
  if (pts.length < 2) return { step: i, seg: 0, point: [...pts[0]] };
  let total = 0;
  for (let k = 1; k < pts.length; k++) total += dist(pts[k - 1], pts[k]);
  let want = f * total;
  for (let k = 1; k < pts.length; k++) {
    const len = dist(pts[k - 1], pts[k]);
    if (want <= len || k === pts.length - 1) {
      const u = len > 0 ? Math.min(1, want / len) : 1;
      const a = pts[k - 1];
      const b = pts[k];
      return {
        step: i,
        seg: k - 1,
        point: [a[0] + (b[0] - a[0]) * u, a[1] + (b[1] - a[1]) * u, a[2] + (b[2] - a[2]) * u],
      };
    }
    want -= len;
  }
  return { step: i, seg: pts.length - 2, point: [...pts[pts.length - 1]] };
}

/** "m:ss" (or "h:mm:ss"). */
export function formatTime(seconds: number): string {
  const s = Math.max(0, Math.round(seconds));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const ss = String(s % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${ss}` : `${m}:${ss}`;
}
