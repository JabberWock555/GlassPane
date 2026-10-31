//! Shared in-memory state: the latest HA state of every entity on the
//! dashboard, plus a broadcast channel that fans updates out to tablets.

use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet};
use std::sync::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::broadcast;

#[derive(Debug, Clone)]
pub enum Update {
    State {
        id: String,
        state: Value,
    },
    Removed {
        id: String,
    },
    Ha {
        connected: bool,
    },
    /// Full item list of a to-do list (shopping list).
    Items {
        id: String,
        items: Value,
    },
    /// Latest air quality reading.
    Aqi {
        aqi: Value,
    },
}

/// Home location reported by Home Assistant (`get_config`).
#[derive(Debug, Clone)]
pub struct Location {
    pub lat: f64,
    pub lon: f64,
    pub name: String,
}

impl Update {
    /// Wire format sent to dashboard clients.
    pub fn to_json(&self) -> String {
        match self {
            Update::State { id, state } => json!({"t": "state", "id": id, "s": state}),
            Update::Removed { id } => json!({"t": "removed", "id": id}),
            Update::Ha { connected } => json!({"t": "ha", "connected": connected}),
            Update::Items { id, items } => json!({"t": "items", "id": id, "items": items}),
            Update::Aqi { aqi } => json!({"t": "aqi", "aqi": aqi}),
        }
        .to_string()
    }
}

/// Keep only what the widget shows: `{uid, summary, status}`.
pub fn slim_items(items: &Value) -> Value {
    let list = items.as_array().map(|v| v.as_slice()).unwrap_or(&[]);
    Value::Array(
        list.iter()
            .map(|i| json!({"uid": i["uid"], "summary": i["summary"], "status": i["status"]}))
            .collect(),
    )
}

pub struct Hub {
    pub location: RwLock<Option<Location>>,
    pub ha_version: RwLock<String>,
    aqi: RwLock<Value>,
    items: RwLock<HashMap<String, Value>>,
    wanted: HashSet<String>,
    states: RwLock<HashMap<String, Value>>,
    connected: AtomicBool,
    tx: broadcast::Sender<Update>,
}

/// Attributes the dashboard actually uses; everything else is dropped to
/// keep messages tiny for the old tablet.
const KEEP_ATTRS: &[&str] = &[
    "friendly_name",
    "brightness",
    "unit_of_measurement",
    "device_class",
    "icon",
    "percentage",
    "percentage_step",
];

/// Reduce a full HA state object to `{state, a: {...}}`.
pub fn slim(ha_state: &Value) -> Value {
    let mut attrs = Map::new();
    if let Some(src) = ha_state.get("attributes").and_then(Value::as_object) {
        for key in KEEP_ATTRS {
            if let Some(v) = src.get(*key) {
                attrs.insert((*key).to_string(), v.clone());
            }
        }
    }
    json!({
        "state": ha_state.get("state").cloned().unwrap_or(Value::Null),
        "a": attrs,
    })
}

impl Hub {
    pub fn new(wanted: HashSet<String>) -> Self {
        let (tx, _) = broadcast::channel(256);
        Hub {
            location: RwLock::new(None),
            ha_version: RwLock::new(String::new()),
            aqi: RwLock::new(Value::Null),
            items: RwLock::new(HashMap::new()),
            wanted,
            states: RwLock::new(HashMap::new()),
            connected: AtomicBool::new(false),
            tx,
        }
    }

    pub fn wants(&self, entity_id: &str) -> bool {
        self.wanted.contains(entity_id)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Update> {
        self.tx.subscribe()
    }

    pub fn set_state(&self, id: &str, full_state: &Value) {
        if !self.wants(id) {
            return;
        }
        let s = slim(full_state);
        {
            let mut map = self.states.write().unwrap();
            if map.get(id) == Some(&s) {
                return; // attribute churn we don't display; skip broadcast
            }
            map.insert(id.to_string(), s.clone());
        }
        let _ = self.tx.send(Update::State {
            id: id.to_string(),
            state: s,
        });
    }

    pub fn set_aqi(&self, aqi: Value) {
        *self.aqi.write().unwrap() = aqi.clone();
        let _ = self.tx.send(Update::Aqi { aqi });
    }

    pub fn aqi(&self) -> Value {
        self.aqi.read().unwrap().clone()
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    pub fn set_items(&self, id: &str, items: &Value) {
        if !self.wants(id) {
            return;
        }
        let slim = slim_items(items);
        self.items
            .write()
            .unwrap()
            .insert(id.to_string(), slim.clone());
        let _ = self.tx.send(Update::Items {
            id: id.to_string(),
            items: slim,
        });
    }

    pub fn remove(&self, id: &str) {
        if self.states.write().unwrap().remove(id).is_some() {
            let _ = self.tx.send(Update::Removed { id: id.to_string() });
        }
    }

    pub fn set_connected(&self, connected: bool) {
        if self.connected.swap(connected, Ordering::SeqCst) != connected {
            let _ = self.tx.send(Update::Ha { connected });
        }
    }

    /// Full snapshot for a newly connected (or lagging) client.
    pub fn snapshot_json(&self) -> String {
        let states = self.states.read().unwrap();
        let items = self.items.read().unwrap();
        json!({
            "t": "snapshot",
            "ha": self.connected.load(Ordering::SeqCst),
            "states": &*states,
            "items": &*items,
            "aqi": &*self.aqi.read().unwrap(),
        })
        .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slim_keeps_only_needed_attributes() {
        let full = json!({
            "entity_id": "light.a", "state": "on",
            "attributes": {"friendly_name": "A", "brightness": 200, "supported_color_modes": ["xy"]},
            "context": {"id": "x"}
        });
        let s = slim(&full);
        assert_eq!(s["state"], "on");
        assert_eq!(s["a"]["brightness"], 200);
        assert!(s["a"].get("supported_color_modes").is_none());
    }

    #[test]
    fn ignores_unwanted_and_dedupes() {
        let hub = Hub::new(["light.a".to_string()].into());
        let mut rx = hub.subscribe();
        hub.set_state("light.b", &json!({"state": "on"}));
        hub.set_state("light.a", &json!({"state": "on"}));
        hub.set_state("light.a", &json!({"state": "on"})); // duplicate
        assert!(matches!(rx.try_recv(), Ok(Update::State { .. })));
        assert!(rx.try_recv().is_err());
        assert!(hub.snapshot_json().contains("light.a"));
        assert!(!hub.snapshot_json().contains("light.b"));
    }
}
