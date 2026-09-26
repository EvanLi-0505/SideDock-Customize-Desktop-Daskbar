// Port of Seelen UI `weg/state/items.svelte.ts`.
//
// The dock renders a flat list: left items, an invisible separator, center items,
// another invisible separator, right items. Dragging an item across one of the
// hardcoded separators moves it to another group.

import { api, Events, on } from "@shared/ipc.ts";
import { debounce } from "@shared/utils.ts";
import type {
  AppDockItem,
  DockItem,
  DockItems,
  ModuleId,
  SeparatorDockItem,
  UserAppWindow,
} from "@shared/types.ts";
import { systemState } from "./system.svelte.ts";

interface DockState {
  isReorderDisabled: boolean;
  items: DockItem[];
}

const CLIENT_ID = crypto.randomUUID();

export const HARDCODED_SEPARATOR_LEFT: SeparatorDockItem = {
  id: "hardcoded-separator-1",
  type: "Separator",
};
export const HARDCODED_SEPARATOR_RIGHT: SeparatorDockItem = {
  id: "hardcoded-separator-2",
  type: "Separator",
};

export function isHardcodedSeparator(item: DockItem): boolean {
  return item.id === HARDCODED_SEPARATOR_LEFT.id || item.id === HARDCODED_SEPARATOR_RIGHT.id;
}

function fromStored(stored: DockItems): DockState {
  const seen = new Set<string>([HARDCODED_SEPARATOR_LEFT.id, HARDCODED_SEPARATOR_RIGHT.id]);
  const sanitize = (items: DockItem[]) =>
    items.filter((item) => {
      if (seen.has(item.id)) return false;
      seen.add(item.id);
      return true;
    });
  return {
    isReorderDisabled: stored.isReorderDisabled,
    items: [
      ...sanitize(stored.left),
      HARDCODED_SEPARATOR_LEFT,
      ...sanitize(stored.center),
      HARDCODED_SEPARATOR_RIGHT,
      ...sanitize(stored.right),
    ],
  };
}

export function listToGroups(items: DockItem[], includeSeparators = false) {
  let idx1 = items.findIndex((i) => i.id === HARDCODED_SEPARATOR_LEFT.id);
  let idx2 = items.findIndex((i) => i.id === HARDCODED_SEPARATOR_RIGHT.id);
  if (idx1 > idx2) [idx1, idx2] = [idx2, idx1];
  return {
    left: items.slice(0, idx1),
    center: includeSeparators ? items.slice(idx1, idx2 + 1) : items.slice(idx1 + 1, idx2),
    right: items.slice(idx2 + 1),
  };
}

let dock = $state<DockState>({ isReorderDisabled: false, items: [] });
/** true while applying state that came from the backend (must not be saved back) */
let remoteUpdate = false;

export const dockState = {
  get items() {
    return dock.items;
  },
  set items(items: DockItem[]) {
    dock = { ...dock, items };
  },
  get isReorderDisabled() {
    return dock.isReorderDisabled;
  },
  set isReorderDisabled(value: boolean) {
    dock = { ...dock, isReorderDisabled: value };
  },
};

const save = debounce((state: DockState) => {
  api
    .saveDockItems({ isReorderDisabled: state.isReorderDisabled, ...listToGroups(state.items) }, CLIENT_ID)
    .catch((err) => console.error("saving dock items failed", err));
}, 600);

// ---------------- window <-> item matching ----------------

/**
 * Grouping rules (same as Seelen UI):
 *  1. the window has an AppUserModelID and the item too -> exact umid match
 *  2. otherwise match by executable path (item path or relaunch command)
 */
export function getWindowsForItem(item: AppDockItem, windows: UserAppWindow[]): UserAppWindow[] {
  const command = item.relaunch?.command.toLowerCase();
  const path = item.path.toLowerCase();
  return windows.filter((w) => {
    if (w.umid && item.umid) return w.umid === item.umid;
    const winPath = w.path?.toLowerCase() ?? "";
    if (!winPath) return false;
    if (w.umid && !item.umid && !item.pinned) return false;
    return command === winPath || path === winPath;
  });
}

function temporalId(key: string): string {
  let h = 0;
  for (let i = 0; i < key.length; i++) h = (Math.imul(31, h) + key.charCodeAt(i)) | 0;
  return `temp-${(h >>> 0).toString(16)}-${key.length}`;
}

