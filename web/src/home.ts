// Picks the device tiles for the Home view: everything that's on right now,
// or — when nothing is on — the devices used most from the dashboard.
import type { EState } from "./conn";
import type { PageCfg, TileCfg } from "./types";

type States = { [id: string]: EState };

const on = (st?: EState) => !!st && st.state === "on";

/** Every controllable device once (fans win over duplicate entries). */
export function deviceTiles(pages: PageCfg[]): TileCfg[] {
  const fanLights: { [id: string]: boolean } = {};
  pages.forEach((p) => p.tiles.forEach((t) => { if (t.kind === "fan" && t.light) fanLights[t.light] = true; }));
  const seen: { [id: string]: boolean } = {};
  const out: TileCfg[] = [];
  pages.forEach((p) =>
    p.tiles.forEach((t) => {
      if (t.kind === "sensor" || t.kind === "todo" || seen[t.entity] || fanLights[t.entity]) return;
      seen[t.entity] = true;
      // Home tiles use the auto-size so they pack predictably.
      out.push({ entity: t.entity, kind: t.kind, name: t.name, light: t.light, icon: t.icon });
    }),
  );
  return out;
}

export function isActive(t: TileCfg, states: States): boolean {
  return on(states[t.entity]) || (!!t.light && on(states[t.light]));
}

/** Grid cells a tile occupies (fans are 2 wide). */
const cells = (t: TileCfg) => (t.kind === "fan" ? 2 : 1);

export interface HomePick {
  mode: "active" | "frequent";
  tiles: TileCfg[];
  /** Active devices that didn't fit. */
  more: number;
}

export function pickHome(
  pages: PageCfg[],
  states: States,
  usage: { [id: string]: number },
  capacity: number,
): HomePick {
  const all = deviceTiles(pages);
  const active = all.filter((t) => isActive(t, states));
  const mode = active.length ? "active" : "frequent";
  let ordered = active;
  if (!active.length) {
    const score = (t: TileCfg) => (usage[t.entity] || 0) + (t.light ? usage[t.light] || 0 : 0);
    ordered = all
      .map((t, i) => ({ t, i, s: score(t) }))
      .sort((a, b) => b.s - a.s || a.i - b.i)
      .map((x) => x.t);
  }
  // Fans first so the 2-wide tiles pack into rows cleanly.
  ordered = ordered.filter((t) => t.kind === "fan").concat(ordered.filter((t) => t.kind !== "fan"));
  const tiles: TileCfg[] = [];
  let used = 0;
  ordered.forEach((t) => {
    if (used + cells(t) <= capacity) {
      tiles.push(t);
      used += cells(t);
    }
  });
  return { mode, tiles, more: mode === "active" ? active.length - tiles.length : 0 };
}
