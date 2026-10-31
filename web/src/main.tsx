// Entry point: install kiosk behaviour (full screen + keep-awake) and render.
import { render } from "preact";
import { App } from "./App";
import { installKiosk } from "./kiosk";

installKiosk();
render(<App />, document.getElementById("root")!);
