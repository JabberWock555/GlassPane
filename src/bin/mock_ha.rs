//! A small fake of the Home Assistant WebSocket API, for development, tests
//! and screenshots — no real Home Assistant (or real devices) needed.
//!
//!   cargo run --bin mock_ha          # listens on ws://127.0.0.1:8124/api/websocket
//!   MOCK_HA_ADDR=0.0.0.0:9000 cargo run --bin mock_ha
//!
//! It implements just the parts glasspane uses:
//! - `auth` (the token is "dev")
//! - `get_states`, `get_config`, `ping`
//! - `subscribe_events` (`state_changed`) and `todo/item/subscribe`
//! - `call_service` for `toggle`, `turn_on`, `turn_off`, `set_percentage`
//!   and the to-do services `add_item`, `update_item`, `remove_completed_items`
//!
//! The entities form a small demo home (see `demo.toml`): ceiling fans with
//! built-in lights and sleep modes, lamps, a coffee maker, two sensors and a
//! shopping list. The temperature drifts every few seconds so live updates
//! can be seen.

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio_tungstenite::{accept_async, tungstenite::Message};

/// All entity states, keyed by entity_id, in HA's own JSON shape.
type States = Arc<Mutex<BTreeMap<String, Value>>>;
/// Shopping list items: (uid, summary, completed).
type Todo = Arc<Mutex<Vec<(String, String, bool)>>>;

const TODO_ENTITY: &str = "todo.shopping_list";

/// Messages fanned out to every open connection.
#[derive(Clone)]
enum Event {
    /// A new entity state (sent to `state_changed` subscribers).
    State(Value),
    /// The shopping list changed (sent to `todo/item/subscribe` subscribers).
    Todo(Value),
}

fn entity(id: &str, state: &str, attrs: Value) -> (String, Value) {
    (
        id.to_string(),
        json!({"entity_id": id, "state": state, "attributes": attrs}),
    )
}

/// A 6-speed ceiling fan, like most smart BLDC fans expose to HA.
fn fan(name: &str, pct: u32) -> Value {
    json!({"friendly_name": name, "percentage": pct, "percentage_step": 100.0 / 6.0, "supported_features": 49})
}

fn demo_home() -> BTreeMap<String, Value> {
    BTreeMap::from([
        entity("fan.living_room", "on", fan("Living Room Fan", 50)),
        entity("fan.bedroom", "off", fan("Bedroom Fan", 0)),
        entity("fan.study", "on", fan("Study Fan", 83)),
        entity(
            "light.living_room_fan_light",
            "on",
            json!({"friendly_name": "Living Room Fan Light"}),
        ),
        entity(
            "light.bedroom_fan_light",
            "off",
            json!({"friendly_name": "Bedroom Fan Light"}),
        ),
        entity(
            "light.study_fan_light",
            "off",
            json!({"friendly_name": "Study Fan Light"}),
        ),
        entity(
            "switch.living_room_fan_sleep",
            "off",
            json!({"friendly_name": "Living Room Fan Sleep Mode"}),
        ),
        entity(
            "switch.bedroom_fan_sleep",
            "on",
            json!({"friendly_name": "Bedroom Fan Sleep Mode"}),
        ),
        entity(
            "light.floor_lamp",
            "on",
            json!({"friendly_name": "Floor Lamp", "brightness": 180}),
        ),
        entity(
            "light.bedside_lamp",
            "off",
            json!({"friendly_name": "Bedside Lamp", "brightness": null}),
        ),
        entity(
            "light.kitchen",
            "off",
            json!({"friendly_name": "Kitchen Light", "brightness": null}),
        ),
        entity(
            "switch.coffee_maker",
            "off",
            json!({"friendly_name": "Coffee Maker"}),
        ),
        entity(
            "sensor.living_room_temperature",
            "24.5",
            json!({"friendly_name": "Temperature", "unit_of_measurement": "°C", "device_class": "temperature"}),
        ),
        entity(
            "sensor.kitchen_humidity",
            "58",
            json!({"friendly_name": "Humidity", "unit_of_measurement": "%", "device_class": "humidity"}),
        ),
    ])
}

