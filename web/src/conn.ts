// WebSocket link to the glasspane server, with automatic reconnect.

export interface EState {
  state: string;
  a: {
    friendly_name?: string;
    brightness?: number | null;
    unit_of_measurement?: string;
    device_class?: string;
    icon?: string;
    percentage?: number | null;
    percentage_step?: number;
  };
}

export interface TodoItem {
  uid: string;
  summary: string;
  status: "needs_action" | "completed";
}

export type ServerMsg =
  | { t: "snapshot"; ha: boolean; states: { [id: string]: EState }; items?: { [id: string]: TodoItem[] }; aqi?: any }
  | { t: "aqi"; aqi: any }
  | { t: "state"; id: string; s: EState }
  | { t: "removed"; id: string }
  | { t: "ha"; connected: boolean }
  | { t: "items"; id: string; items: TodoItem[] };

export class Conn {
  private ws: WebSocket | null = null;
  private retryMs = 1000;

  constructor(
    private onMsg: (m: ServerMsg) => void,
    private onLink: (up: boolean) => void,
  ) {
    this.open();
    // Tablets sleep; when the screen wakes, reconnect immediately.
    document.addEventListener("visibilitychange", () => {
      if (!document.hidden && !this.ws) this.open();
    });
  }

  private open() {
    const proto = location.protocol === "https:" ? "wss:" : "ws:";
    const ws = new WebSocket(proto + "//" + location.host + "/ws");
    this.ws = ws;
    ws.onopen = () => {
      this.retryMs = 1000;
      this.onLink(true);
    };
    ws.onmessage = (e) => {
      try {
        this.onMsg(JSON.parse(e.data));
      } catch (err) {
        console.warn("bad message", err);
      }
    };
    ws.onclose = () => {
      if (this.ws !== ws) return;
      this.ws = null;
      this.onLink(false);
      setTimeout(() => {
        if (!this.ws) this.open();
      }, this.retryMs);
      this.retryMs = Math.min(this.retryMs * 2, 15000);
    };
  }

  private send(msg: object): boolean {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) return false;
    this.ws.send(JSON.stringify(msg));
    return true;
  }

  toggle(id: string): boolean {
    return this.send({ t: "toggle", id: id });
  }

  todoAdd(id: string, text: string): boolean {
    return this.send({ t: "todo_add", id: id, text: text });
  }

  todoSet(id: string, uid: string, done: boolean): boolean {
    return this.send({ t: "todo_set", id: id, uid: uid, done: done });
  }

  todoClear(id: string): boolean {
    return this.send({ t: "todo_clear", id: id });
  }

  /** Fan speed as a percentage (0 = off). */
  speed(id: string, pct: number): boolean {
    return this.send({ t: "speed", id: id, pct: Math.max(0, Math.min(100, Math.round(pct))) });
  }
}
