import { api, Events, on } from "@shared/ipc.ts";
import { pinnedEntries, withoutApp, type PinnedEntry } from "@shared/pinnedItems.ts";
import type { DockItem, DockItems, ModuleId } from "@shared/types.ts";

const SOURCE = "settings";

let items = $state<DockItems | null>(null);

export const dockItemsState = {
  get value() {
    return items;
  },
  /** pinned apps, those in folder stacks included */
  get pinnedEntries(): PinnedEntry[] {
    return pinnedEntries(items);
  },
  hasModule(module: ModuleId): boolean {
    if (!items) return false;
    return [...items.left, ...items.center, ...items.right].some(
      (i) => i.type === "Module" && i.module === module,
    );
  },
};

async function save(next: DockItems) {
  items = next;
  items = await api.saveDockItems(next, SOURCE);
}

function without(list: DockItem[], predicate: (i: DockItem) => boolean) {
  return list.filter((i) => !predicate(i));
}

export const dockItemsActions = {
  async init() {
    await on<{ source: string; store: string | null; items: DockItems }>(Events.DockItemsChanged, (p) => {
      // auxiliary docks have their own items
      if (p.source !== SOURCE && !p.store) items = p.items;
    });
    items = await api.getDockItems();
  },
  /** removes an app from the dock, or from the folder stack holding it */
  async removeItem(id: string) {
    if (!items) return;
    await save(withoutApp(items, id));
  },
  async toggleModule(module: ModuleId, enabled: boolean) {
    if (!items) return;
    const match = (i: DockItem) => i.type === "Module" && i.module === module;
    if (enabled) {
      if (dockItemsState.hasModule(module)) return;
      await save({ ...items, right: [...items.right, { id: `module-${module}`, type: "Module", module }] });
    } else {
      await save({
        ...items,
        left: without(items.left, match),
        center: without(items.center, match),
        right: without(items.right, match),
      });
    }
  },
  async reset() {
    items = await api.resetDockItems();
  },
};