fn todo_json(todo: &Todo) -> Value {
    Value::Array(
        todo.lock()
            .unwrap()
            .iter()
            .map(|(uid, summary, done)| {
                json!({"uid": uid, "summary": summary,
                "status": if *done { "completed" } else { "needs_action" }, "description": null})
            })
            .collect(),
    )
}

/// HA's to-do entity state is the number of open items.
fn todo_state(todo: &Todo) -> Value {
    let open = todo.lock().unwrap().iter().filter(|i| !i.2).count();
    json!({"entity_id": TODO_ENTITY, "state": open.to_string(),
           "attributes": {"friendly_name": "Shopping List", "supported_features": 15}})
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr = std::env::var("MOCK_HA_ADDR").unwrap_or_else(|_| "127.0.0.1:8124".into());
    let todo: Todo = Arc::new(Mutex::new(vec![
        ("u1".into(), "Milk".into(), false),
        ("u2".into(), "Eggs".into(), false),
        ("u3".into(), "Basmati rice".into(), false),
        ("u4".into(), "Coffee beans".into(), false),
        ("u5".into(), "Tomatoes".into(), true),
    ]));
    let states: States = Arc::new(Mutex::new(demo_home()));
    states
        .lock()
        .unwrap()
        .insert(TODO_ENTITY.into(), todo_state(&todo));
    let (events, _) = broadcast::channel::<Event>(64);

    // Drift the temperature so clients see live updates.
    {
        let (states, events) = (states.clone(), events.clone());
        tokio::spawn(async move {
            for tick in 0u32.. {
                tokio::time::sleep(Duration::from_secs(5)).await;
                let t = 24.5 + ((tick % 7) as f64 - 3.0) * 0.1;
                let new = {
                    let mut map = states.lock().unwrap();
                    let s = map.get_mut("sensor.living_room_temperature").unwrap();
                    s["state"] = json!(format!("{t:.1}"));
                    s.clone()
                };
                let _ = events.send(Event::State(new));
            }
        });
    }

    let listener = TcpListener::bind(&addr).await?;
    println!("mock Home Assistant on ws://{addr}/api/websocket (token: dev)");
    loop {
        let (stream, _) = listener.accept().await?;
        let (states, events, todo) = (states.clone(), events.clone(), todo.clone());
        tokio::spawn(async move {
            if let Err(e) = connection(stream, states, events, todo).await {
                eprintln!("mock connection ended: {e}");
            }
        });
    }
}

