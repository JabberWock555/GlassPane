// Shopping list (any Home Assistant to-do list), live-synced through the server.

import { useEffect, useState } from "preact/hooks";
import type { EState, TodoItem } from "./conn";
import { spanStyle, TileCfg } from "./types";
import { Cart, Check, Plus } from "./icons";

interface Props {
  cfg: TileCfg;
  st?: EState;
  items?: TodoItem[];
  onAdd: (id: string, text: string) => boolean;
  onSet: (id: string, uid: string, done: boolean) => boolean;
  onClear: (id: string) => boolean;
}

/**
 * Shopping list (any HA to-do list). Tap an item to tick it off; ticked items
 * drop to the bottom. Add items from the box at the bottom; "Clear" removes
 * ticked items. Changes made on phones or in HA appear live.
 */
export function TodoTile({ cfg, st, items, onAdd, onSet, onClear }: Props) {
  const [text, setText] = useState("");
  // Optimistic ticks until HA's updated list arrives.
  const [ticks, setTicks] = useState<{ [uid: string]: boolean }>({});
  useEffect(() => setTicks({}), [items]);

  const name = cfg.name || (st && st.a.friendly_name) || "Shopping list";
  const list = (items || []).map((i) => ({
    uid: i.uid,
    summary: i.summary,
    done: i.uid in ticks ? ticks[i.uid] : i.status === "completed",
  }));
  const open = list.filter((i) => !i.done);
  const done = list.filter((i) => i.done);
  const rows = cfg.rows || 2;

  const tick = (uid: string, isDone: boolean) => {
    if (onSet(cfg.entity, uid, !isDone)) {
      setTicks((prev) => Object.assign({}, prev, { [uid]: !isDone }));
    }
  };

  const submit = (e: Event) => {
    e.preventDefault();
    const t = text.replace(/^\s+|\s+$/g, "");
    if (!t) return;
    if (onAdd(cfg.entity, t.slice(0, 120))) setText("");
  };

  return (
    <div class={"tile glass todo-tile wide" + (rows > 1 ? " tall" : "")} style={spanStyle(cfg)}>
      <div class="todo-head">
        <span class="todo-icon"><Cart size={26} /></span>
        <span class="todo-title">{name}</span>
        <span class="todo-count">{items ? (open.length ? open.length + " to buy" : "All done") : "Loading…"}</span>
      </div>

      <div class="todo-list">
        {list.length === 0 && items && <div class="todo-empty">Nothing on the list</div>}
        {open.concat(done).map((i) => (
          <button key={i.uid} class={"todo-item" + (i.done ? " done" : "")} onClick={() => tick(i.uid, i.done)}>
            <span class="box">{i.done && <Check size={16} />}</span>
            <span class="label">{i.summary}</span>
          </button>
        ))}
      </div>

      <form class="todo-add" onSubmit={submit}>
        <input
          type="text"
          value={text}
          maxLength={120}
          placeholder="Add item…"
          onInput={(e) => setText((e.target as HTMLInputElement).value)}
        />
        <button type="submit" class="round-btn add" aria-label="Add item" disabled={!text}>
          <Plus size={20} />
        </button>
        {done.length > 0 && (
          <button type="button" class="clear" onClick={() => onClear(cfg.entity)}>
            Clear {done.length}
          </button>
        )}
      </form>
    </div>
  );
}
