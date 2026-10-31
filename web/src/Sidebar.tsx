// Left side panel: clock + AQI, the built-in Home and Shopping sections, the
// rooms from the config, and connection status. Collapses to an icon rail.

import { useEffect, useState } from "preact/hooks";
import { ChevronLeft, ChevronRight, ICONS } from "./icons";
import { Aqi, aqiColor, ampm, formatDate, formatTime, PageCfg } from "./types";

export type View = { kind: "home" } | { kind: "shopping" } | { kind: "room"; index: number };

interface Props {
  pages: PageCfg[];
  view: View;
  onView: (v: View) => void;
  collapsed: boolean;
  onCollapse: (c: boolean) => void;
  hasShopping: boolean;
  homeBadge: number;
  shoppingBadge: number;
  roomBadge: (p: PageCfg) => number;
  h24: boolean;
  aqi: Aqi | null;
  showAqi: boolean;
  status: string;
}

export function Sidebar(p: Props) {
  const item = (key: string, icon: string, label: string, active: boolean, badge: number, go: () => void) => {
    const Icon = ICONS[icon] || ICONS.home;
    return (
      <button key={key} class={"room" + (active ? " active" : "")} onClick={go} aria-label={label}>
        <span class="room-icon">
          <Icon size={24} />
          {badge > 0 && p.collapsed && <span class="dot-badge">{badge}</span>}
        </span>
        {!p.collapsed && <span class="room-name">{label}</span>}
        {!p.collapsed && badge > 0 && <span class="count">{badge}</span>}
      </button>
    );
  };
  const v = p.view;

  return (
    <aside class={"side glass" + (p.collapsed ? " collapsed" : "")}>
      <Clock h24={p.h24} compact={p.collapsed} aqi={p.showAqi ? p.aqi : null} />
      <nav class="rooms">
        {item("home", "home", "Home", v.kind === "home", p.homeBadge, () => p.onView({ kind: "home" }))}
        {p.hasShopping &&
          item("shop", "cart", "Shopping", v.kind === "shopping", p.shoppingBadge, () => p.onView({ kind: "shopping" }))}
        <div class="side-label">{p.collapsed ? "" : "Rooms"}</div>
        {p.pages.map((pg, i) =>
          item("r" + i, pg.icon || "home", pg.title, v.kind === "room" && v.index === i, p.roomBadge(pg), () =>
            p.onView({ kind: "room", index: i }),
          ),
        )}
      </nav>
      <div class="side-foot">
        <span class={"link-dot" + (p.status ? " bad" : "")} />
        {!p.collapsed && <span class={"status" + (p.status ? " bad" : "")}>{p.status || "Connected"}</span>}
        <button
          class="collapse-btn"
          aria-label={p.collapsed ? "Expand sidebar" : "Collapse sidebar"}
          onClick={() => p.onCollapse(!p.collapsed)}
        >
          {p.collapsed ? <ChevronRight size={20} /> : <ChevronLeft size={20} />}
        </button>
      </div>
    </aside>
  );
}

function Clock({ h24, compact, aqi }: { h24: boolean; compact: boolean; aqi: Aqi | null }) {
  const [now, setNow] = useState(new Date());
  useEffect(() => {
    const t = setInterval(() => setNow(new Date()), 10000);
    return () => clearInterval(t);
  }, []);

  if (compact) {
    return (
      <div class="clock compact">
        <div class="time">{formatTime(now, h24)}</div>
        {aqi && (
          <div class="aqi-mini" title={"AQI " + aqi.aqi}>
            <span class="aqi-dot" style={{ background: aqiColor(aqi.aqi) }} />
            {aqi.aqi}
          </div>
        )}
      </div>
    );
  }
  return (
    <div class="clock">
      <div class="time">
        {formatTime(now, h24)}
        {!h24 && <span class="ampm">{ampm(now)}</span>}
      </div>
      <div class="date">{formatDate(now)}</div>
      {aqi && (
        <div class="aqi" title={aqi.source}>
          <span class="aqi-dot" style={{ background: aqiColor(aqi.aqi) }} />
          <span class="aqi-num">AQI {aqi.aqi}</span>
          <span class="aqi-cat">{aqi.category}</span>
        </div>
      )}
    </div>
  );
}
