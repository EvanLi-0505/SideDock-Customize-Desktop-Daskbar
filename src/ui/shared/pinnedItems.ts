// Pinned apps of a dock as the settings pages list them: the apps on the dock and the
// apps inside its folder stacks (marked with the stack's name).

import type { AppDockItem, DockItem, DockItems } from "./types.ts";

export interface PinnedEntry {
  app: AppDockItem;
  /** name of the folder stack holding it, null when it is on the dock itself */
  stack: string | null;
}

export function pinnedEntries(items: DockItems | null | undefined): PinnedEntry[] {
  if (!items) return [];
  const out: PinnedEntry[] = [];
  for (const item of [...items.left, ...items.center, ...items.right]) {
    if (item.type === "App" && item.pinned) out.push({ app: item, stack: null });
    else if (item.type === "Stack") {
      for (const app of item.items) out.push({ app, stack: item.name });
    }
  }
  return out;
}

/** The items without app `id`, on the dock or in a stack; a stack left empty goes too. */
export function withoutApp(items: DockItems, id: string): DockItems {
  const strip = (list: DockItem[]) =>
    list
      .filter((i) => i.id !== id)
      .map((i) => (i.type === "Stack" ? { ...i, items: i.items.filter((a) => a.id !== id) } : i))
      .filter((i) => i.type !== "Stack" || i.items.length > 0);
  return { ...items, left: strip(items.left), center: strip(items.center), right: strip(items.right) };
}
