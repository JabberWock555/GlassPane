// Simple tiles: light and switch (tap to toggle) and sensor (read-only value).

import { EState } from "./conn";
import type { TileCfg } from "./types";
import { Bulb, Drop, Gauge, ICONS, Power, Thermo } from "./icons";

interface Props {
  cfg: TileCfg;
  st?: EState;
  pending: boolean;
  onTap: (id: string) => void;
}

/** Light, switch and sensor tiles. Fans have their own FanTile. */
export function TileView({ cfg, st, pending, onTap }: Props) {
  const name = cfg.name || (st && st.a.friendly_name) || cfg.entity;
  const unavailable = !st || st.state === "unavailable" || st.state === "unknown";
  const Override = cfg.icon ? ICONS[cfg.icon] : undefined;

  if (cfg.kind === "sensor") {
    const unit = st ? st.a.unit_of_measurement || "" : "";
    const dc = st ? st.a.device_class : undefined;
    const Icon = Override || (dc === "temperature" ? Thermo : dc === "humidity" ? Drop : Gauge);
    return (
      <div class={"tile glass sensor" + (unavailable ? " off-line" : "")}>
        <div class="icon"><Icon /></div>
        <div class="value">
          {unavailable ? "—" : st!.state}
          <span class="unit">{unavailable ? "" : unit}</span>
        </div>
        <div class="name">{name}</div>
      </div>
    );
  }

  const on = !!st && st.state === "on";
  let detail = unavailable ? "Unavailable" : on ? "On" : "Off";
  if (on && st && typeof st.a.brightness === "number") {
    detail += " · " + Math.round((st.a.brightness / 255) * 100) + "%";
  }
  const Icon = Override || (cfg.kind === "light" ? Bulb : Power);
  const cls =
    "tile glass toggle " + cfg.kind + (on ? " on" : "") + (pending ? " pending" : "") +
    (unavailable ? " off-line" : "");

  return (
    <button class={cls} disabled={unavailable} onClick={() => onTap(cfg.entity)}>
      <div class="icon"><Icon /></div>
      <div class="name">{name}</div>
      <div class="detail">{detail}</div>
    </button>
  );
}
