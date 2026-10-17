// Popups (calendar, power, keyboard, bluetooth, menus) and tooltips of the toolbar.
// Same popup window as the dock; they open away from the toolbar's screen edge.

import { api, Events, onSelf } from "@shared/ipc.ts";
import type { MenuEntry } from "@shared/menu.ts";
import { settingsState } from "@shared/state/settings.svelte.ts";
import { anchorOf } from "@shared/utils.ts";
import { toolbar } from "./state.svelte.ts";

type ActionHandler = (action: string, value: unknown) => void;
let handler: ActionHandler | null = null;

export async function initOverlays(): Promise<void> {
  await onSelf<{ kind: string; action: string; value: unknown }>(Events.PopupAction, (p) => {
    handler?.(p.action, p.value);
  });
}

/** the toolbar may have its own theme: its popups follow it */
const theme = () => settingsState.value.toolbar.theme;

export function openPopup(
  el: Element,
  kind: string,
  data: unknown,
  onAction?: ActionHandler,
  opts: { activate?: boolean } = {},
): void {
  handler = onAction ?? null;
  hideTooltip();
  api.popupOpen({
    kind,
    data,
    anchor: anchorOf(el),
    placement: toolbar.placement,
    key: kind,
    activate: opts.activate ?? true,
    theme: theme(),
  });
}

export function openMenu(e: MouseEvent, items: MenuEntry[], onAction: ActionHandler): void {
  e.preventDefault();
  e.stopPropagation();
  handler = onAction;
  hideTooltip();
  api.popupOpen({
    kind: "menu",
    data: { items },
    anchor: { x: e.clientX, y: e.clientY, width: 1, height: 1 },
    placement: toolbar.placement,
    align: "start",
    theme: theme(),
  });
}

// ---------------- tooltips ----------------

const SHOW_DELAY = 450;
let timer: ReturnType<typeof setTimeout> | null = null;
let visible = false;

export function hideTooltip() {
  if (timer) {
    clearTimeout(timer);
    timer = null;
  }
  if (visible) {
    visible = false;
    api.tooltipHide();
  }
}

/** Svelte action: `use:tooltip={() => text}` */
export function tooltip(node: HTMLElement, getText: () => string | null | undefined) {
  let current = getText;
  const enter = () => {
    hideTooltip();
    timer = setTimeout(() => {
      const text = current();
      if (!text) return;
      visible = true;
      api.tooltipShow(text, anchorOf(node), toolbar.placement);
    }, SHOW_DELAY);
  };
  node.addEventListener("mouseenter", enter);
  node.addEventListener("mouseleave", hideTooltip);
  node.addEventListener("pointerdown", hideTooltip);
  return {
    update(next: () => string | null | undefined) {
      current = next;
    },
    destroy() {
      node.removeEventListener("mouseenter", enter);
      node.removeEventListener("mouseleave", hideTooltip);
      node.removeEventListener("pointerdown", hideTooltip);
      hideTooltip();
    },
  };
}
