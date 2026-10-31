//! Small JSON files kept next to the server: dashboard settings (changed from
//! the tablet's Settings page) and per-device usage counts (for the Home
//! view's "frequently used" fallback).

use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tracing::warn;

/// Largest settings document accepted from a client.
pub const MAX_SETTINGS_BYTES: usize = 8 * 1024;

pub struct Store {
    settings_path: PathBuf,
    usage_path: PathBuf,
    settings: Mutex<Value>,
    usage: Mutex<HashMap<String, u64>>,
}

impl Store {
    pub fn open(dir: &Path) -> Self {
        let _ = std::fs::create_dir_all(dir);
        let settings_path = dir.join("settings.json");
        let usage_path = dir.join("usage.json");
        let settings = read_json(&settings_path)
            .filter(Value::is_object)
            .unwrap_or_else(|| json!({}));
        let usage = read_json(&usage_path)
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();
        Store {
            settings_path,
            usage_path,
            settings: Mutex::new(settings),
            usage: Mutex::new(usage),
        }
    }

    pub fn settings(&self) -> Value {
        self.settings.lock().unwrap().clone()
    }

    /// Merge a partial settings object (one level deep for nested groups).
    pub fn update_settings(&self, patch: &Map<String, Value>) -> anyhow::Result<Value> {
        let mut s = self.settings.lock().unwrap();
        let obj = s.as_object_mut().expect("settings is an object");
        for (k, v) in patch {
            match (obj.get_mut(k), v) {
                (Some(Value::Object(cur)), Value::Object(new)) => {
                    for (k2, v2) in new {
                        cur.insert(k2.clone(), v2.clone());
                    }
                }
                _ => {
                    obj.insert(k.clone(), v.clone());
                }
            }
        }
        let text = serde_json::to_string_pretty(&*s)?;
        if text.len() > MAX_SETTINGS_BYTES {
            anyhow::bail!("settings too large");
        }
        write_atomic(&self.settings_path, &text)?;
        Ok(s.clone())
    }

    /// Count one use of a device (a tap from the dashboard).
    pub fn bump(&self, entity: &str) {
        let snapshot = {
            let mut u = self.usage.lock().unwrap();
            *u.entry(entity.to_string()).or_insert(0) += 1;
            serde_json::to_string(&*u).unwrap_or_default()
        };
        if let Err(e) = write_atomic(&self.usage_path, &snapshot) {
            warn!("could not save usage: {e}");
        }
    }

    pub fn usage(&self) -> Value {
        json!(&*self.usage.lock().unwrap())
    }
}

fn read_json(path: &Path) -> Option<Value> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

/// Write via a temp file + rename so a crash never leaves a half file.
fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_merge_and_usage_persist() {
        let dir = std::env::temp_dir().join(format!("gp-store-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let s = Store::open(&dir);
        let patch = json!({"screensaver": {"enabled": true, "idle_min": 5}, "clock24": false});
        s.update_settings(patch.as_object().unwrap()).unwrap();
        let patch2 = json!({"screensaver": {"idle_min": 2}});
        s.update_settings(patch2.as_object().unwrap()).unwrap();
        s.bump("fan.a");
        s.bump("fan.a");

        let reopened = Store::open(&dir);
        let st = reopened.settings();
        assert_eq!(st["screensaver"]["enabled"], true); // kept by the nested merge
        assert_eq!(st["screensaver"]["idle_min"], 2);
        assert_eq!(reopened.usage()["fan.a"], 2);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
