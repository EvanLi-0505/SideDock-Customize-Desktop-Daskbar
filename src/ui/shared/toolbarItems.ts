// Toolbar item catalog. Keep in sync with `TOOLBAR_ITEMS` in src/background/state/settings.rs.
import type { IconName } from "./components/Icon.svelte";
import type { TranslationKey } from "./i18n/index.svelte.ts";
import type { ToolbarItemId, ToolbarItems } from "./types.ts";

export interface ToolbarItemMeta {
  id: ToolbarItemId;
  icon: IconName;
  label: TranslationKey;
}

export const TOOLBAR_ITEMS: ToolbarItemMeta[] = [
  { id: "start", icon: "LayoutGrid", label: "toolbarItem.start" },
  { id: "app-name", icon: "LayoutDashboard", label: "toolbarItem.app-name" },
  { id: "media", icon: "Music", label: "toolbarItem.media" },
  { id: "tray", icon: "ChevronUp", label: "toolbarItem.tray" },
  { id: "keyboard", icon: "Keyboard", label: "toolbarItem.keyboard" },
  { id: "bluetooth", icon: "Bluetooth", label: "toolbarItem.bluetooth" },
  { id: "quick-settings", icon: "Wifi", label: "toolbarItem.quick-settings" },
  { id: "battery", icon: "BatteryMedium", label: "toolbarItem.battery" },
  { id: "notifications", icon: "Bell", label: "toolbarItem.notifications" },
  { id: "clock", icon: "Clock", label: "toolbarItem.clock" },
  { id: "task-manager", icon: "SquareActivity", label: "toolbarItem.task-manager" },
  { id: "show-desktop", icon: "Monitor", label: "toolbarItem.show-desktop" },
];

export const TOOLBAR_SECTIONS = ["left", "center", "right"] as const;
export type ToolbarSection = (typeof TOOLBAR_SECTIONS)[number];

export function toolbarItemMeta(id: ToolbarItemId): ToolbarItemMeta {
  return TOOLBAR_ITEMS.find((i) => i.id === id)!;
}

/** Items in no section (not shown). */
export function unusedToolbarItems(items: ToolbarItems): ToolbarItemId[] {
  const used = new Set([...items.left, ...items.center, ...items.right]);
  return TOOLBAR_ITEMS.map((i) => i.id).filter((id) => !used.has(id));
}
