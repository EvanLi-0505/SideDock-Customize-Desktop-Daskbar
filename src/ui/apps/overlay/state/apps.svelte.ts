// Start menu apps for the launcher, kept up to date by the backend (modules/start_apps.rs).

import { match, pinyin } from "pinyin-pro";
import { api, Events, on } from "@shared/ipc.ts";
import type { StartApp } from "@shared/types.ts";

// Like the Start menu, Chinese names sort by their pinyin among the Latin ones
// (暴雪战网 is under B, next to "Blender").
const collator = new Intl.Collator("en", { numeric: true, sensitivity: "base" });

function sortKey(name: string): string {
  return pinyin(name, { toneType: "none", type: "string", nonZh: "consecutive", v: true })
    .replace(/\s+/g, "")
    .toLowerCase();
}

let apps = $state<StartApp[]>([]);

function sorted(list: StartApp[]): StartApp[] {
  const keys = new Map(list.map((a) => [a.id, sortKey(a.name)]));
  return [...list].sort((a, b) => collator.compare(keys.get(a.id)!, keys.get(b.id)!));
}

export const startApps = {
  get list(): StartApp[] {
    return apps;
  },
};

export async function initStartApps(): Promise<void> {
  apps = sorted(await api.getStartApps());
  await on<StartApp[]>(Events.StartAppsChanged, (next) => (apps = sorted(next)));
}

/**
 * Filters by name: plain substring first, then pinyin (full, initials or mixed:
 * "weixin", "wx", "wxin" all find 微信). Better matches come first.
 */
export function search(list: StartApp[], query: string): StartApp[] {
  const q = query.trim().toLowerCase();
  if (!q) return list;
  const compact = q.replace(/\s+/g, "");
  const ranked: { app: StartApp; rank: number; at: number }[] = [];
  for (const app of list) {
    const name = app.name.toLowerCase();
    const at = name.indexOf(q);
    if (at === 0) {
      ranked.push({ app, rank: 0, at });
    } else if (at > 0) {
      ranked.push({ app, rank: 1, at });
    } else {
      const hit = match(app.name, compact, { continuous: true });
      if (hit && hit.length) ranked.push({ app, rank: 2, at: hit[0]! });
    }
  }
  ranked.sort((a, b) => a.rank - b.rank || a.at - b.at);
  return ranked.map((r) => r.app);
}
