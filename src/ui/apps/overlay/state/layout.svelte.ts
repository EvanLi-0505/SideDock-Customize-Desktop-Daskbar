// The launcher's arrangement: apps placed by the user, collections (folders) and removed
// apps, persisted by the backend (state/launcher_layout.rs). What the home grid shows
// depends on Settings > Launcher: every app, or only the apps the user added.

import { t } from "@shared/i18n/index.svelte.ts";
import { api, Events, on } from "@shared/ipc.ts";
import { settingsState } from "@shared/state/settings.svelte.ts";
import type { LauncherEntry, LauncherLayout, StartApp } from "@shared/types.ts";
import { startApps } from "./apps.svelte.ts";

export type FolderEntry = Extract<LauncherEntry, { type: "Folder" }>;

export type Tile =
  | { kind: "app"; id: string; app: StartApp }
  | { kind: "folder"; id: string; name: string; apps: StartApp[]; entry: FolderEntry };

let layout = $state<LauncherLayout>({ items: [], removed: [] });

export async function initLayout(): Promise<void> {
  await on<LauncherLayout>(Events.LauncherLayoutChanged, (next) => (layout = next));
  layout = await api.getLauncherLayout();
}

const appById = $derived(new Map(startApps.list.map((a) => [a.id, a])));

/** apps placed in the layout (directly or in a collection) */
function placedIds(items: LauncherEntry[]): Set<string> {
  const ids = new Set<string>();
  for (const e of items) {
    if (e.type === "App") ids.add(e.id);
    else e.apps.forEach((a) => ids.add(a));
  }
  return ids;
}

function tileOf(entry: LauncherEntry): Tile | null {
  if (entry.type === "App") {
    const app = appById.get(entry.id);
    return app ? { kind: "app", id: app.id, app } : null;
  }
  const apps = entry.apps.map((id) => appById.get(id)).filter((a): a is StartApp => !!a);
  return apps.length ? { kind: "folder", id: entry.id, name: entry.name, apps, entry } : null;
}

export const launcherLayout = {
  get value(): LauncherLayout {
    return layout;
  },
  get onlyAdded(): boolean {
    return settingsState.value.launcher.content === "Pinned";
  },
  /** what the home grid shows, in order */
  get tiles(): Tile[] {
    const tiles = layout.items.map(tileOf).filter((x): x is Tile => !!x);
    if (launcherLayout.onlyAdded) return tiles;
    // every other app follows, alphabetically (startApps.list is sorted)
    const placed = placedIds(layout.items);
    const removed = new Set(layout.removed);
    for (const app of startApps.list) {
      if (!placed.has(app.id) && !removed.has(app.id)) tiles.push({ kind: "app", id: app.id, app });
    }
    return tiles;
  },
  /** the app shows on the home grid (directly or in a collection) */
  isShown(id: string): boolean {
    if (placedIds(layout.items).has(id)) return true;
    return !launcherLayout.onlyAdded && !layout.removed.includes(id);
  },
  folderOf(appId: string): FolderEntry | undefined {
    return layout.items.find((e): e is FolderEntry => e.type === "Folder" && e.apps.includes(appId));
  },
};

// ---------------- editing ----------------

/**
 * The home grid as explicit entries (tile order), followed by the entries whose apps are
 * not installed right now (kept for when they come back). Edits work on tile indexes.
 */
function explicit(): LauncherEntry[] {
  const tiles = launcherLayout.tiles;
  const entries: LauncherEntry[] = tiles.map((tile) =>
    tile.kind === "app" ? { type: "App", id: tile.id } : $state.snapshot(tile.entry),
  );
  const shown = new Set(tiles.map((t) => t.id));
  for (const e of layout.items) if (!shown.has(e.id)) entries.push($state.snapshot(e));
  return entries;
}

function save(items: LauncherEntry[], removed = layout.removed) {
  const next: LauncherLayout = { items, removed: [...removed] };
  layout = next; // optimistic: the grid follows the drop at once
  api.saveLauncherLayout(next).catch((err) => {
    console.error(err);
    api.getLauncherLayout().then((l) => (layout = l));
  });
}

