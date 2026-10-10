// Dragging a notification area icon between the toolbar and its overflow flyout (two
// different windows), like on the Windows taskbar.
//
// The source keeps the pointer captured and reports each move to the backend, which
// tells every toolbar and the flyout where the cursor is in their own coordinates
// (`tray-drag` event). The window under the cursor when the button is released places
// the icon (`api.trayPlace`). A press that does not move is a normal click.

import { api } from "./ipc.ts";

const THRESHOLD = 5;

export interface TrayDragSource {
  key: string;
  /** a drag started (the press moved) */
  onStart?: () => void;
  /** the drag ended, dropped anywhere or cancelled */
  onEnd?: () => void;
}

/** Svelte action on a tray icon button. Clicks following a drag are swallowed. */
export function trayDraggable(node: HTMLElement, source: TrayDragSource) {
  let current = source;
  let start: { x: number; y: number; pointer: number } | null = null;
  let dragging = false;
  let frame = 0;
  let suppressClick = false;

  const down = (e: PointerEvent) => {
    if (e.button !== 0) return;
    start = { x: e.clientX, y: e.clientY, pointer: e.pointerId };
  };
  const move = (e: PointerEvent) => {
    if (!start || e.pointerId !== start.pointer) return;
    if (!dragging) {
      if (Math.hypot(e.clientX - start.x, e.clientY - start.y) < THRESHOLD) return;
      dragging = true;
      // keeps receiving the pointer outside of this window
      node.setPointerCapture(e.pointerId);
      current.onStart?.();
    }
    if (!frame) {
      frame = requestAnimationFrame(() => {
        frame = 0;
        if (dragging) api.trayDrag("move", current.key);
      });
    }
  };
  const finish = (phase: "end" | "cancel") => (e: PointerEvent) => {
    if (!start || e.pointerId !== start.pointer) return;
    start = null;
    if (!dragging) return;
    dragging = false;
    suppressClick = true;
    cancelAnimationFrame(frame);
    frame = 0;
    api.trayDrag(phase, current.key);
    current.onEnd?.();
  };
  const up = finish("end");
  const cancel = finish("cancel");
  const click = (e: MouseEvent) => {
    if (!suppressClick) return;
    suppressClick = false;
    e.stopImmediatePropagation();
    e.preventDefault();
  };

  node.addEventListener("pointerdown", down);
  node.addEventListener("pointermove", move);
  node.addEventListener("pointerup", up);
  node.addEventListener("pointercancel", cancel);
  // capture phase: runs before the button's own click handler
  node.addEventListener("click", click, true);
  return {
    update(next: TrayDragSource) {
      current = next;
    },
    destroy() {
      node.removeEventListener("pointerdown", down);
      node.removeEventListener("pointermove", move);
      node.removeEventListener("pointerup", up);
      node.removeEventListener("pointercancel", cancel);
      node.removeEventListener("click", click, true);
      cancelAnimationFrame(frame);
    },
  };
}

/**
 * Insertion index among `elements` (laid out in a row, or a grid read row by row) for
 * a point in the window's CSS px.
 */
export function insertionIndex(elements: Element[], x: number, y: number): number {
  for (let i = 0; i < elements.length; i++) {
    const r = elements[i]!.getBoundingClientRect();
    const sameRow = y >= r.top && y < r.bottom;
    if ((sameRow && x < r.left + r.width / 2) || y < r.top) return i;
  }
  return elements.length;
}