/// One client connection, following HA's protocol: auth handshake, then
/// request/response messages correlated by `id`, plus pushed events.
async fn connection(
    stream: TcpStream,
    states: States,
    events: broadcast::Sender<Event>,
    todo: Todo,
) -> anyhow::Result<()> {
    let ws = accept_async(stream).await?;
    let (mut tx, mut rx) = ws.split();
    let msg = |v: Value| Message::Text(v.to_string().into());
    let ok = |id: u64, result: Value| {
        msg(json!({"id": id, "type": "result", "success": true, "result": result}))
    };

    tx.send(msg(json!({"type": "auth_required", "ha_version": "mock"})))
        .await?;
    let mut authed = false;
    let mut state_sub: Option<u64> = None;
    let mut todo_sub: Option<u64> = None;
    let mut next_uid = 100u32;
    let mut event_rx = events.subscribe();

    loop {
        tokio::select! {
            incoming = rx.next() => {
                let Some(Ok(Message::Text(text))) = incoming else { return Ok(()) };
                let m: Value = serde_json::from_str(&text)?;
                let id = m["id"].as_u64().unwrap_or(0);
                match (authed, m["type"].as_str().unwrap_or("")) {
                    (false, "auth") if m["access_token"] == "dev" => {
                        authed = true;
                        tx.send(msg(json!({"type": "auth_ok", "ha_version": "mock"}))).await?;
                    }
                    (false, "auth") => {
                        tx.send(msg(json!({"type": "auth_invalid", "message": "bad token"}))).await?;
                        return Ok(());
                    }
                    (true, "get_states") => {
                        let list: Vec<Value> = states.lock().unwrap().values().cloned().collect();
                        tx.send(ok(id, Value::Array(list))).await?;
                    }
                    (true, "get_config") => {
                        tx.send(ok(id, json!({"latitude": 52.37, "longitude": 4.90, "location_name": "Demo Home"}))).await?;
                    }
                    (true, "subscribe_events") => {
                        state_sub = Some(id);
                        tx.send(ok(id, Value::Null)).await?;
                    }
                    (true, "todo/item/subscribe") => {
                        // Like HA: a result, then the full list as an event.
                        todo_sub = Some(id);
                        tx.send(ok(id, Value::Null)).await?;
                        tx.send(msg(json!({"id": id, "type": "event", "event": {"items": todo_json(&todo)}}))).await?;
                    }
                    (true, "call_service") if m["domain"] == "todo" => {
                        apply_todo(&todo, m["service"].as_str().unwrap_or(""), &m["service_data"], &mut next_uid);
                        let _ = events.send(Event::Todo(todo_json(&todo)));
                        let _ = events.send(Event::State(todo_state(&todo)));
                        tx.send(ok(id, Value::Null)).await?;
                    }
                    (true, "call_service") => {
                        let target = m["target"]["entity_id"].as_str().unwrap_or("");
                        let changed = apply_service(&states, target, m["service"].as_str().unwrap_or(""), &m["service_data"]);
                        if let Some(new) = &changed {
                            let _ = events.send(Event::State(new.clone()));
                        }
                        tx.send(if changed.is_some() { ok(id, Value::Null) } else {
                            msg(json!({"id": id, "type": "result", "success": false, "error": {"message": "unknown entity"}}))
                        }).await?;
                    }
                    (true, "ping") => tx.send(msg(json!({"id": id, "type": "pong"}))).await?,
                    _ => {}
                }
            }
            event = event_rx.recv() => {
                let out = match (event, state_sub, todo_sub) {
                    (Ok(Event::State(s)), Some(sid), _) => json!({"id": sid, "type": "event",
                        "event": {"event_type": "state_changed", "data": {"entity_id": s["entity_id"], "new_state": s}}}),
                    (Ok(Event::Todo(items)), _, Some(tid)) => json!({"id": tid, "type": "event", "event": {"items": items}}),
                    _ => continue,
                };
                tx.send(msg(out)).await?;
            }
        }
    }
}

/// Apply a device service call; returns the entity's new state.
fn apply_service(states: &States, target: &str, service: &str, data: &Value) -> Option<Value> {
    let mut map = states.lock().unwrap();
    let s = map.get_mut(target)?;
    let on = match (service, data["percentage"].as_u64()) {
        ("set_percentage", Some(p)) => {
            s["attributes"]["percentage"] = json!(p);
            p > 0
        }
        ("turn_off", _) => false,
        ("turn_on", _) => true,
        _ => s["state"] != "on", // toggle
    };
    s["state"] = json!(if on { "on" } else { "off" });
    if target.starts_with("fan.") {
        // Like a real fan: off means 0 %, turning on resumes a low speed.
        let pct = s["attributes"]["percentage"].as_u64().unwrap_or(0);
        if !on {
            s["attributes"]["percentage"] = json!(0);
        } else if pct == 0 {
            s["attributes"]["percentage"] = json!(33);
        }
    }
    Some(s.clone())
}

fn apply_todo(todo: &Todo, service: &str, data: &Value, next_uid: &mut u32) {
    let mut list = todo.lock().unwrap();
    match service {
        "add_item" => {
            *next_uid += 1;
            list.push((
                format!("n{next_uid}"),
                data["item"].as_str().unwrap_or("").into(),
                false,
            ));
        }
        "update_item" => {
            // HA accepts either the uid or the summary as `item`.
            let key = data["item"].as_str().unwrap_or("");
            let done = data["status"] == "completed";
            for item in list.iter_mut().filter(|i| i.0 == key || i.1 == key) {
                item.2 = done;
            }
        }
        "remove_completed_items" => list.retain(|i| !i.2),
        _ => {}
    }
}
