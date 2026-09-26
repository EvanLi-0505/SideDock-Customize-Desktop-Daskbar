// Pointer based drag-to-reorder along the dock axis (replaces dnd-kit used by Seelen UI).
// Listeners live on `window` so the drag survives the dragged node being moved in the DOM.

import { api } from "@shared/ipc.ts";
import { dockState } from "./state/items.svelte.ts";
import { layout } from "./state/layout.svelte.ts";

const DRAG_THRESHOLD = 5;

let draggingId = $state<string | null>(null);

export const sortable = {
  get draggingId() {
    return draggingId;
  },
};

/** Set when a drag just ended, so the click that follows it is ignored. */
let suppressClickUntil = 0;

export function shouldSuppressClick(): boolean {
  return performance.now() < suppressClickUntil;
}

function reorder(id: string, pointer: number) {
  const horizontal = layout.horizontal;
  const containers = Array.from(document.querySelectorAll<HTMLElement>(".weg-item-drag-container"));
  let targetId: string | null = null;
  let after = false;
  for (const el of containers) {
    const r = el.getBoundingClientRect();
    const start = horizontal ? r.left : r.top;
    const end = horizontal ? r.right : r.bottom;
    if (pointer >= start && pointer <= end) {
      targetId = el.dataset.itemId ?? null;
      after = pointer > (start + end) / 2;
      break;
    }
  }
  if (!targetId || targetId === id) return;

  const items = [...dockState.items];
  const from = items.findIndex((i) => i.id === id);
  let to = items.findIndex((i) => i.id === targetId);
  if (from === -1 || to === -1) return;
  // only move once the pointer crossed the middle of the neighbour
  if ((from < to && !after) || (from > to && after)) return;
  const [moved] = items.splice(from, 1);
  to = items.findIndex((i) => i.id === targetId);
  items.splice(after ? to + 1 : to, 0, moved!);
  dockState.items = items;
}

/** Svelte action for `.weg-item-drag-container` elements. */
export function draggable(node: HTMLElement, id: string) {
  let itemId = id;

  const onPointerDown = (e: PointerEvent) => {
    if (e.button !== 0 || dockState.isReorderDisabled) return;
    const horizontal = layout.horizontal;
    const startX = e.clientX;
    const startY = e.clientY;
    let started = false;

    const move = (ev: PointerEvent) => {
      const delta = horizontal ? ev.clientX - startX : ev.clientY - startY;
      if (!started) {
        if (Math.abs(delta) < DRAG_THRESHOLD) return;
        started = true;
        draggingId = itemId;
        api.dockSetDragging(true);
        document.body.classList.add("dragging");
      }
      reorder(itemId, horizontal ? ev.clientX : ev.clientY);
    };

    const end = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", end);
      window.removeEventListener("pointercancel", end);
      if (started) {
        suppressClickUntil = performance.now() + 150;
        draggingId = null;
        api.dockSetDragging(false);
        document.body.classList.remove("dragging");
      }
    };

    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", end);
    window.addEventListener("pointercancel", end);
  };

  node.addEventListener("pointerdown", onPointerDown);
  return {
    update(next: string) {
      itemId = next;
    },
    destroy() {
      node.removeEventListener("pointerdown", onPointerDown);
    },
  };
}
