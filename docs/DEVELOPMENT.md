# Development

## Prerequisites

- **Rust** (stable, 1.85+ for edition 2024): <https://rustup.rs>
- **Node 22+**, only to change the frontend or run the end-to-end test. The
  built frontend is committed in `web/dist`.

## Run the demo home

```bash
cargo run --bin mock_ha &              # fake Home Assistant on :8124 (token "dev")
HA_TOKEN=dev cargo run -- demo.toml    # dashboard on http://localhost:8090
```

The mock (`src/bin/mock_ha.rs`) speaks the parts of the Home Assistant
WebSocket API that glasspane uses:

- auth, `get_states`, `get_config` and `state_changed` events
- `call_service` for toggles, fan speeds and to-do lists
- `todo/item/subscribe`

Its home has four rooms of fans, lights, switches and sensors, plus a
shopping list. The living-room temperature drifts, so you can watch live
updates arrive.

## Frontend

```bash
cd web
npm install
npm run dev      # rebuild web/dist on every change (with source maps)
npm run build    # production build + Chrome 57 compatibility check
npm run check    # TypeScript type check
```

The server serves `web/dist` straight from disk, so reload the browser after
a rebuild. **Commit `web/dist` with your change.** Phones run the server
without Node, and CI fails if `web/dist` doesn't match a fresh build.

**Rules for the frontend:**

- **Chrome 57 is the target.** `npm run build` fails on syntax newer than
  ES2017, or on known newer APIs.
  - Don't use object spread on runtime objects (use `Object.assign`), and
    don't use optional chaining.
  - Don't use `Array.prototype.flat`, `Promise.finally`, `globalThis` and
    the like.
- **No `backdrop-filter`, flexbox `gap`, `aspect-ratio` or `inset` in CSS.**
  Use the `.glass` class for frosted panels.
- **Only animate `transform` and `opacity`.**
- **Check it on a slow device** when you can. Chrome DevTools' 6× CPU
  throttling is a fair stand-in for a 2013 tablet.

## Tests

```bash
cargo test                                  # unit tests
cargo clippy --all-targets -- -D warnings   # lint
cargo fmt --check                           # formatting

# end to end: mock HA + server + a scripted "tablet"
cargo run --bin mock_ha &
HA_TOKEN=dev cargo run -- demo.toml &
node tests/e2e.mjs
```

`tests/e2e.mjs` checks:

- the HTTP API, the first snapshot and live updates
- every command type: toggle, fan speed, and adding, ticking and clearing
  shopping-list items
- that the server **rejects** commands for entities that aren't on the
  dashboard

It exits non-zero on any failure.

## Screenshots and demo photos

```bash
python3 scripts/make-demo-photos.py           # regenerate demo/photos (numpy + Pillow)
npm i --no-save playwright && npx playwright install chromium
node scripts/screenshots.mjs docs/screenshots # with mock_ha + demo server running
```

The script captures PNGs at 1024×600 and 2× density. The README uses JPEG
copies, which are much smaller.

The demo photos are generated landscapes, painted from noise. That way the
repo ships no one's photographs and no licensing questions.

## Project layout

```text
src/
  main.rs          startup: config, tasks, server
  config.rs        glasspane.toml + the control allowlist
  ha.rs            Home Assistant WebSocket client
  hub.rs           state cache, slimming, broadcast to dashboards
  web.rs           HTTP routes + dashboard WebSocket
  background.rs    wallpaper + pre-blurred glass copy
  photos.rs        screensaver photos: orient, resize per screen, cache
  aqi.rs           air quality from Open-Meteo
  store.rs         settings.json + usage.json
  bin/mock_ha.rs   fake Home Assistant (demo home)
web/
  src/             Preact + TypeScript UI (see docs/ARCHITECTURE.md)
  static/          index.html (with inline splash), app.css, manifest.json
  build.mjs        esbuild + Chrome 57 checks
  dist/            built frontend (committed)
tests/e2e.mjs      end-to-end test
scripts/           Termux install/update, demo photos, screenshots
demo.toml          demo home config        demo/photos/  demo screensaver photos
docs/              documentation + screenshots
```

## Logging

Set `RUST_LOG=glasspane=debug` to see every message exchanged with Home
Assistant and every rejected client command.

## Releasing

1. Bump `version` in `Cargo.toml` (and `web/package.json`).
2. Add a `## <version>` section at the top of `CHANGELOG.md`.
3. Push to `main`. The Release workflow sees the new version, tags
   `v<version>`, and publishes a GitHub release. The release notes come from
   the changelog, and a zip is attached that `scripts/update-termux.sh`
   can install.
