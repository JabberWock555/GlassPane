//! Home Assistant WebSocket API client.
//!
//! Keeps one long-lived connection: authenticates with the long-lived token,
//! loads current states, subscribes to `state_changed` and to the items of
//! every to-do list on the dashboard, and forwards commands as
//! `call_service`. Reconnects with exponential backoff.

use crate::hub::{Hub, Location};
use anyhow::{Context, bail};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, info, warn};

#[derive(Debug)]
pub enum Command {
    Toggle(String),
    /// Set a fan's speed as a percentage; 0 turns it off.
    SetPercentage(String, u8),
    /// Add an item to a to-do list.
    TodoAdd(String, String),
    /// Mark a to-do item (by uid) done or not done.
    TodoSet(String, String, bool),
    /// Remove all completed items from a to-do list.
    TodoClear(String),
}

fn call(id: u64, domain: &str, service: &str, entity: &str, data: Option<Value>) -> Value {
    let mut msg = json!({"id": id, "type": "call_service", "domain": domain,
                         "service": service, "target": {"entity_id": entity}});
    if let Some(d) = data {
        msg["service_data"] = d;
    }
    msg
}

/// Build the `call_service` message for a command.
pub fn service_call(id: u64, cmd: &Command) -> Value {
    match cmd {
        Command::Toggle(entity) => {
            let domain = entity.split('.').next().unwrap_or("homeassistant");
            call(id, domain, "toggle", entity, None)
        }
        Command::SetPercentage(entity, 0) => call(id, "fan", "turn_off", entity, None),
        Command::SetPercentage(entity, pct) => call(
            id,
            "fan",
            "set_percentage",
            entity,
            Some(json!({"percentage": pct})),
        ),
        Command::TodoAdd(entity, text) => {
            call(id, "todo", "add_item", entity, Some(json!({"item": text})))
        }
        Command::TodoSet(entity, uid, done) => call(
            id,
            "todo",
            "update_item",
            entity,
            Some(json!({"item": uid, "status": if *done { "completed" } else { "needs_action" }})),
        ),
        Command::TodoClear(entity) => call(id, "todo", "remove_completed_items", entity, None),
    }
}

// HA rejects a request whose id isn't higher than every id before it
// (`id_reuse`), so these must follow the order they're sent in.
const ID_GET_STATES: u64 = 1;
const ID_GET_CONFIG: u64 = 2;
const ID_SUBSCRIBE: u64 = 3;
/// To-do list subscriptions use ids TODO_BASE + index into `todos`.
const TODO_BASE: u64 = 100;
/// Service calls and pings start here, clear of the fixed ids above.
const DYNAMIC_BASE: u64 = 1000;

pub async fn run(
    url: String,
    token: String,
    hub: Arc<Hub>,
    todos: Vec<String>,
    mut cmds: mpsc::Receiver<Command>,
) {
    let mut backoff = 1u64;
    loop {
        match session(&url, &token, &hub, &todos, &mut cmds, &mut backoff).await {
            Ok(()) => warn!("Home Assistant connection closed"),
            Err(e) => warn!("Home Assistant connection error: {e:#}"),
        }
        hub.set_connected(false);
        // Drop commands queued while offline rather than replaying stale taps.
        while cmds.try_recv().is_ok() {}
        info!("reconnecting to Home Assistant in {backoff}s");
        tokio::time::sleep(Duration::from_secs(backoff)).await;
        backoff = (backoff * 2).min(30);
    }
}

async fn session(
    url: &str,
    token: &str,
    hub: &Hub,
    todos: &[String],
    cmds: &mut mpsc::Receiver<Command>,
    backoff: &mut u64,
) -> anyhow::Result<()> {
    let (ws, _) = connect_async(url).await.context("connecting")?;
    let (mut tx, mut rx) = ws.split();

    // --- auth handshake -------------------------------------------------
    let first = next_json(&mut rx).await?;
    if first["type"] != "auth_required" {
        bail!("unexpected first message: {first}");
    }
    send(&mut tx, json!({"type": "auth", "access_token": token})).await?;
    let auth = next_json(&mut rx).await?;
    match auth["type"].as_str() {
        Some("auth_ok") => {
            let v = auth["ha_version"].as_str().unwrap_or("?");
            info!("authenticated with Home Assistant {v}");
            *hub.ha_version.write().unwrap() = v.to_string();
        }
        Some("auth_invalid") => {
            *backoff = 60; // a bad token won't fix itself; don't hammer HA
            bail!("token rejected: {}", auth["message"]);
        }
        _ => bail!("unexpected auth reply: {auth}"),
    }
    *backoff = 1;

    // --- initial state + subscriptions ----------------------------------
    send(&mut tx, json!({"id": ID_GET_STATES, "type": "get_states"})).await?;
    // Home location (for the AQI widget).
    send(&mut tx, json!({"id": ID_GET_CONFIG, "type": "get_config"})).await?;
    send(
        &mut tx,
        json!({"id": ID_SUBSCRIBE, "type": "subscribe_events", "event_type": "state_changed"}),
    )
    .await?;
    for (i, entity) in todos.iter().enumerate() {
        // HA answers with a result, then pushes the full item list as an
        // event now and after every change.
        send(
            &mut tx,
            json!({"id": TODO_BASE + i as u64, "type": "todo/item/subscribe", "entity_id": entity}),
        )
        .await?;
    }

    let mut next_id = DYNAMIC_BASE;
    let mut ping = tokio::time::interval(Duration::from_secs(30));
    ping.tick().await;

    loop {
        tokio::select! {
            msg = rx.next() => {
                let Some(msg) = msg else { return Ok(()) };
                match msg.context("reading")? {
                    Message::Text(text) => handle(hub, todos, &text),
                    Message::Close(_) => return Ok(()),
                    _ => {}
                }
            }
            cmd = cmds.recv() => {
                let Some(cmd) = cmd else { return Ok(()) };
                next_id += 1;
                debug!("{cmd:?}");
                send(&mut tx, service_call(next_id, &cmd)).await?;
            }
            _ = ping.tick() => {
                next_id += 1;
                send(&mut tx, json!({"id": next_id, "type": "ping"})).await?;
            }
        }
    }
}

