// End-to-end test: fake Home Assistant ⇄ glasspane server ⇄ this script
// (acting as the tablet). Needs Node 22+ (built-in fetch and WebSocket).
//
//   cargo run --bin mock_ha &
//   HA_TOKEN=dev cargo run -- demo.toml &
//   node tests/e2e.mjs [http://127.0.0.1:8090]
//
// Covers the HTTP API, the live WebSocket protocol, every command type, and
// the server-side allowlist that stops a client controlling anything that
// isn't on the dashboard.

const base = process.argv[2] || "http://127.0.0.1:8090";
const LIST = "todo.shopping_list";
let failures = 0;
const ok = (m) => console.log("  ✓ " + m);
const fail = (m) => { console.error("  ✗ " + m); failures++; };
const check = (cond, m) => (cond ? ok(m) : fail(m));
const section = (name) => console.log("\n" + name);

// ---- WebSocket helper ------------------------------------------------------
const ws = new WebSocket(base.replace(/^http/, "ws") + "/ws");
const inbox = [];
ws.onmessage = (e) => inbox.push(JSON.parse(e.data));
await new Promise((resolve, reject) => { ws.onopen = resolve; ws.onerror = reject; });
const send = (msg) => ws.send(JSON.stringify(msg));
/** Wait for (and remove) the first message matching `pred`. */
function next(pred, ms = 4000) {
  return new Promise((resolve) => {
    const start = Date.now();
    (function poll() {
      const i = inbox.findIndex(pred);
      if (i >= 0) return resolve(inbox.splice(i, 1)[0]);
      if (Date.now() - start > ms) return resolve(null);
      setTimeout(poll, 20);
    })();
  });
}
const stateOf = (id) => next((m) => m.t === "state" && m.id === id);
const listUpdate = () => next((m) => m.t === "items" && m.id === LIST);

// ---- HTTP API ----------------------------------------------------------------
section("HTTP API");
const layout = await (await fetch(base + "/api/layout")).json();
check(layout.pages.length > 0 && layout.shopping_list === LIST, `layout: ${layout.pages.length} rooms + shopping list`);
for (const path of ["/", "/app.js", "/app.css", "/manifest.json", "/bg.jpg", "/bg-blur.jpg"]) {
  const r = await fetch(base + path);
  if (r.status !== 200) fail(`${path} -> ${r.status}`);
}
ok("static files and glass backgrounds served");
const info = await (await fetch(base + "/api/info")).json();
check(typeof info.version === "string" && "ha_connected" in info, "info endpoint");
const saved = await (await fetch(base + "/api/settings", {
  method: "PUT", headers: { "Content-Type": "application/json" },
  body: JSON.stringify({ screensaver: { idleMin: 2 } }),
})).json();
check(saved.screensaver && saved.screensaver.idleMin === 2, "settings PUT merges and persists");
const bad = await fetch(base + "/api/settings", { method: "PUT", headers: { "Content-Type": "application/json" }, body: "[1,2]" });
check(bad.status === 400, "settings rejects non-objects");

// ---- Snapshot ------------------------------------------------------------------
section("Live snapshot");
const snap = await next((m) => m.t === "snapshot" && m.ha);
if (!snap) { fail("no snapshot with HA connected"); process.exit(1); }
check(!!snap.states["fan.living_room"] && snap.states["fan.living_room"].a.percentage_step > 0, "fan state with speed steps");
check(!!snap.states["light.living_room_fan_light"], "fan's built-in light tracked");
check(!("supported_color_modes" in (snap.states["light.floor_lamp"].a)), "attributes slimmed for the tablet");
check(Array.isArray(snap.items[LIST]) && !("description" in snap.items[LIST][0]), "shopping list items slimmed");

// ---- Devices ---------------------------------------------------------------------
section("Devices");
const lamp = snap.states["light.floor_lamp"].state;
send({ t: "toggle", id: "light.floor_lamp" });
const lampNow = await stateOf("light.floor_lamp");
check(lampNow && lampNow.s.state !== lamp, `toggle light: ${lamp} -> ${lampNow && lampNow.s.state}`);

send({ t: "speed", id: "fan.living_room", pct: 83 });
let fan = await stateOf("fan.living_room");
check(fan && fan.s.state === "on" && fan.s.a.percentage === 83, "fan speed 83%");
send({ t: "speed", id: "fan.living_room", pct: 0 });
fan = await stateOf("fan.living_room");
check(fan && fan.s.state === "off", "fan speed 0 turns it off");

send({ t: "toggle", id: "light.living_room_fan_light" });
check(!!(await stateOf("light.living_room_fan_light")), "fan light button toggles the companion light");

// ---- Shopping list ------------------------------------------------------------------
section("Shopping list");
send({ t: "todo_add", id: LIST, text: "  Bread  " });
let items = await listUpdate();
const bread = items && items.items.find((i) => i.summary === "Bread");
check(!!bread, "add item (trimmed) appears live");
send({ t: "todo_set", id: LIST, uid: bread.uid, done: true });
items = await listUpdate();
check(items && items.items.find((i) => i.uid === bread.uid).status === "completed", "tick item");
send({ t: "todo_clear", id: LIST });
items = await listUpdate();
check(items && !items.items.some((i) => i.status === "completed"), "clear removes ticked items");

// ---- Allowlist -------------------------------------------------------------------------
section("Allowlist (must all be rejected)");
inbox.length = 0;
send({ t: "toggle", id: "switch.not_on_dashboard" });
send({ t: "toggle", id: "sensor.living_room_temperature" });
send({ t: "speed", id: "light.floor_lamp", pct: 50 });
send({ t: "speed", id: "fan.living_room", pct: 250 });
send({ t: "todo_add", id: "todo.other_list", text: "x" });
send({ t: "todo_add", id: LIST, text: "   " });
send({ t: "todo_add", id: LIST, text: "x".repeat(500) });
await new Promise((r) => setTimeout(r, 500));
const leaked = inbox.filter((m) => (m.t === "state" && m.id !== "sensor.living_room_temperature") || m.t === "items");
check(leaked.length === 0, "unknown entities, non-fans, bad values and other lists are ignored");

ws.close();
console.log(failures ? `\n${failures} check(s) failed` : "\nAll checks passed");
process.exit(failures ? 1 : 0);