// ---------------- actions ----------------

export const dockActions = {
  remove(id: string) {
    dockState.items = dock.items.filter((i) => i.id !== id);
  },
  pin(id: string) {
    dockState.items = dock.items.map((i) =>
      i.id === id && i.type === "App" ? { ...i, pinned: true } : i,
    );
  },
  unpin(id: string) {
    dockState.items = dock.items.map((i) => (i.id === id && i.type === "App" ? { ...i, pinned: false } : i));
  },
  hasModule(module: ModuleId): boolean {
    return dock.items.some((i) => i.type === "Module" && i.module === module);
  },
  toggleModule(module: ModuleId, enabled: boolean) {
    if (enabled && !this.hasModule(module)) {
      const items = [...dock.items, { id: `module-${module}`, type: "Module", module } as DockItem];
      dockState.items = items;
    } else if (!enabled) {
      dockState.items = dock.items.filter((i) => !(i.type === "Module" && i.module === module));
    }
  },
  /** Inserts a separator next to the rendered item closest to the cursor. */
  addSeparatorNear(cursor: { x: number; y: number }, horizontal: boolean) {
    const separator: SeparatorDockItem = { id: crypto.randomUUID(), type: "Separator" };
    const items = [...dock.items];
    let insertIdx = items.findIndex((i) => i.id === HARDCODED_SEPARATOR_RIGHT.id);
    let nearestId: string | undefined;
    let nearest = Infinity;
    let after = false;
    for (const el of document.querySelectorAll<HTMLElement>(".weg-item-drag-container")) {
      const r = el.getBoundingClientRect();
      const cx = r.left + r.width / 2;
      const cy = r.top + r.height / 2;
      const dist = Math.hypot(cursor.x - cx, cursor.y - cy);
      if (dist < nearest) {
        nearest = dist;
        nearestId = el.dataset.itemId;
        after = horizontal ? cursor.x > cx : cursor.y > cy;
      }
    }
    const idx = items.findIndex((i) => i.id === nearestId);
    if (idx !== -1) insertIdx = after ? idx + 1 : idx;
    items.splice(insertIdx, 0, separator);
    dockState.items = items;
  },
};

// ---------------- lifecycle ----------------

export async function initItems(): Promise<void> {
  await on<{ source: string; items: DockItems }>(Events.DockItemsChanged, ({ source, items }) => {
    if (source === CLIENT_ID) return;
    remoteUpdate = true;
    dock = fromStored(items);
  });
  remoteUpdate = true;
  dock = fromStored(await api.getDockItems());

  $effect.root(() => {
    // persist local changes
    $effect(() => {
      const snapshot = dock;
      if (remoteUpdate) {
        remoteUpdate = false;
        return;
      }
      save(snapshot);
    });

    // keep temporal (unpinned, running) items in sync with the open windows
    $effect(() => {
      const windows = systemState.windows;
      const state = dock;
      const apps = state.items.filter((i): i is AppDockItem => i.type === "App");

      const toRemove = new Set(
        apps.filter((i) => !i.pinned && getWindowsForItem(i, windows).length === 0).map((i) => i.id),
      );
      const remaining = apps.filter((i) => !toRemove.has(i.id));
      const uncovered = windows.filter(
        (w) => !remaining.some((item) => getWindowsForItem(item, [w]).length > 0),
      );

      const seen = new Set<string>();
      const added: AppDockItem[] = [];
      for (const w of uncovered) {
        const key = w.umid ?? w.path;
        if (!key || seen.has(key)) continue;
        seen.add(key);
        added.push({
          id: temporalId(key),
          type: "App",
          displayName: w.appName || w.title,
          umid: w.umid,
          path: w.path ?? "",
          pinned: false,
          preventPinning: !w.path,
          relaunch: null,
        });
      }
      if (toRemove.size === 0 && added.length === 0) return;

      const filtered = state.items.filter((i) => !toRemove.has(i.id));
      const rightIdx = filtered.findIndex((i) => i.id === HARDCODED_SEPARATOR_RIGHT.id);
      // only unpinned items are added/removed here and those are never persisted
      remoteUpdate = true;
      dock = {
        ...state,
        items: [...filtered.slice(0, rightIdx), ...added, ...filtered.slice(rightIdx)],
      };
    });
  });
}
