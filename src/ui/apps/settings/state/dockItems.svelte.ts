import { api, Events, on } from "@shared/ipc.ts";
import type { AppDockItem, DockItem, DockItems, ModuleId } from "@shared/types.ts";

const SOURCE = "settings";

let items = $state<DockItems | null>(null);

export const dockItemsState = {
  get value() {
    return items;
  },
  get pinnedApps(): AppDockItem[] {
    if (!items) return [];
    return [...items.left, ...items.center, ...items.right].filter(
      (i): i is AppDockItem => i.type === "App" && i.pinned,
    );
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
    await on<{ source: string; items: DockItems }>(Events.DockItemsChanged, (p) => {
      if (p.source !== SOURCE) items = p.items;
    });
    items = await api.getDockItems();
  },
  async removeItem(id: string) {
    if (!items) return;
    const match = (i: DockItem) => i.id === id;
    await save({
      ...items,
      left: without(items.left, match),
      center: without(items.center, match),
      right: without(items.right, match),
    });
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
