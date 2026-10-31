// Settings overlay (gear button): screensaver and display options plus an
// About section. Changes apply immediately and are saved on the server
// (PUT /api/settings) so they survive reloads and are shared by all screens.

import { useEffect, useState } from "preact/hooks";
import { Close } from "./icons";
import { aqiColor, Settings } from "./types";

interface Info {
  version: string;
  uptime_s: number;
  ha_connected: boolean;
  ha_version: string;
  location: { name: string; lat: number; lon: number } | null;
  aqi: { aqi: number; category: string; pm25?: number; pm10?: number; time?: string; source?: string } | null;
  entities: number;
  photos: number;
  photos_dir: string;
}

interface Props {
  settings: Settings;
  onChange: (patch: any) => void;
  onClose: () => void;
  onPreviewScreensaver: () => void;
}

function Toggle({ on, onChange, label }: { on: boolean; onChange: (v: boolean) => void; label: string }) {
  return (
    <button class={"toggle-sw" + (on ? " on" : "")} role="switch" aria-checked={on} aria-label={label} onClick={() => onChange(!on)}>
      <span class="knob" />
    </button>
  );
}

function Choice<T>({ value, options, onChange }: { value: T; options: { v: T; label: string }[]; onChange: (v: T) => void }) {
  return (
    <div class="choice">
      {options.map((o) => (
        <button key={String(o.v)} class={o.v === value ? "sel" : ""} onClick={() => onChange(o.v)}>
          {o.label}
        </button>
      ))}
    </div>
  );
}

function uptime(s: number): string {
  const d = Math.floor(s / 86400);
  const h = Math.floor((s % 86400) / 3600);
  const m = Math.floor((s % 3600) / 60);
  return (d ? d + "d " : "") + (d || h ? h + "h " : "") + m + "m";
}

export function SettingsPage({ settings, onChange, onClose, onPreviewScreensaver }: Props) {
  const [info, setInfo] = useState<Info | null>(null);
  useEffect(() => {
    fetch("/api/info").then((r) => r.json()).then(setInfo).catch(() => setInfo(null));
  }, []);
  const ss = settings.screensaver;
  const chrome = (navigator.userAgent.match(/Chrome\/(\d+)/) || [])[1];

  const row = (label: string, hint: string | null, control: any) => (
    <div class="set-row">
      <div class="set-text">
        <div class="set-label">{label}</div>
        {hint && <div class="set-hint">{hint}</div>}
      </div>
      {control}
    </div>
  );

  return (
    <div class="settings-backdrop" onClick={onClose}>
      <div class="settings glass-solid" onClick={(e) => e.stopPropagation()}>
        <div class="settings-head">
          <h2>Settings</h2>
          <button class="round-btn" aria-label="Close settings" onClick={onClose}>
            <Close size={22} />
          </button>
        </div>

        <div class="settings-body">
          <section>
            <h3>Screensaver</h3>
            {row(
              "Photo screensaver",
              info ? info.photos + " photo" + (info.photos === 1 ? "" : "s") + " in " + info.photos_dir : null,
              <Toggle label="Photo screensaver" on={ss.enabled} onChange={(v) => onChange({ screensaver: { enabled: v } })} />,
            )}
            {row("Start after", "of no one touching the screen",
              <Choice value={ss.idleMin} onChange={(v) => onChange({ screensaver: { idleMin: v } })}
                options={[1, 2, 5, 10, 15].map((m) => ({ v: m, label: m + " min" }))} />)}
            {row("Change photo every", null,
              <Choice value={ss.intervalSec} onChange={(v) => onChange({ screensaver: { intervalSec: v } })}
                options={[10, 20, 30, 60].map((s) => ({ v: s, label: s + " s" }))} />)}
            {row("Photo fit", "Portrait photos are always shown whole",
              <Choice value={ss.fit} onChange={(v) => onChange({ screensaver: { fit: v } })}
                options={[{ v: "fill" as const, label: "Fill screen" }, { v: "whole" as const, label: "Whole photo" }]} />)}
            {row("Show clock on photos", null,
              <Toggle label="Show clock on photos" on={ss.showClock} onChange={(v) => onChange({ screensaver: { showClock: v } })} />)}
            <button class="pill-btn" onClick={onPreviewScreensaver}>Preview screensaver</button>
          </section>

          <section>
            <h3>Display</h3>
            {row("24-hour clock", null, <Toggle label="24-hour clock" on={settings.clock24} onChange={(v) => onChange({ clock24: v })} />)}
            {row("Air quality (AQI)", "Shown under the clock", <Toggle label="Air quality" on={settings.showAqi} onChange={(v) => onChange({ showAqi: v })} />)}
            {row("Compact sidebar", "Room icons only", <Toggle label="Compact sidebar" on={settings.sidebarCollapsed} onChange={(v) => onChange({ sidebarCollapsed: v })} />)}
            {row("Keep screen awake", "While the dashboard is open", <Toggle label="Keep screen awake" on={settings.keepAwake} onChange={(v) => onChange({ keepAwake: v })} />)}
          </section>

          <section>
            <h3>About</h3>
            {!info ? (
              <div class="set-hint">Loading…</div>
            ) : (
              <div class="info-grid">
                <span>Home Assistant</span>
                <span class={info.ha_connected ? "ok" : "bad"}>
                  {info.ha_connected ? "Connected" : "Disconnected"}{info.ha_version ? " · " + info.ha_version : ""}
                </span>
                <span>Air quality</span>
                <span>
                  {info.aqi ? (
                    <span>
                      <span class="aqi-dot" style={{ background: aqiColor(info.aqi.aqi) }} />
                      {" " + info.aqi.aqi + " · " + info.aqi.category}
                      {info.aqi.pm25 != null ? " · PM2.5 " + info.aqi.pm25 : ""}
                    </span>
                  ) : "Waiting for data"}
                </span>
                <span>Location</span>
                <span>{info.location ? info.location.name + " (" + info.location.lat.toFixed(2) + ", " + info.location.lon.toFixed(2) + ")" : "—"}</span>
                <span>Devices</span>
                <span>{info.entities} entities on the dashboard</span>
                <span>Server</span>
                <span>glasspane {info.version} · up {uptime(info.uptime_s)}</span>
                <span>This screen</span>
                <span>{location.host + (chrome ? " · Chrome " + chrome : "") + " · " + window.innerWidth + "×" + window.innerHeight}</span>
              </div>
            )}
            <div class="set-hint">AQI from Open-Meteo, US AQI scale (not India's CPCB index).</div>
            <button class="pill-btn" onClick={() => location.reload()}>Reload dashboard</button>
          </section>
        </div>
      </div>
    </div>
  );
}
