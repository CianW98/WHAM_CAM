import { useEffect, useRef } from "react";
import type { Playback } from "../engine";
import { formatTime, stepAt, toolAt } from "../playback";

const SPEEDS = [1, 2, 5, 10, 50, 100];
const EPS = 1e-9;

interface Props {
  playback: Playback;
  /** Current playback time (seconds). */
  time: number;
  onTime: (t: number) => void;
  playing: boolean;
  onPlaying: (p: boolean) => void;
  speed: number;
  onSpeed: (s: number) => void;
  /** Operation names by id, for the readout. */
  names: Map<number, string>;
}

/** Tool playback controls and readout, along the bottom of the drawing area. */
export default function PlaybackBar({ playback, time, onTime, playing, onPlaying, speed, onSpeed, names }: Props) {
  const total = playback.total_s;
  const live = useRef({ time, speed, total, onTime, onPlaying });
  live.current = { time, speed, total, onTime, onPlaying };

  // Advance the time while playing, at the chosen multiple of real time.
  useEffect(() => {
    if (!playing) return;
    let last = performance.now();
    let frame = requestAnimationFrame(function tick(now) {
      const l = live.current;
      const t = Math.min(l.total, l.time + ((now - last) / 1000) * l.speed);
      last = now;
      l.onTime(t);
      if (t >= l.total) l.onPlaying(false);
      else frame = requestAnimationFrame(tick);
    });
    return () => cancelAnimationFrame(frame);
  }, [playing]);

  const steps = playback.steps;
  const at = toolAt(playback, time);
  const step = at ? steps[at.step] : null;

  const play = () => {
    if (time >= total - EPS) onTime(0); // play again from the start
    onPlaying(!playing);
  };
  const back = () => {
    onPlaying(false);
    if (steps.length === 0) return;
    const i = stepAt(playback, time);
    const s = steps[i];
    // Mid-move: back to its start; at its start: back to the start of the move before.
    if (time > s.start_s + EPS || i === 0) onTime(s.start_s);
    else onTime(steps[i - 1].start_s);
  };
  const forward = () => {
    onPlaying(false);
    if (steps.length === 0) return;
    let i = stepAt(playback, time);
    let end = steps[i].start_s + steps[i].duration_s;
    // Skip moves that take no time, so each press moves the tool.
    while (end <= time + EPS && i < steps.length - 1) {
      i++;
      end = steps[i].start_s + steps[i].duration_s;
    }
    onTime(Math.min(total, end));
  };

  const fmt = (n: number) => n.toFixed(3);
  return (
    <div className="playback-bar" onPointerDown={(e) => e.stopPropagation()}>
      <button title="Back to start" onClick={() => (onPlaying(false), onTime(0))}>
        ⏮
      </button>
      <button title="One move back" onClick={back}>
        ◀
      </button>
      <button className="play" title={playing ? "Pause" : "Play"} onClick={play}>
        {playing ? "⏸" : "▶"}
      </button>
      <button title="One move forward" onClick={forward}>
        ▶|
      </button>
      <input
        type="range"
        min={0}
        max={total}
        step="any"
        value={time}
        onChange={(e) => {
          onPlaying(false);
          onTime(Number(e.target.value));
        }}
      />
      <select value={speed} title="Playback speed (× real time)" onChange={(e) => onSpeed(Number(e.target.value))}>
        {SPEEDS.map((s) => (
          <option key={s} value={s}>
            {s}×
          </option>
        ))}
      </select>
      <span className="playback-time" title="Estimated: rapids assumed at 5000 mm/min">
        {formatTime(time)} / {formatTime(total)} (est.)
      </span>
      {step && at && (
        <span className="playback-readout">
          {names.get(step.operation_id) ?? `Operation ${step.operation_id}`} · Move {step.move_number} ·{" "}
          {step.kind}
          {step.feed !== null ? ` F${step.feed}` : ""} · X {fmt(at.point[0])} Y {fmt(at.point[1])} Z{" "}
          {fmt(at.point[2])}
        </span>
      )}
    </div>
  );
}
