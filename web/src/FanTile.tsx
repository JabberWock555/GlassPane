// A ceiling fan: speed slider (Off + N steps from HA's percentage_step), power
// button, and the fan's built-in light as a button in the top-left corner.
// The spinning icon's speed follows the fan speed (a CSS rotate animation —
// transform only, so it's cheap on old GPUs).

import { useEffect, useRef, useState } from "preact/hooks";
import type { EState } from "./conn";
import type { TileCfg } from "./types";
import { Bulb, Fan, Power } from "./icons";

interface Props {
  cfg: TileCfg;
  fan?: EState;
  light?: EState;
  pending: { [id: string]: boolean };
  onToggle: (id: string) => void;
  onSpeed: (id: string, pct: number) => void;
}

/** Speed levels the fan supports (6 for the Atomberg-style fans). */
function stepsOf(fan?: EState): number {
  const step = fan && fan.a.percentage_step;
  return step && step > 0 && step < 100 ? Math.round(100 / step) : 6;
}

function levelOf(fan: EState | undefined, steps: number): number {
  if (!fan || fan.state !== "on") return 0;
  const pct = typeof fan.a.percentage === "number" ? fan.a.percentage : 100;
  return Math.max(1, Math.round((pct / 100) * steps));
}

export function FanTile({ cfg, fan, light, pending, onToggle, onSpeed }: Props) {
  const steps = stepsOf(fan);
  const actual = levelOf(fan, steps);
  const unavailable = !fan || fan.state === "unavailable" || fan.state === "unknown";

  // While dragging we show the finger's level; after release we show the
  // requested level until Home Assistant confirms it (or 4 s pass).
  const [drag, setDrag] = useState<number | null>(null);
  const [target, setTarget] = useState<number | null>(null);
  const targetTimer = useRef(0);
  useEffect(() => {
    if (target !== null && actual === target) setTarget(null);
  }, [actual, target]);

  const level = drag !== null ? drag : target !== null ? target : actual;
  const on = level > 0;
  const track = useRef<HTMLDivElement>(null);
  const dragging = useRef(false);

  const levelAt = (clientX: number) => {
    const el = track.current;
    if (!el) return level;
    const r = el.getBoundingClientRect();
    const x = Math.min(Math.max(clientX - r.left, 0), r.width);
    return Math.round((x / r.width) * steps);
  };

  const commit = (v: number) => {
    setDrag(null);
    if (v === actual) return;
    setTarget(v);
    clearTimeout(targetTimer.current);
    targetTimer.current = window.setTimeout(() => setTarget(null), 4000);
    onSpeed(cfg.entity, v === steps ? 100 : Math.round((v * 100) / steps));
  };

  // Touch (tablet)
  const onTouchStart = (e: TouchEvent) => {
    if (unavailable) return;
    dragging.current = true;
    setDrag(levelAt(e.touches[0].clientX));
  };
  const onTouchMove = (e: TouchEvent) => {
    if (!dragging.current) return;
    e.preventDefault();
    setDrag(levelAt(e.touches[0].clientX));
  };
  const onTouchEnd = (e: TouchEvent) => {
    if (!dragging.current) return;
    dragging.current = false;
    commit(levelAt(e.changedTouches[0].clientX));
  };

  // Mouse (PC browser)
  const onMouseDown = (e: MouseEvent) => {
    if (unavailable || dragging.current) return;
    dragging.current = true;
    setDrag(levelAt(e.clientX));
    const move = (ev: MouseEvent) => setDrag(levelAt(ev.clientX));
    const up = (ev: MouseEvent) => {
      window.removeEventListener("mousemove", move);
      window.removeEventListener("mouseup", up);
      dragging.current = false;
      commit(levelAt(ev.clientX));
    };
    window.addEventListener("mousemove", move);
    window.addEventListener("mouseup", up);
  };

  const name = cfg.name || (fan && fan.a.friendly_name) || cfg.entity;
  const ledOn = !!light && light.state === "on";
  const busy = target !== null || !!pending[cfg.entity];
  const spin = on ? (2.2 - (1.8 * level) / steps).toFixed(2) + "s" : "0s";
  const pct = (level / steps) * 100;

  const ticks = [];
  for (let i = 0; i <= steps; i++) {
    ticks.push(<span key={i} class={"tick" + (i <= level && on ? " lit" : "")} style={{ left: (i / steps) * 100 + "%" }} />);
  }

  return (
    <div class={"tile glass fan-tile wide" + (on ? " on" : "") + (unavailable ? " off-line" : "")}>
      <div class="fan-top">
        {cfg.light ? (
          <button
            class={"round-btn led" + (ledOn ? " on" : "") + (pending[cfg.light] ? " pending" : "")}
            aria-label="Fan light"
            onClick={() => onToggle(cfg.light!)}
          >
            <Bulb size={30} />
          </button>
        ) : (
          <span />
        )}
        <button
          class={"round-btn power" + (on ? " on" : "") + (pending[cfg.entity] ? " pending" : "")}
          aria-label="Fan power"
          disabled={unavailable}
          onClick={() => onToggle(cfg.entity)}
        >
          <Power size={30} />
        </button>
      </div>

      <div class="fan-mid">
        <div class={"fan-spin" + (on ? " running" : "")} style={{ animationDuration: spin }}>
          <Fan size={50} />
        </div>
        <div>
          <div class="fan-name">{name}</div>
          <div class={"fan-detail" + (busy ? " pending" : "")}>
            {unavailable ? "Unavailable" : on ? "Speed " + level + " of " + steps : "Off"}
          </div>
        </div>
      </div>

      <div
        class="slider"
        ref={track}
        onTouchStart={onTouchStart}
        onTouchMove={onTouchMove}
        onTouchEnd={onTouchEnd}
        onMouseDown={onMouseDown}
      >
        <div class="track">
          <div class="fill" style={{ width: pct + "%" }} />
        </div>
        {ticks}
        <div class="thumb" style={{ left: pct + "%" }} />
      </div>
      <div class="slider-labels">
        <span>Off</span>
        <span>{steps}</span>
      </div>
    </div>
  );
}
