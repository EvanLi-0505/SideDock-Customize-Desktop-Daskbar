// Opening popups/tooltips next to dock items, and routing popup actions back.

import { api, Events, on } from "@shared/ipc.ts";
import { anchorOf } from "@shared/utils.ts";
import type { MenuEntry } from "@shared/menu.ts";
import { layout } from "./state/layout.svelte.ts";

type ActionHandler = (action: string, value: unknown) => void;

let handler: ActionHandler | null = null;

export async function initOverlays(): Promise<void> {
  await on<{ kind: string; action: string; value: unknown }>(Events.PopupAction, (p) => {
    handler?.(p.action, p.value);
  });
}

export function openPopup(
  el: Element,
  kind: string,
  data: unknown,
  opts: { key?: string; onAction?: ActionHandler } = {},
): void {
  handler = opts.onAction ?? null;
  api.popupOpen({
    kind,
    data,
    anchor: anchorOf(el),
    placement: layout.placement,
    key: opts.key,
  });
}

/** Context menu anchored at the cursor. */
export function openMenu(e: MouseEvent, items: MenuEntry[], onAction: ActionHandler): void {
  e.preventDefault();
  e.stopPropagation();
  handler = onAction;
  api.popupOpen({
    kind: "menu",
    data: { items },
    anchor: { x: e.clientX, y: e.clientY, width: 1, height: 1 },
    placement: layout.placement,
    align: "start",
  });
}

// ---------------- tooltips ----------------

const SHOW_DELAY = 450;
let tooltipTimer: ReturnType<typeof setTimeout> | null = null;
let tooltipVisible = false;

function hideTooltip() {
  if (tooltipTimer) {
    clearTimeout(tooltipTimer);
    tooltipTimer = null;
  }
  if (tooltipVisible) {
    tooltipVisible = false;
    api.tooltipHide();
  }
}

/** Svelte action: `use:tooltip={() => text}` */
export function tooltip(node: HTMLElement, getText: () => string | null | undefined) {
  let current = getText;
  const enter = () => {
    hideTooltip();
    tooltipTimer = setTimeout(() => {
      const text = current();
      if (!text) return;
      tooltipVisible = true;
      api.tooltipShow(text, anchorOf(node), layout.placement);
    }, SHOW_DELAY);
  };
  node.addEventListener("mouseenter", enter);
  node.addEventListener("mouseleave", hideTooltip);
  node.addEventListener("pointerdown", hideTooltip);
  node.addEventListener("contextmenu", hideTooltip);
  return {
    update(next: () => string | null | undefined) {
      current = next;
    },
    destroy() {
      node.removeEventListener("mouseenter", enter);
      node.removeEventListener("mouseleave", hideTooltip);
      node.removeEventListener("pointerdown", hideTooltip);
      node.removeEventListener("contextmenu", hideTooltip);
      hideTooltip();
    },
  };
}
