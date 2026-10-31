// Frosted glass without backdrop-filter (needs Chrome 76; the tablet has 57).
//
// The page shows /bg.jpg stretched to the viewport. Every `.glass` element
// shows /bg-blur.jpg (a pre-blurred copy made by the server) at the same size,
// shifted by its own on-screen position, so each tile displays exactly the
// blurred version of whatever is behind it.

export function alignGlass(root: ParentNode) {
  const w = window.innerWidth;
  const h = window.innerHeight;
  const els = root.querySelectorAll(".glass");
  for (let i = 0; i < els.length; i++) {
    const el = els[i] as HTMLElement;
    const r = el.getBoundingClientRect();
    el.style.backgroundSize = w + "px " + h + "px";
    el.style.backgroundPosition = -Math.round(r.left) + "px " + -Math.round(r.top) + "px";
  }
}
