// Photo screensaver, shown after the idle time set in Settings.

import { useEffect, useRef, useState } from "preact/hooks";
import { Aqi, aqiColor, ampm, formatDate, formatTime } from "./types";

interface Props {
  count: number;
  intervalSec: number;
  showClock: boolean;
  /** "fill": landscape photos fill the screen; "whole": every photo shown whole. */
  fit: "fill" | "whole";
  h24: boolean;
  aqi: Aqi | null;
  onExit: () => void;
}

function shuffled(n: number): number[] {
  const a: number[] = [];
  for (let i = 0; i < n; i++) a.push(i);
  for (let i = n - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    const t = a[i];
    a[i] = a[j];
    a[j] = t;
  }
  return a;
}

interface Slide {
  src: string;
  /** Blurred backdrop, shown when the photo doesn't fill the screen. */
  fill: string;
  /** Show the whole photo (letterboxed) instead of filling the screen. */
  whole: boolean;
  key: number;
}

/**
 * The screen's size in real pixels. The server sizes each photo to exactly
 * this, so the browser never has to scale it (scaling looks soft).
 */
function screenPixels(): { w: number; h: number } {
  const dpr = window.devicePixelRatio || 1;
  return { w: Math.round(window.innerWidth * dpr), h: Math.round(window.innerHeight * dpr) };
}

/**
 * Full-screen photo slideshow. Two layers cross-fade (opacity only) and each
 * photo slowly zooms (transform only) — both cheap on the tablet's GPU. The
 * next photo is fully downloaded before the fade starts. Tap to exit; the
 * tap never reaches the dashboard underneath.
 */
export function Screensaver({ count, intervalSec, showClock, fit, h24, aqi, onExit }: Props) {
  const [slides, setSlides] = useState<[Slide | null, Slide | null]>([null, null]);
  const [front, setFront] = useState(0);
  const [now, setNow] = useState(new Date());
  const order = useRef<number[]>(shuffled(count));
  const pos = useRef(0);
  const seq = useRef(0);

  useEffect(() => {
    const t = setInterval(() => setNow(new Date()), 10000);
    return () => clearInterval(t);
  }, []);

  useEffect(() => {
    if (!count) return;
    let alive = true;
    let timer = 0;
    let layer = 0;
    const show = () => {
      if (pos.current >= order.current.length) {
        order.current = shuffled(count);
        pos.current = 0;
      }
      const idx = order.current[pos.current++];
      const img = new Image();
      img.onload = () => {
        if (!alive) return;
        // A photo is shown whole if the user prefers that, or if its
        // orientation doesn't match the screen (portrait photo, landscape
        // tablet) — filling would crop most of it away.
        const screenPortrait = window.innerHeight > window.innerWidth;
        const photoPortrait = img.naturalHeight > img.naturalWidth;
        const slide = {
          src: img.src,
          fill: "/photo/" + idx + "/blur",
          whole: fit === "whole" || photoPortrait !== screenPortrait,
          key: ++seq.current,
        };
        layer = 1 - layer;
        const l = layer;
        setSlides((prev) => (l === 0 ? [slide, prev[1]] : [prev[0], slide]));
        setFront(l);
        timer = window.setTimeout(show, intervalSec * 1000);
      };
      img.onerror = () => {
        if (alive) timer = window.setTimeout(show, 2000);
      };
      const px = screenPixels();
      img.src = "/photo/" + idx + "?w=" + px.w + "&h=" + px.h + "&fit=" + (fit === "whole" ? "contain" : "cover");
    };
    show();
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [count, intervalSec, fit]);

  const exit = (e: Event) => {
    e.preventDefault();
    e.stopPropagation();
    onExit();
  };

  return (
    <div class="saver" onClick={exit}>
      {slides.map((s, i) => (
        // Layers persist (so opacity can transition); the inner image is
        // re-keyed per photo so its slow zoom restarts.
        <div key={i} class={"saver-slide" + (s && i === front ? " show" : "")}>
          {s && s.whole && <div key={"f" + s.key} class="saver-fill" style={{ backgroundImage: "url(" + s.fill + ")" }} />}
          {s && (
            <div
              key={s.key}
              class={"saver-img" + (s.whole ? " contain" : "")}
              style={{ backgroundImage: "url(" + s.src + ")", animationDuration: intervalSec + 4 + "s" }}
            />
          )}
        </div>
      ))}
      {!count && <div class="saver-empty">Add photos to the screensaver folder to see them here.</div>}
      {(showClock || !count) && (
        <div class="saver-clock">
          <div class="saver-time">
            {formatTime(now, h24)}
            {!h24 && <span class="ampm">{ampm(now)}</span>}
          </div>
          <div class="saver-date">{formatDate(now, true)}</div>
          {aqi && (
            <div class="saver-aqi">
              <span class="aqi-dot" style={{ background: aqiColor(aqi.aqi) }} />
              {"AQI " + aqi.aqi + " · " + aqi.category}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
