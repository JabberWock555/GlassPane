# Configuration

glasspane reads one TOML file. It's given as the first argument, and defaults
to `glasspane.toml` in the current directory:

```bash
HA_TOKEN=<long-lived token> glasspane /path/to/glasspane.toml
```

Start from [`glasspane.example.toml`](../glasspane.example.toml), or look at
[`demo.toml`](../demo.toml) for a complete home.

## Environment variables

| Variable | Required | Meaning |
|---|---|---|
| `HA_TOKEN` | yes | Home Assistant long-lived access token (Profile → Security → Long-lived access tokens). Kept out of the config so the config can be shared or committed safely. |
| `RUST_LOG` | no | Log filter, e.g. `glasspane=debug`. Default: `glasspane=info`. |
| `GLASSPANE_AQI_URL` | no | Replace the Open-Meteo endpoint (for tests or a proxy). |

## Top-level options

| Key | Default | Meaning |
|---|---|---|
| `listen` | `"0.0.0.0:8090"` | Address and port to serve the dashboard on. |
| `ha_url` | `"ws://127.0.0.1:8123/api/websocket"` | Home Assistant WebSocket API. Use `wss://` only if your Rust build has TLS. The usual setup runs glasspane on the same machine as HA. |
| `web_dir` | `"web/dist"` | The built frontend. |
| `shopping_list` | none | A to-do entity, e.g. `"todo.shopping_list"`. It adds the **Shopping** section to the sidebar and shows the list on **Home**. |
| `photos_dir` | `"/sdcard/Pictures/Glasspane"` | Folder of `.jpg`/`.jpeg`/`.png` screensaver photos. Subfolders aren't scanned. |
| `data_dir` | `"."` | Where `settings.json`, `usage.json` and `.photo-cache/` are kept. Point it at a volume in Docker. |
| `background` | none | Wallpaper image (jpg/png). Without it, a soft gradient is generated. |
| `blur_sigma` | `22.0` | Strength of the frosted-glass blur. |
| `aqi_location` | HA's home | `[latitude, longitude]` for the air-quality reading. |

## Rooms: `[[pages]]`

Each `[[pages]]` entry is a room in the sidebar.

```toml
[[pages]]
title = "Living Room"
icon = "sofa"          # home (default), sofa, bed, dining, fan, cart, bulb, …
tiles = [ ... ]
```

A room is a **4 × 2 grid**. Tiles fill it left to right, top to bottom. Most
tiles take one cell; fan tiles take two (2 × 1).

## Tiles

```toml
{ entity = "light.floor_lamp", kind = "light", name = "Floor lamp", icon = "bulb", cols = 1, rows = 1 }
```

| Field | Meaning |
|---|---|
| `entity` | The Home Assistant entity id. |
| `kind` | `light`, `switch`, `sensor`, `fan` or `todo` (below). |
| `name` | Label. Defaults to the entity's `friendly_name`. |
| `icon` | `bulb`, `power`, `moon`, `fan`, `thermo`, `drop`, `gauge`, `cart`, `home`, `sofa`, `bed`, `dining`. Defaults depend on the kind. |
| `cols`, `rows` | Size in grid cells (cols 1–4, rows 1–2). |
| `light` | **Fans only:** the fan's built-in light, shown as a button on the fan tile. |

### Kinds

- **`light` / `switch`** — tap to toggle. A light shows its brightness when HA
  reports it.
- **`sensor`** — read only. Shows the value and unit. Temperature and
  humidity get matching icons from their `device_class`.
- **`fan`** — the tile has:
  - a speed slider, from *Off* to *N* steps, where N comes from HA's
    `percentage_step` (e.g. 6 speeds for a 16.67 % step). Releasing the
    slider calls `fan.set_percentage`; dragging it to Off calls
    `fan.turn_off`.
  - a power button that toggles the fan
  - an optional light button (`light = "light.…"`), for fans with a
    built-in light, so the light doesn't need a tile of its own
- **`todo`** — any HA to-do list. Tap an item to tick it, add items from the
  box, and use *Clear N* to remove ticked ones. Changes made in the HA app
  appear live. The tile defaults to 2 × 2; use `rows = 1` for a compact list.

## Built-in sections

- **Home** is always first:
  - It shows the devices that are **on right now**, from every room.
  - When nothing is on, it shows your **most-used** devices instead. Each
    toggle and speed change is counted in `usage.json`.
  - The shopping list sits next to them, if `shopping_list` is set.
  - Tiles that don't fit are counted in the header ("2 more in rooms").
- **Shopping** appears when `shopping_list` is set. It shows the list at
  full size.

## Only dashboard entities are controllable

The config is also the security allowlist. The server only relays commands
for entities that appear in it:

- toggles for light, switch and fan tiles, plus their `light`
- speeds for fan tiles
- edits for to-do lists

Entities that aren't on the dashboard aren't even sent to the browser. To
control something new from the tablet, add a tile for it.

## Settings page (stored on the server)

These settings are changed on the dashboard itself (the gear, top right),
not in the TOML file. They're saved in `<data_dir>/settings.json` and shared
by every screen:

| Setting | Default |
|---|---|
| Photo screensaver | off |
| Start after (idle) | 5 min (1, 2, 5, 10 or 15) |
| Change photo every | 20 s (10, 20, 30 or 60) |
| Photo fit | *Fill screen*, or *Whole photo* (never crop). Portrait photos are always shown whole. |
| Clock on photos | on |
| 24-hour clock | on |
| Show AQI | on |
| Keep screen awake | on |
| Sidebar collapsed | off |

Deleting `settings.json` restores the defaults.

## Running behind Docker

The image's working directory is `/app`. Mount your config there, and put
data and photos on volumes:

```toml
# glasspane.toml (inside the container)
ha_url = "ws://homeassistant.local:8123/api/websocket"
data_dir = "/data"
photos_dir = "/photos"
```

```bash
docker run -d -p 8090:8090 -e HA_TOKEN=... \
  -v $PWD/glasspane.toml:/app/glasspane.toml \
  -v $PWD/data:/data -v $HOME/Pictures/wall:/photos glasspane
```
