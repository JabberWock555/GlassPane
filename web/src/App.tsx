// The dashboard shell: owns all live state (entity states, shopping list,
// AQI, settings), routes between Home / Shopping / room views, and shows the
// settings page and screensaver on top. Tiles are plain components that get
// their state and callbacks from here.

import { useEffect, useRef, useState } from "preact/hooks";
import { Conn, EState, ServerMsg, TodoItem } from "./conn";
import { FanTile } from "./FanTile";
import { alignGlass } from "./glass";
import { isActive, pickHome, deviceTiles } from "./home";
import { Gear } from "./icons";
import { setKeepAwake } from "./kiosk";
import { Screensaver } from "./Screensaver";
import { SettingsPage } from "./SettingsPage";
import { Sidebar, View } from "./Sidebar";
import { TileView } from "./Tile";
import { TodoTile } from "./TodoTile";
import { Aqi, DEFAULT_SETTINGS, Layout, mergeSettings, PageCfg, saveSettings, Settings, TileCfg } from "./types";


type StateMap = { [id: string]: EState };

/** How long a tap shows as "pending" if HA never answers. */
const PENDING_MS = 4000;
const VIEW_KEY = "glasspane.view";

function savedView(): View {
  try {
    const v = JSON.parse(localStorage.getItem(VIEW_KEY) || "null");
    if (v && (v.kind === "home" || v.kind === "shopping" || (v.kind === "room" && typeof v.index === "number"))) return v;
  } catch (e) {
    /* ignore */
  }
  return { kind: "home" };
}

function hideSplash(status?: string) {
  const el = document.getElementById("splash");
  if (!el) return;
  if (status) {
    const s = document.getElementById("splash-status");
    if (s) s.textContent = status;
    return;
  }
  el.className = "hide";
  setTimeout(() => el.parentNode && el.parentNode.removeChild(el), 700);
}

