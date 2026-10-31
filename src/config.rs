//! Configuration file (`glasspane.toml`): which Home Assistant to talk to,
//! the rooms and tiles to show, and optional features (shopping list,
//! screensaver photos, AQI location).
//!
//! The config also defines the *allowlist*: a client can only control
//! entities that appear on the dashboard, and only in ways their tile allows
//! (see `toggleable`, `fans`, `todos`). This matters because anyone on the
//! network can open the dashboard, but they should not be able to drive
//! arbitrary Home Assistant services through it.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

/// Top-level dashboard configuration, loaded from `glasspane.toml`.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// Address the dashboard server listens on, e.g. "0.0.0.0:8090".
    #[serde(default = "default_listen")]
    pub listen: String,
    /// Home Assistant WebSocket API URL.
    #[serde(default = "default_ha_url")]
    pub ha_url: String,
    /// Optional background image (jpg/png). A gradient is generated if missing.
    #[serde(default)]
    pub background: Option<String>,
    /// Blur strength for the "glass" background.
    #[serde(default = "default_blur")]
    pub blur_sigma: f32,
    /// Directory holding the built frontend (index.html, app.js, app.css).
    #[serde(default = "default_web_dir")]
    pub web_dir: String,
    /// To-do list shown as the sidebar's "Shopping" section and on Home.
    #[serde(default)]
    pub shopping_list: Option<String>,
    /// Folder of photos for the screensaver (jpg/png).
    #[serde(default = "default_photos_dir")]
    pub photos_dir: String,
    /// Where settings.json, usage.json and the photo cache are kept.
    #[serde(default = "default_data_dir")]
    pub data_dir: String,
    /// Override the AQI location [latitude, longitude]; defaults to HA's home.
    #[serde(default)]
    pub aqi_location: Option<[f64; 2]>,
    #[serde(default)]
    pub pages: Vec<Page>,
}

fn default_photos_dir() -> String {
    "/sdcard/Pictures/Glasspane".into()
}
fn default_data_dir() -> String {
    ".".into()
}

/// A page is a room in the sidebar.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Page {
    pub title: String,
    /// Sidebar icon: home, dining, sofa, bed (default: home).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default)]
    pub tiles: Vec<Tile>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Tile {
    pub entity: String,
    pub kind: TileKind,
    /// Optional display name; falls back to HA friendly_name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// For fans: the fan's built-in light, shown as a button on the fan tile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub light: Option<String>,
    /// Icon override: bulb, power, moon, fan, thermo, drop, cart.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Grid rows the tile spans (to-do lists default to 2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rows: Option<u8>,
    /// Grid columns the tile spans, 1–4 (fans and lists default to 2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cols: Option<u8>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TileKind {
    Light,
    Switch,
    Sensor,
    Fan,
    /// A to-do list such as the shopping list.
    Todo,
}

impl TileKind {
    /// Whether the tablet is allowed to toggle this kind of entity.
    pub fn toggleable(self) -> bool {
        matches!(self, TileKind::Light | TileKind::Switch | TileKind::Fan)
    }
}

fn default_listen() -> String {
    "0.0.0.0:8090".into()
}
fn default_ha_url() -> String {
    "ws://127.0.0.1:8123/api/websocket".into()
}
fn default_blur() -> f32 {
    22.0
}
fn default_web_dir() -> String {
    "web/dist".into()
}

impl Config {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("reading {}: {e}", path.display()))?;
        let cfg: Config = toml::from_str(&text)?;
        Ok(cfg)
    }

    fn tiles(&self) -> impl Iterator<Item = &Tile> {
        self.pages.iter().flat_map(|p| p.tiles.iter())
    }

    /// Every entity referenced by the layout (including fan lights).
    /// Only these are cached and sent to clients.
    pub fn entities(&self) -> HashSet<String> {
        self.tiles()
            .flat_map(|t| std::iter::once(t.entity.clone()).chain(t.light.clone()))
            .chain(self.shopping_list.clone())
            .collect()
    }

    /// Entities the client may toggle (allowlist enforced server-side).
    pub fn toggleable(&self) -> HashSet<String> {
        self.tiles()
            .filter(|t| t.kind.toggleable())
            .flat_map(|t| std::iter::once(t.entity.clone()).chain(t.light.clone()))
            .collect()
    }

    /// Fans whose speed the client may set.
    pub fn fans(&self) -> HashSet<String> {
        self.of_kind(TileKind::Fan).into_iter().collect()
    }

    /// To-do lists on the dashboard, deduplicated, in first-seen order
    /// (the order defines their subscription ids).
    pub fn todos(&self) -> Vec<String> {
        let mut out: Vec<String> = self.shopping_list.iter().cloned().collect();
        for t in self.of_kind(TileKind::Todo) {
            if !out.contains(&t) {
                out.push(t);
            }
        }
        out
    }

    fn of_kind(&self, kind: TileKind) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for t in self.tiles().filter(|t| t.kind == kind) {
            if !out.contains(&t.entity) {
                out.push(t.entity.clone());
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_example_and_builds_allowlist() {
        let cfg: Config = toml::from_str(
            r#"
            [[pages]]
            title = "Living"
            tiles = [
              { entity = "light.ceiling", kind = "light" },
              { entity = "sensor.temp", kind = "sensor", name = "Temp" },
            ]
            "#,
        )
        .unwrap();
        assert_eq!(cfg.listen, "0.0.0.0:8090");
        assert!(cfg.entities().contains("sensor.temp"));
        assert!(cfg.toggleable().contains("light.ceiling"));
        assert!(!cfg.toggleable().contains("sensor.temp"));
    }

    /// The shipped configs must always parse (they are what new users copy).
    #[test]
    fn shipped_configs_parse() {
        for text in [
            include_str!("../glasspane.example.toml"),
            include_str!("../demo.toml"),
        ] {
            let cfg: Config = toml::from_str(text).unwrap();
            assert!(!cfg.pages.is_empty());
            assert_eq!(cfg.todos()[0], "todo.shopping_list");
        }
    }

    #[test]
    fn fan_light_is_tracked_and_toggleable() {
        let cfg: Config = toml::from_str(
            r#"
            [[pages]]
            title = "Dining"
            icon = "dining"
            tiles = [{ entity = "fan.a", kind = "fan", light = "light.a_led" }]
            "#,
        )
        .unwrap();
        assert!(cfg.entities().contains("light.a_led"));
        assert!(cfg.toggleable().contains("light.a_led"));
        assert!(cfg.toggleable().contains("fan.a"));
        assert!(cfg.fans().contains("fan.a"));
        assert!(!cfg.fans().contains("light.a_led"));
    }
}