/// Apply one HA message to the hub.
pub fn handle(hub: &Hub, todos: &[String], text: &str) {
    let Ok(msg) = serde_json::from_str::<Value>(text) else {
        warn!("non-JSON message from HA");
        return;
    };
    let id = msg["id"].as_u64().unwrap_or(0);
    match msg["type"].as_str() {
        Some("result") => {
            if msg["success"] != true {
                warn!("HA request {} failed: {}", msg["id"], msg["error"]);
                return;
            }
            if id == ID_GET_STATES {
                if let Some(list) = msg["result"].as_array() {
                    for s in list {
                        if let Some(id) = s["entity_id"].as_str() {
                            hub.set_state(id, s);
                        }
                    }
                }
                hub.set_connected(true);
            } else if id == ID_GET_CONFIG {
                let r = &msg["result"];
                if let (Some(lat), Some(lon)) = (r["latitude"].as_f64(), r["longitude"].as_f64()) {
                    let name = r["location_name"].as_str().unwrap_or("Home").to_string();
                    *hub.location.write().unwrap() = Some(Location { lat, lon, name });
                }
            }
        }
        Some("event") if id >= TODO_BASE && id < TODO_BASE + todos.len() as u64 => {
            let entity = &todos[(id - TODO_BASE) as usize];
            hub.set_items(entity, &msg["event"]["items"]);
        }
        Some("event") => {
            let data = &msg["event"]["data"];
            let Some(id) = data["entity_id"].as_str() else {
                return;
            };
            if data["new_state"].is_null() {
                hub.remove(id);
            } else {
                hub.set_state(id, &data["new_state"]);
            }
        }
        _ => {}
    }
}

/// Read the next JSON text frame (used during the auth handshake).
async fn next_json<S>(rx: &mut S) -> anyhow::Result<Value>
where
    S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    loop {
        match rx
            .next()
            .await
            .context("connection closed during handshake")??
        {
            Message::Text(t) => return Ok(serde_json::from_str(&t)?),
            Message::Close(_) => bail!("closed during handshake"),
            _ => continue,
        }
    }
}

async fn send<S>(tx: &mut S, v: Value) -> anyhow::Result<()>
where
    S: SinkExt<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    tx.send(Message::Text(v.to_string().into())).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_zero_turns_off_and_other_speeds_set_percentage() {
        let off = service_call(5, &Command::SetPercentage("fan.a".into(), 0));
        assert_eq!(off["service"], "turn_off");
        let set = service_call(6, &Command::SetPercentage("fan.a".into(), 50));
        assert_eq!(set["service"], "set_percentage");
        assert_eq!(set["service_data"]["percentage"], 50);
        let t = service_call(7, &Command::Toggle("light.x".into()));
        assert_eq!(
            (t["domain"].as_str(), t["service"].as_str()),
            (Some("light"), Some("toggle"))
        );
    }

    #[test]
    fn todo_service_calls() {
        let add = service_call(1, &Command::TodoAdd("todo.s".into(), "Milk".into()));
        assert_eq!(
            (
                add["service"].as_str(),
                add["service_data"]["item"].as_str()
            ),
            (Some("add_item"), Some("Milk"))
        );
        let done = service_call(2, &Command::TodoSet("todo.s".into(), "u1".into(), true));
        assert_eq!(done["service_data"]["status"], "completed");
        let undo = service_call(3, &Command::TodoSet("todo.s".into(), "u1".into(), false));
        assert_eq!(undo["service_data"]["status"], "needs_action");
        let clear = service_call(4, &Command::TodoClear("todo.s".into()));
        assert_eq!(clear["service"], "remove_completed_items");
    }

    #[test]
    fn todo_events_route_to_their_list() {
        let hub = Hub::new(["todo.s".to_string()].into());
        let todos = vec!["todo.s".to_string()];
        handle(
            &hub,
            &todos,
            r#"{"id":100,"type":"event","event":{"items":[
            {"uid":"a","summary":"Milk","status":"needs_action","description":null}]}}"#,
        );
        let snap = hub.snapshot_json();
        assert!(snap.contains("Milk"));
        assert!(!snap.contains("description"));
    }
}