export function App() {
  const [layout, setLayout] = useState<Layout | null>(null);
  const [states, setStates] = useState<StateMap>({});
  const [lists, setLists] = useState<{ [id: string]: TodoItem[] }>({});
  const [aqi, setAqi] = useState<Aqi | null>(null);
  const [link, setLink] = useState(false);
  const [ha, setHa] = useState(false);
  const [gotSnapshot, setGotSnapshot] = useState(false);
  const [pending, setPending] = useState<{ [id: string]: boolean }>({});
  const [view, setViewState] = useState<View>(savedView);
  const [settings, setSettings] = useState<Settings>(DEFAULT_SETTINGS);
  const [usage, setUsage] = useState<{ [id: string]: number }>({});
  const [photoCount, setPhotoCount] = useState(0);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [saver, setSaver] = useState(false);
  const conn = useRef<Conn | null>(null);
  const lastActivity = useRef(Date.now());

  // ---- startup ------------------------------------------------------------
  useEffect(() => {
    fetch("/api/layout")
      .then((r) => r.json())
      .then((l) => setLayout({ pages: l.pages || [], shopping_list: l.shopping_list }))
      .catch(() => setLayout({ pages: [] }));
    fetch("/api/settings").then((r) => r.json()).then((s) => setSettings(mergeSettings(s))).catch(() => {});
    fetch("/api/usage").then((r) => r.json()).then(setUsage).catch(() => {});
    fetch("/api/photos").then((r) => r.json()).then((p) => setPhotoCount(p.count || 0)).catch(() => {});

    const onMsg = (m: ServerMsg) => {
      if (m.t === "snapshot") {
        setStates(m.states);
        setLists(m.items || {});
        setAqi(m.aqi && typeof m.aqi.aqi === "number" ? m.aqi : null);
        setHa(m.ha);
        setPending({});
        setGotSnapshot(true);
      } else if (m.t === "items") {
        setLists((prev) => Object.assign({}, prev, { [m.id]: m.items }));
      } else if (m.t === "aqi") {
        setAqi(m.aqi && typeof m.aqi.aqi === "number" ? m.aqi : null);
      } else if (m.t === "state") {
        setStates((prev) => Object.assign({}, prev, { [m.id]: m.s }));
        setPending((prev) => without(prev, m.id));
      } else if (m.t === "removed") {
        setStates((prev) => without(prev, m.id));
      } else if (m.t === "ha") {
        setHa(m.connected);
      }
    };
    conn.current = new Conn(onMsg, setLink);

    const slow = setTimeout(() => hideSplash("Still connecting to the server…"), 8000);
    const realign = () => alignGlass(document.body);
    const touch = () => (lastActivity.current = Date.now());
    window.addEventListener("resize", realign);
    document.addEventListener("touchstart", touch, true);
    document.addEventListener("mousedown", touch, true);
    document.addEventListener("keydown", touch, true);
    return () => {
      clearTimeout(slow);
      window.removeEventListener("resize", realign);
    };
  }, []);

  // Splash goes away once we have the layout and the first live snapshot.
  useEffect(() => {
    if (layout && gotSnapshot) hideSplash();
  }, [layout, gotSnapshot]);

  useEffect(() => setKeepAwake(settings.keepAwake), [settings.keepAwake]);

  // ---- screensaver idle timer -------------------------------------------------
  useEffect(() => {
    const t = setInterval(() => {
      const ss = settings.screensaver;
      if (!ss.enabled || saver || settingsOpen) return;
      if (Date.now() - lastActivity.current > ss.idleMin * 60000) setSaver(true);
    }, 5000);
    return () => clearInterval(t);
  }, [settings, saver, settingsOpen]);

  // Re-align the frosted glass whenever the visible tiles move.
  const collapsed = settings.sidebarCollapsed;
  useEffect(() => {
    const t = setTimeout(() => alignGlass(document.body), 0);
    return () => clearTimeout(t);
  }, [view, layout, collapsed, states]);

  // ---- actions ------------------------------------------------------------
  const setView = (v: View) => {
    setViewState(v);
    try {
      localStorage.setItem(VIEW_KEY, JSON.stringify(v));
    } catch (e) {
      /* private mode: ignore */
    }
    if (v.kind === "home") fetch("/api/usage").then((r) => r.json()).then(setUsage).catch(() => {});
  };

  const changeSettings = (patch: any) => {
    // Optimistic: apply locally, then persist on the server.
    setSettings((prev) => {
      const next: any = Object.assign({}, prev, patch);
      if (patch.screensaver) next.screensaver = Object.assign({}, prev.screensaver, patch.screensaver);
      return next;
    });
    saveSettings(patch).then((s) => setSettings(mergeSettings(s))).catch(() => {});
  };

  const markPending = (id: string) => {
    setPending((prev) => Object.assign({}, prev, { [id]: true }));
    setTimeout(() => setPending((prev) => without(prev, id)), PENDING_MS);
  };
  const toggle = (id: string) => {
    if (conn.current && conn.current.toggle(id)) markPending(id);
  };
  const speed = (id: string, pct: number) => {
    if (conn.current) conn.current.speed(id, pct);
  };
  const todoAdd = (id: string, text: string) => !!conn.current && conn.current.todoAdd(id, text);
  const todoSet = (id: string, uid: string, done: boolean) => !!conn.current && conn.current.todoSet(id, uid, done);
  const todoClear = (id: string) => !!conn.current && conn.current.todoClear(id);

  // ---- rendering helpers --------------------------------------------------
  const pages = layout ? layout.pages : [];
  const shopping = layout && layout.shopping_list ? layout.shopping_list : null;
  const status = !link ? "Offline" : !ha ? "Home Assistant unavailable" : "";
  const openItems = shopping && lists[shopping] ? lists[shopping].filter((i) => i.status !== "completed").length : 0;

  const renderTile = (t: TileCfg) =>
    t.kind === "todo" ? (
      <TodoTile key={t.entity} cfg={t} st={states[t.entity]} items={lists[t.entity]} onAdd={todoAdd} onSet={todoSet} onClear={todoClear} />
    ) : t.kind === "fan" ? (
      <FanTile key={t.entity} cfg={t} fan={states[t.entity]} light={t.light ? states[t.light] : undefined}
        pending={pending} onToggle={toggle} onSpeed={speed} />
    ) : (
      <TileView key={t.entity} cfg={t} st={states[t.entity]} pending={!!pending[t.entity]} onTap={toggle} />
    );

  let title = "";
  let subtitle = "";
  let tiles: TileCfg[] = [];
  let viewKey = "";
  if (view.kind === "room" && pages.length) {
    const i = Math.min(view.index, pages.length - 1);
    title = pages[i].title;
    tiles = pages[i].tiles;
    viewKey = "room" + i;
  } else if (view.kind === "shopping" && shopping) {
    title = "Shopping";
    subtitle = openItems ? openItems + " to buy" : "";
    tiles = [{ entity: shopping, kind: "todo", name: "Shopping list", cols: 4, rows: 2 }];
    viewKey = "shopping";
  } else {
    const pick = pickHome(pages, states, usage, shopping ? 4 : 8);
    title = "Home";
    subtitle =
      pick.mode === "active"
        ? pick.tiles.length + pick.more + " on now" + (pick.more ? " · " + pick.more + " more in rooms" : "")
        : "Nothing on · your most used";
    tiles = (shopping ? [{ entity: shopping, kind: "todo", name: "Shopping list", cols: 2, rows: 2 } as TileCfg] : []).concat(pick.tiles);
    viewKey = "home";
  }

  const homeBadge = deviceTiles(pages).filter((t) => isActive(t, states)).length;
  const roomBadge = (p: PageCfg) =>
    p.tiles.filter((t) => (t.kind === "fan" || t.kind === "switch" || t.kind === "light") && isActive(t, states)).length;

  return (
    <div class={"app" + (collapsed ? " side-collapsed" : "")}>
      <Sidebar
        pages={pages}
        view={view}
        onView={setView}
        collapsed={collapsed}
        onCollapse={(c) => changeSettings({ sidebarCollapsed: c })}
        hasShopping={!!shopping}
        homeBadge={homeBadge}
        shoppingBadge={openItems}
        roomBadge={roomBadge}
        h24={settings.clock24}
        aqi={aqi}
        showAqi={settings.showAqi}
        status={status}
      />

      <main class="main">
        <div class="main-head">
          <h1 class="room-title">{title}</h1>
          {subtitle && <span class="room-sub">{subtitle}</span>}
          <button class="round-btn gear" aria-label="Settings" onClick={() => setSettingsOpen(true)}>
            <Gear size={24} />
          </button>
        </div>
        {layout && (
          <div class="room-view" key={viewKey}>
            <section class="grid">{tiles.map(renderTile)}</section>
          </div>
        )}
      </main>

      {settingsOpen && (
        <SettingsPage
          settings={settings}
          onChange={changeSettings}
          onClose={() => setSettingsOpen(false)}
          onPreviewScreensaver={() => {
            setSettingsOpen(false);
            setSaver(true);
          }}
        />
      )}

      {saver && (
        <Screensaver
          count={photoCount}
          intervalSec={settings.screensaver.intervalSec}
          showClock={settings.screensaver.showClock}
          fit={settings.screensaver.fit}
          h24={settings.clock24}
          aqi={settings.showAqi ? aqi : null}
          onExit={() => {
            lastActivity.current = Date.now();
            setSaver(false);
            fetch("/api/photos").then((r) => r.json()).then((p) => setPhotoCount(p.count || 0)).catch(() => {});
          }}
        />
      )}
    </div>
  );
}

function without<T>(obj: { [k: string]: T }, key: string): { [k: string]: T } {
  if (!(key in obj)) return obj;
  const next = Object.assign({}, obj);
  delete next[key];
  return next;
}
