// Dock module catalog. Keep in sync with `MODULES` in src/background/state/dock_items.rs.
import type { IconName } from "./components/Icon.svelte";
import type { ModuleId } from "./types.ts";

export interface ModuleMeta {
  id: ModuleId;
  icon: IconName;
  /** takes 2 or 3 slots on the dock */
  size?: "medium" | "large";
}

export const MODULES: ModuleMeta[] = [
  { id: "start-menu", icon: "LayoutGrid" },
  { id: "show-desktop", icon: "Monitor" },
  { id: "recycle-bin", icon: "Trash2" },
  { id: "clock", icon: "Clock" },
  { id: "keyboard", icon: "Keyboard" },
  { id: "media", icon: "Music", size: "large" },
  { id: "power", icon: "BatteryMedium" },
  { id: "bluetooth", icon: "Bluetooth" },
  { id: "network", icon: "Wifi" },
  { id: "notifications", icon: "Bell" },
  { id: "task-manager", icon: "SquareActivity" },
];

export function moduleMeta(id: ModuleId): ModuleMeta {
  return MODULES.find((m) => m.id === id) ?? { id, icon: "Puzzle" };
}