const entryId = (e: LauncherEntry) => e.id;

/** removes an app wherever it is (top level or in a collection) */
function withoutApp(items: LauncherEntry[], appId: string): LauncherEntry[] {
  return items
    .filter((e) => !(e.type === "App" && e.id === appId))
    .map((e) => (e.type === "Folder" ? { ...e, apps: e.apps.filter((a) => a !== appId) } : e))
    .filter((e) => e.type === "App" || e.apps.length > 0);
}

export const layoutActions = {
  /** moves a tile (app or collection) to `index` among the tiles without it */
  move(id: string, index: number) {
    let items = explicit();
    const entry = items.find((e) => entryId(e) === id) ?? { type: "App" as const, id };
    items = items.filter((e) => entryId(e) !== id);
    items.splice(Math.min(index, items.length), 0, entry);
    save(items);
  },

  /** takes an app out of its collection and places it at `index` */
  moveOutOfFolder(appId: string, index: number) {
    const before = explicit();
    const folderAt = before.findIndex((e) => e.type === "Folder" && e.apps.includes(appId));
    const items = withoutApp(before, appId);
    // its collection disappeared with its last app: later tiles moved up
    if (folderAt >= 0 && items.length < before.length && folderAt < index) index -= 1;
    items.splice(Math.min(index, items.length), 0, { type: "App", id: appId });
    save(items);
  },

  /** drops app `appId` onto tile `targetId`: into its collection, or a new one with both */
  merge(appId: string, targetId: string) {
    if (appId === targetId) return;
    let items = withoutApp(explicit(), appId);
    const at = items.findIndex((e) => entryId(e) === targetId);
    if (at < 0) return;
    const target = items[at]!;
    if (target.type === "Folder") {
      items[at] = { ...target, apps: [...target.apps, appId] };
    } else {
      items[at] = { type: "Folder", id: crypto.randomUUID(), name: t("launcher.newFolder"), apps: [target.id, appId] };
    }
    items = items.filter((e) => e.type === "App" || e.apps.length > 0);
    save(items);
  },

  /** a collection holding only this app, in its place */
  newFolder(appId: string): string {
    const id = crypto.randomUUID();
    let items = explicit();
    const at = items.findIndex((e) => e.type === "App" && e.id === appId);
    const folder: LauncherEntry = { type: "Folder", id, name: t("launcher.newFolder"), apps: [appId] };
    if (at >= 0) {
      items[at] = folder;
    } else {
      // it was in another collection
      items = withoutApp(items, appId);
      items.push(folder);
    }
    save(items);
    return id;
  },

  rename(folderId: string, name: string) {
    const trimmed = name.trim();
    if (!trimmed) return;
    save(layout.items.map((e) => (e.type === "Folder" && e.id === folderId ? { ...e, name: trimmed } : e)));
  },

  /** reorders inside a collection */
  moveInFolder(folderId: string, appId: string, index: number) {
    save(
      layout.items.map((e) => {
        if (e.type !== "Folder" || e.id !== folderId) return e;
        const apps = e.apps.filter((a) => a !== appId);
        apps.splice(Math.min(index, apps.length), 0, appId);
        return { ...e, apps };
      }),
    );
  },

  /** dissolves a collection: its apps take its place */
  ungroup(folderId: string) {
    const items = explicit().flatMap((e): LauncherEntry[] =>
      e.type === "Folder" && e.id === folderId ? e.apps.map((id) => ({ type: "App", id })) : [e],
    );
    save(items);
  },

  /** off the home grid (search and "All apps" still find it) */
  remove(appId: string) {
    const items = withoutApp(explicit(), appId);
    const removed = launcherLayout.onlyAdded ? layout.removed : [...layout.removed, appId];
    save(items, removed);
  },

  /** back on the home grid (at the end when only added apps are shown) */
  add(appId: string) {
    const removed = layout.removed.filter((a) => a !== appId);
    const items = [...layout.items];
    if (launcherLayout.onlyAdded && !placedIds(items).has(appId)) items.push({ type: "App", id: appId });
    save(items, removed);
  },
};
