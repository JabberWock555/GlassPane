// Capture the README screenshots from the demo home.
//
//   cargo run --bin mock_ha &                      # fake Home Assistant
//   HA_TOKEN=dev cargo run -- demo.toml &          # glasspane on :8090
//   npm i --no-save playwright && npx playwright install chromium
//   node scripts/screenshots.mjs docs/screenshots
//
// Uses a 1024×600 viewport (a typical 7" tablet) at 2× pixel density so the
// images stay sharp on high-DPI screens. Devices are switched on through the
// same WebSocket the dashboard uses, so the shots show a lived-in home.
import { chromium } from "playwright";

const out = process.argv[2] || "docs/screenshots";
const base = process.env.GLASSPANE_URL || "http://127.0.0.1:8090";
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// ---- Put the demo home into a nice state ----------------------------------
const ws = new WebSocket(base.replace(/^http/, "ws") + "/ws");
await new Promise((r) => (ws.onopen = r));
const snap = await new Promise((r) => (ws.onmessage = (e) => r(JSON.parse(e.data))));
const isOn = (id) => {
  const s = (snap.states || {})[id];
  return s && s.state === "on";
};
const turnOn = (id) => isOn(id) || ws.send(JSON.stringify({ t: "toggle", id }));
ws.send(JSON.stringify({ t: "speed", id: "fan.living_room", pct: 67 }));
ws.send(JSON.stringify({ t: "speed", id: "fan.bedroom", pct: 33 }));
turnOn("light.living_room_fan_light");
turnOn("light.floor_lamp");
await sleep(500);
ws.close();

// ---- Shots -----------------------------------------------------------------
const browser = await chromium.launch(
  process.env.CHROMIUM ? { executablePath: process.env.CHROMIUM } : {},
);
const page = await browser.newPage({ viewport: { width: 1024, height: 600 }, deviceScaleFactor: 2 });
const errors = [];
page.on("pageerror", (e) => errors.push(e.message));
const shot = async (name, wait = 700) => {
  await sleep(wait);
  await page.screenshot({ path: `${out}/${name}.png` });
  console.log("saved", name);
};

// Splash: delay the layout request so the loading screen stays up.
await page.route("**/api/layout", async (r) => {
  await sleep(1500);
  r.continue();
});
await page.goto(base);
await shot("splash", 400);
await page.waitForSelector(".grid .tile");
await page.mouse.click(600, 20); // first tap = full screen (swallowed)
await shot("home", 1500);

await page.click(".collapse-btn");
await shot("collapsed");
await page.click(".collapse-btn");

await page.click("text=Shopping");
await shot("shopping");

await page.click("text=Living Room");
await shot("room");

await page.click(".gear");
await shot("settings");

await page.click(".settings .pill-btn"); // "Preview" the screensaver
await shot("screensaver", 4000);

await browser.close();
if (errors.length) {
  console.error("page errors:", errors);
  process.exit(1);
}
