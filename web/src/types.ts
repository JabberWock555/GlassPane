// Shared types, settings and small helpers.

export interface TileCfg {
  entity: string;
  kind: "light" | "switch" | "sensor" | "fan" | "todo";
  name?: string;
  /** For fans: the built-in light. */
  light?: string;
  icon?: string;
  /** Grid rows to span (to-do lists default to 2). */
  rows?: number;
  /** Grid columns to span, 1–4 (fans and lists default to 2). */
  cols?: number;
}

export interface PageCfg {
  title: string;
  icon?: string;
  tiles: TileCfg[];
}

export interface Layout {
  pages: PageCfg[];
  shopping_list?: string | null;
}

export interface Aqi {
  aqi: number;
  pm25?: number | null;
  pm10?: number | null;
  category: string;
  time?: string;
  source?: string;
}

/** Inline grid placement for tiles that override their default size. */
export function spanStyle(cfg: TileCfg): { [k: string]: string } | undefined {
  if (!cfg.rows && !cfg.cols) return undefined;
  const s: { [k: string]: string } = {};
  if (cfg.cols) s.gridColumn = "span " + Math.max(1, Math.min(4, cfg.cols));
  if (cfg.rows) s.gridRow = "span " + Math.max(1, Math.min(2, cfg.rows));
  return s;
}

// ---- Settings (stored on the server in settings.json) ----------------------

export interface Settings {
  clock24: boolean;
  showAqi: boolean;
  keepAwake: boolean;
  sidebarCollapsed: boolean;
  screensaver: {
    enabled: boolean;
    idleMin: number;
    intervalSec: number;
    showClock: boolean;
    /** "fill": landscape photos fill the screen; "whole": never crop. */
    fit: "fill" | "whole";
  };
}

export const DEFAULT_SETTINGS: Settings = {
  clock24: true,
  showAqi: true,
  keepAwake: true,
  sidebarCollapsed: false,
  screensaver: { enabled: false, idleMin: 5, intervalSec: 20, showClock: true, fit: "fill" },
};

/** Defaults overlaid with whatever the server has stored. */
export function mergeSettings(stored: any): Settings {
  const s = Object.assign({}, DEFAULT_SETTINGS, stored || {});
  s.screensaver = Object.assign({}, DEFAULT_SETTINGS.screensaver, (stored && stored.screensaver) || {});
  return s;
}

export function saveSettings(patch: any): Promise<any> {
  return fetch("/api/settings", {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(patch),
  }).then((r) => r.json());
}

// ---- Time ------------------------------------------------------------------

const DAYS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const DAYS_LONG = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

export function formatTime(d: Date, h24: boolean): string {
  const mm = ("0" + d.getMinutes()).slice(-2);
  if (h24) return d.getHours() + ":" + mm;
  const h = d.getHours() % 12 || 12;
  return h + ":" + mm;
}

export function ampm(d: Date): string {
  return d.getHours() < 12 ? "AM" : "PM";
}

export function formatDate(d: Date, long?: boolean): string {
  return (long ? DAYS_LONG : DAYS)[d.getDay()] + ", " + d.getDate() + " " + MONTHS[d.getMonth()];
}

/** US AQI colour scale. */
export function aqiColor(aqi: number): string {
  if (aqi <= 50) return "#4ade80";
  if (aqi <= 100) return "#facc15";
  if (aqi <= 150) return "#fb923c";
  if (aqi <= 200) return "#f87171";
  if (aqi <= 300) return "#c084fc";
  return "#e11d48";
}
