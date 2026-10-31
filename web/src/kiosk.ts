// Kiosk mode: full screen + keep-awake, both started by the first tap
// (browsers only allow them from a user gesture). The first tap is swallowed
// so it doesn't also toggle a device — except on text boxes, which still need
// focus. Chrome 57 has no Wake Lock API, so NoSleep plays a tiny invisible
// looping video, which keeps the screen on while the dashboard is visible.
import NoSleep from "nosleep.js";

const noSleep = new NoSleep();
let wantAwake = true;

export function setKeepAwake(on: boolean) {
  wantAwake = on;
  if (!on && noSleep.isEnabled) noSleep.disable();
}

export function installKiosk() {
  document.addEventListener(
    "click",
    (ev) => {
      if (wantAwake && !noSleep.isEnabled) {
        try {
          const p: any = noSleep.enable();
          if (p && p.catch) p.catch(() => {});
        } catch (e) {
          /* keep-awake unavailable; screen timeout still applies */
        }
      }
      const d: any = document;
      if (d.fullscreenElement || d.webkitFullscreenElement) return;
      const el: any = document.documentElement;
      const req = el.requestFullscreen || el.webkitRequestFullscreen;
      if (!req) return;
      const target = ev.target as HTMLElement | null;
      if (!target || target.tagName !== "INPUT") {
        ev.stopPropagation();
        ev.preventDefault();
      }
      try {
        const p = req.call(el);
        if (p && p.catch) p.catch(() => {});
      } catch (e) {
        /* full screen refused */
      }
    },
    true,
  );
}
