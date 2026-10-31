# Architecture

This document explains what glasspane is made of, how the pieces talk to each
other, and why each part is built the way it is. Most decisions come from one
constraint: **the display may be a 2013 tablet running Chrome 57 with 1 GB of
RAM.**

- [The big picture](#the-big-picture)
- [Tech stack](#tech-stack)
- [Server (Rust)](#server-rust)
  - [Home Assistant client](#home-assistant-client--srchars)
  - [Hub: state cache and fan-out](#hub-state-cache-and-fan-out--srchubrs)
  - [HTTP and WebSocket API](#http-and-websocket-api--srcwebrs)
  - [Frosted-glass backgrounds](#frosted-glass-backgrounds--srcbackgroundrs)
  - [Photo pipeline](#photo-pipeline--srcphotosrs)
  - [Air quality](#air-quality--srcaqirs)
  - [Settings and usage](#settings-and-usage--srcstorers)
- [Frontend (Preact + TypeScript)](#frontend-preact--typescript)
  - [Targeting Chrome 57](#targeting-chrome-57)
  - [Views and components](#views-and-components)
  - [Glass alignment](#glass-alignment)
  - [Animation rules](#animation-rules)
  - [Kiosk mode](#kiosk-mode)
  - [Splash screen](#splash-screen)
- [Wire protocol](#wire-protocol)
- [Security](#security)
- [Testing](#testing)

## The big picture

```text
                       ┌───────────────────────── glasspane server ─────────────────────────┐
┌────────────────┐ WS  │ ha.rs ──▶ hub.rs (state cache, broadcast) ──▶ web.rs ── /ws ───────┼──▶ tablet
│ Home Assistant │◀───▶│   ▲             ▲                              │   ── /api/* ──────┼──▶ phone
└────────────────┘     │   │ commands    │ aqi.rs ◀── Open-Meteo (curl) │   ── /photo/* ────┼──▶ PC browser
                       │   └─────────────┼──────────────────────────────┘   ── static files ─┤
                       │  photos.rs (resize + cache)   background.rs (glass)  store.rs (json)│
                       └────────────────────────────────────────────────────────────────────┘
```

1. The server opens **one** WebSocket to Home Assistant. It authenticates with
   a long-lived token, loads the states, and subscribes to state changes and
   to-do lists.
2. It keeps only the entities that appear in the config, and only the
   attributes their tiles show. It sends those to every connected dashboard
   over the server's own WebSocket (`/ws`).
3. When a user taps a tile, the dashboard sends a small command (`toggle`,
   `speed`, …). The server checks it against the allowlist and turns it into
   a Home Assistant `call_service`.
4. Anything heavy, such as blurring, resizing photos or fetching AQI, happens
   on the server once and is cached. The tablet just shows the results.

## Tech stack

| Layer | Technology | Why |
|---|---|---|
| Server language | **Rust** (edition 2024) | One small native binary. It builds on a phone in Termux, uses little memory, and runs unattended for months. |
| Async runtime | **tokio** | Runs HA, clients, AQI and photo warming as cheap tasks in one process. |
| HTTP / WebSocket server | **axum 0.8** (+ `tower-http` ServeDir) | Typed handlers, built-in WebSocket upgrade, and static file serving. |
| HA client | **tokio-tungstenite** | Plain `ws://` to Home Assistant, which usually runs on the same machine. No TLS stack is needed. |
| Images | **image 0.25** (jpeg, png) | EXIF orientation, Lanczos3 resizing, unsharp mask, Gaussian blur and JPEG encoding, in pure Rust. |
| Config | **toml** + **serde** | Readable hand-edited config, deserialized straight into typed structs. |
| HTTP client (AQI) | system **curl** | Already present in Termux and Docker images, so the Rust build needs no TLS (rustls or OpenSSL). Building TLS crates on a phone is slow and fragile. |
| Logging | **tracing** | Structured logs. On Android they're collected by runit's `svlogger`. |
| UI framework | **Preact 10** | React's API in about 4 KB, with no virtual-DOM-heavy runtime. |
| UI language | **TypeScript** | Catches mistakes in the protocol and settings types. |
| Bundler | **esbuild** (`target=chrome57`) | Rebuilds in milliseconds and lowers modern syntax for old Chrome. |
| Compatibility check | **acorn** | Re-parses the bundle as ES2017 and fails the build on newer syntax (see below). |
| Keep-awake | **NoSleep.js** | Chrome 57 has no Wake Lock API. NoSleep loops a tiny silent video instead. |
| Fake Home Assistant | `src/bin/mock_ha.rs` | A demo home for development, screenshots and end-to-end tests. |
| Packaging | **Docker** (multi-stage) / **Termux + runit** | Docker for PCs, Pis and NAS boxes. A native build plus a supervised service for Android phones. |

## Server (Rust)

`src/main.rs` reads the config and creates the shared pieces. It then starts
three background tasks (HA, AQI, photo warming) and the axum server. All
shared state lives in `AppState`, which is cheap to clone because everything
heavy sits behind an `Arc`.

### Home Assistant client — `src/ha.rs`

- **One connection, many screens.** Each tablet could open its own HA
  connection, but then every screen would need the token, and HA would do
  the work N times. Instead, glasspane connects once and fans updates out.
- **Handshake:**
  1. `auth` with the token.
  2. `get_states`: the initial state of everything.
  3. `get_config`: the home location (used for AQI) and the HA version.
  4. `subscribe_events` with `event_type: state_changed`.
  5. `todo/item/subscribe`, once per to-do list on the dashboard. HA replies
     with an `event` carrying the full `items` list now and after every
     change.
- **Message ids.** HA matches replies to requests by `id`. The fixed requests
  use ids 1–3. To-do subscriptions use `100 + index`, so an incoming event's
  id tells us which list it belongs to without a lookup table. Commands use
  ids from 1000 upward.
- **Commands** arrive from the web layer over an `mpsc` channel and become
  `call_service`:

  | Dashboard command | Home Assistant service |
  |---|---|
  | toggle | `<domain>.toggle` |
  | fan speed > 0 | `fan.set_percentage` |
  | fan speed 0 | `fan.turn_off` |
  | add item | `todo.add_item` |
  | tick / untick | `todo.update_item` (`completed` / `needs_action`) |
  | clear | `todo.remove_completed_items` |

- **Liveness:**
  - The client sends `ping` every 30 seconds, so it notices a dead connection
    even when HA is idle.
  - If the connection drops, it reconnects with exponential backoff, from 1
    to 30 seconds.
  - A rejected token backs off for 60 seconds, because retrying won't fix
    it.

### Hub: state cache and fan-out — `src/hub.rs`

- **Holds** the latest *slimmed* state of each dashboard entity, the to-do
  items, the AQI reading, the HA location and version, and a
  `tokio::broadcast` channel.
- **Slimming.** HA state objects can be several KB each, with every
  attribute, context id and timestamp. `slim()` keeps `state` plus just the
  attributes a tile shows: `friendly_name`, `brightness`,
  `unit_of_measurement`, `device_class`, `icon`, `percentage` and
  `percentage_step`. To-do items are cut down to `{uid, summary, status}`.
- **Dropping no-op updates.** Many devices report attribute churn the
  dashboard doesn't show, such as a changed `last_updated` or power readings.
  After slimming, the hub compares the new state with the cached one. If
  nothing visible changed, nothing is sent. This keeps the tablet's CPU and
  radio quiet.
- **New clients** get one `snapshot` message (everything), then the
  incremental updates from the broadcast channel.

### HTTP and WebSocket API — `src/web.rs`

| Route | Purpose |
|---|---|
| `GET /` and static files | The built frontend from `web_dir` (ServeDir fallback) |
| `GET /api/layout` | Rooms and tiles from the config, plus `shopping_list` |
| `GET/PUT /api/settings` | Dashboard settings, shared by all screens |
| `GET /api/usage` | Per-entity tap counts (for the Home view's "most used") |
| `GET /api/info` | About-page data: version, uptime, HA status and version, location, AQI, photo count |
| `GET /api/photos` | Number of screensaver photos and their folder |
| `GET /photo/{i}?w=&h=&fit=cover\|contain` | Photo *i*, sized for a `w`×`h` pixel screen |
| `GET /photo/{i}/blur` | Small blurred copy (backdrop for portrait photos) |
| `GET /bg.jpg`, `/bg-blur.jpg` | Wallpaper and its pre-blurred "glass" copy |
| `GET /ws` | Live state and commands (see [Wire protocol](#wire-protocol)) |

Request bodies are capped at 16 KB. The server pings each dashboard
WebSocket every 25 seconds, which keeps NAT and Wi-Fi power-save from
silently dropping idle connections.

### Frosted-glass backgrounds — `src/background.rs`

The look depends on frosted glass: tiles that blur what's behind them. The
CSS way is `backdrop-filter: blur()`, but that needs Chrome 76, and on a weak
GPU it would re-blur every frame.

So the server blurs the wallpaper **once** at startup:

- `/bg.jpg` is the sharp wallpaper. It's your `background` image, or a
  generated multi-stop gradient.
- `/bg-blur.jpg` is a small, heavily blurred copy (`blur_sigma`). It's small
  because a blurred image has no fine detail to lose, and the browser scales
  it up smoothly.

The frontend makes each tile show its own slice of the blurred copy (see
[Glass alignment](#glass-alignment)). The result looks like real glass and
costs no GPU work.

### Photo pipeline — `src/photos.rs`

Camera photos are 12+ megapixels and several MB each. Sending them as they
are to a 1024×600 tablet wastes bandwidth and memory. And when an old
browser scales them down, the result is soft and shimmering, because it uses
cheap filtering. So the server makes a version for **each screen size**:

1. **The screen says how big it is.** The screensaver requests
   `/photo/3?w=2048&h=1200&fit=cover`. Those are *device pixels*: CSS size ×
   `devicePixelRatio`, so high-DPI screens get sharp images too.
2. **Sizes are normalized.** `Target::new` rounds each dimension up to a
   multiple of 8 and clamps it to 320–2560. That way, slightly different
   window sizes share a cache entry, and nobody can request a 20000-px
   render.
3. **The photo is turned upright.** The EXIF orientation is applied, because
   phones often store photos sideways.
4. **The output size is chosen:**
   - `cover` fills the screen. The photo is scaled to cover it, and the
     browser crops the edges.
   - `contain` shows the whole photo.
   - A photo whose orientation doesn't match the screen (portrait on a
     landscape tablet) is always shown whole. Cropping it to fill would throw
     away most of the picture.
   - Photos are **never upscaled**.
5. **The photo is resampled** with **Lanczos3**, the sharpest standard
   filter. Then a light **unsharp mask** (σ 0.6) restores the crispness that
   downscaling loses. It's saved as **JPEG quality 90**.
6. **Results are cached** in `<data_dir>/.photo-cache/` as
   `{hash}-{w}x{h}{c|f}.jpg`. The hash covers the file's path, size and
   modification time, so an edited photo gets a new cache entry.
7. **The cache is warmed ahead of time.** The last 4 screen sizes are kept
   in `targets.txt`. A background task (`warm`) renders every photo for
   those sizes at startup, whenever a new size appears, and every 30
   minutes. The screensaver then never waits on a resize.

Portrait photos sit on a blurred, darkened copy of themselves
(`/photo/{i}/blur`, 160 px wide) instead of black bars.

### Air quality — `src/aqi.rs`

- **Source:** the US AQI from the free [Open-Meteo Air Quality
  API](https://open-meteo.com/en/docs/air-quality-api), which needs no API
  key.
- **Location:** Home Assistant's home location (from `get_config`), or
  `aqi_location` in the config.
- **Schedule:** it refreshes every 20 minutes and retries after 2 minutes on
  failure.
- **Fetching:** it uses the system `curl` rather than a Rust HTTP client,
  which keeps TLS out of the build (see the tech stack).
- **Testing override:** the `GLASSPANE_AQI_URL` environment variable
  replaces the endpoint, for testing.

### Settings and usage — `src/store.rs`

Two small JSON files live in `data_dir`:

- `settings.json` is written by the Settings page (`PUT /api/settings`):
  - The page sends only the parts that changed. Those are merged one level
    deep, so changing `screensaver.idleMin` keeps `screensaver.enabled`.
  - Documents are limited to 8 KB.
  - The file is written atomically: to a temp file, then renamed. Losing
    power mid-write can't corrupt it.
  - Settings live on the server, not in `localStorage`, so every screen
    shares them, and they survive the browser's data being cleared. The only
    thing each browser keeps locally is the view it last had open.
- `usage.json` counts how often each entity is toggled. The Home view uses
  it when nothing is on, to show your most-used devices.

## Frontend (Preact + TypeScript)

The frontend source is in `web/src`. `npm run build` writes `web/dist/`
(`index.html`, `app.css`, `app.js`, `manifest.json`). `web/dist` is
committed, so a phone can run the server without installing Node.

### Targeting Chrome 57

esbuild lowers *syntax* (object spread, optional chaining, …) to the target,
but it **cannot add missing runtime APIs**. `web/build.mjs` therefore checks
the output:

1. It re-parses `dist/app.js` with **acorn** at `ecmaVersion: 2017`. Any
   syntax newer than ES2017 fails the build.
2. It scans for APIs Chrome 57 lacks: `globalThis`, `queueMicrotask`,
   `Object.fromEntries`, `.flat()`, `.flatMap()`, `Promise.finally`,
   `ResizeObserver`, `AbortController`, `structuredClone`, `replaceAll`,
   `matchAll`. Each one fails the build with the Chrome version that added
   it.

In the source, this means: use `Object.assign` instead of spread for objects
built at runtime, use regex `trim` fallbacks, and use plain `setTimeout`.
Comments mark these spots.

CSS follows the same rule:

- no `backdrop-filter`, `gap` on flexbox, `aspect-ratio`, `inset` or `:is()`
- grid is used (Chrome 57 was the first version to support it)
- `-webkit-` prefixes are added where Chrome 57 still needed them (keyframes,
  transforms)

### Views and components

| File | Role |
|---|---|
| `main.tsx` | Entry point: installs kiosk mode and renders `<App/>` |
| `App.tsx` | Owns all live state (entities, to-do items, AQI, settings). Routes between Home, Shopping and rooms. Runs the idle timer that starts the screensaver. |
| `conn.ts` | WebSocket client: parses server messages, sends commands, reconnects with backoff and immediately when the screen wakes |
| `home.ts` | Picks the Home view tiles: active devices first, otherwise the most used. Packs them into the grid's capacity (fans first, because they're 2 wide). |
| `Sidebar.tsx` | Clock + AQI, Home/Shopping/rooms with on-counts, connection status, collapse button |
| `FanTile.tsx` | Speed slider (steps from `percentage_step`), power button, light button, spin animation |
| `Tile.tsx` | Light, switch and sensor tiles |
| `TodoTile.tsx` | Shopping list, with optimistic ticks until HA confirms |
| `SettingsPage.tsx` | Settings overlay and About section |
| `Screensaver.tsx` | Photo rotation with cross-fades, Ken Burns zoom, clock overlay, preloading |
| `glass.ts` | Lines up each glass tile's background with the wallpaper |
| `kiosk.ts` | Full screen + keep-awake on the first tap |
| `types.ts` | Shared types, default settings, time and AQI helpers |
| `icons.tsx` | Inline SVG icons (no icon font to download) |

The UI updates **optimistically**: a tap changes the tile at once, then HA's
real `state_changed` event confirms or corrects it. Fan sliders send a speed
only when the drag is released, not for every pixel of movement.

### Glass alignment

Every `.glass` element uses `/bg-blur.jpg` as its background, sized to the
whole viewport. `alignGlass()` sets each element's `background-position` to
minus its own on-screen position. The tile then shows exactly the blurred
version of the wallpaper behind it, so it looks transparent and frosted. This
re-runs on resize and whenever tiles move: on a view change, when the sidebar
collapses or expands, and when new states arrive.

### Animation rules

- **Only `transform` and `opacity` animate.** The compositor handles these
  without layout or repaint. Fan spin, fades, the Ken Burns zoom and the
  splash spinner all follow this rule.
- **Fan spin speed** is driven by `animation-duration`. Changing a fan's
  speed doesn't restart the animation, so the fan doesn't jump.
- **No shadows or filters animate.** Shadows are static, and blur is
  pre-rendered.

### Kiosk mode

Browsers only allow full screen and video playback from a user gesture. The
first tap anywhere therefore:

- enters full screen (`requestFullscreen` / `webkitRequestFullscreen`)
- starts NoSleep's silent looping video, which keeps the screen on while the
  dashboard is visible

That first tap is swallowed, so it doesn't also toggle whatever was under
the finger. Text boxes are the exception, because they still need focus.
`manifest.json` (`display: fullscreen`, `orientation: landscape`) makes a
home-screen shortcut launch straight into full screen. The keep-awake
setting turns NoSleep off.

### Splash screen

The splash is plain HTML and CSS **inline in `index.html`**, so it paints in
the first frame, before `app.js` (or the network) is ready. Its status line
says "Connecting to Home Assistant…", and changes to "Still connecting to the
server…" after 8 seconds. The app fades the splash out once it has both the
layout and the first live snapshot.

## Wire protocol

All messages are JSON over `/ws`, with a `t` field for the type.

**Server → dashboard**

```jsonc
{"t":"snapshot","ha":true,"states":{"fan.bedroom":{"state":"on","a":{"percentage":33,"percentage_step":16.67}}},
 "items":{"todo.shopping_list":[{"uid":"1","summary":"Milk","status":"needs_action"}]},
 "aqi":{"aqi":42,"category":"Good","pm25":9.8,"pm10":21,"source":"Open-Meteo (US AQI)"}}
{"t":"state","id":"light.kitchen","s":{"state":"off","a":{"friendly_name":"Kitchen"}}}
{"t":"removed","id":"light.kitchen"}
{"t":"items","id":"todo.shopping_list","items":[ ... ]}
{"t":"aqi","aqi":{ ... }}
{"t":"ha","connected":false}
```

**Dashboard → server**

```jsonc
{"t":"toggle","id":"light.kitchen"}
{"t":"speed","id":"fan.bedroom","pct":50}          // 0 = off
{"t":"todo_add","id":"todo.shopping_list","text":"Bread"}
{"t":"todo_set","id":"todo.shopping_list","uid":"1","done":true}
{"t":"todo_clear","id":"todo.shopping_list"}
```

## Security

glasspane is designed for a **trusted home network**:

- **The token stays on the server.** It's read from the `HA_TOKEN`
  environment variable and never sent to clients or written to the config.
- **Allowlist.** A client can only:
  - `toggle` entities on light, switch or fan tiles, plus fan lights
  - set the speed of fans on fan tiles
  - edit to-do lists on the dashboard

  Everything else is rejected and logged. A guest's phone on your Wi-Fi
  can't call arbitrary HA services or read entities that aren't on the
  dashboard.
- **Input limits:**
  - 16 KB request bodies
  - 8 KB settings documents
  - 120-character list items and 128-character item ids
  - clamped photo sizes
- **No login of its own.** Anyone who can reach port 8090 can use the
  dashboard. Keep it on your LAN, or reach it remotely through a private
  network such as Tailscale or WireGuard. Don't port-forward it to the
  internet.

## Testing

| What | How |
|---|---|
| Unit tests | `cargo test`: config and allowlist, service-call mapping, to-do routing, attribute slimming, AQI parsing, settings merge, photo sizing and caching |
| Lint | `cargo clippy --all-targets -- -D warnings` and `npm run check` (tsc) |
| Chrome 57 compatibility | `npm run build` (acorn + API scan) |
| End to end | `tests/e2e.mjs` against `mock_ha`: HTTP API, snapshot, every command type, live updates, allowlist rejections |
| Visual | `scripts/screenshots.mjs` (Playwright) regenerates `docs/screenshots/` |

CI runs all of these on every push. See
[`.github/workflows/ci.yml`](../.github/workflows/ci.yml).
