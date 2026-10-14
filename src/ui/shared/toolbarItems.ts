// Toolbar item catalog. Keep in sync with `TOOLBAR_ITEMS` in src/background/state/settings.rs.
import type { IconName } from "./components/Icon.svelte";
import { t, type TranslationKey } from "./i18n/index.svelte.ts";
import type { CustomButton, ToolbarItemId, ToolbarItems } from "./types.ts";

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
  { id: "cpu", icon: "Cpu", label: "toolbarItem.cpu" },
  { id: "memory", icon: "MemoryStick", label: "toolbarItem.memory" },
  { id: "net-speed", icon: "ArrowDownUp", label: "toolbarItem.net-speed" },
];

export const TOOLBAR_SECTIONS = ["left", "center", "right"] as const;
export type ToolbarSection = (typeof TOOLBAR_SECTIONS)[number];

export const CUSTOM_PREFIX = "custom:";

export function customItemId(button: CustomButton): ToolbarItemId {
  return `custom:${button.id}`;
}

/** The custom button an item stands for, if it is one. */
export function customButtonOf(id: ToolbarItemId, buttons: CustomButton[]): CustomButton | undefined {
  if (!id.startsWith(CUSTOM_PREFIX)) return undefined;
  const key = id.slice(CUSTOM_PREFIX.length);
  return buttons.find((b) => b.id === key);
}

/** Name and icon of any item, built-in or custom. */
export function describeItem(id: ToolbarItemId, buttons: CustomButton[]): { icon: IconName; label: string } {
  const custom = customButtonOf(id, buttons);
  if (custom) return { icon: (custom.icon || "MousePointerClick") as IconName, label: custom.name || custom.target };
  const meta = TOOLBAR_ITEMS.find((i) => i.id === id);
  return meta ? { icon: meta.icon, label: t(meta.label) } : { icon: "Puzzle", label: id };
}

/** Items in no section (not shown), custom buttons included. */
export function unusedToolbarItems(items: ToolbarItems, buttons: CustomButton[]): ToolbarItemId[] {
  const used = new Set([...items.left, ...items.center, ...items.right]);
  return [...TOOLBAR_ITEMS.map((i) => i.id), ...buttons.map(customItemId)].filter((id) => !used.has(id));
}

/** "1.2 MB/s", "34 KB/s" */
export function formatSpeed(bytesPerSecond: number): string {
  const kb = bytesPerSecond / 1024;
  if (kb < 1000) return `${Math.round(kb)} KB/s`;
  const mb = kb / 1024;
  return `${mb < 100 ? mb.toFixed(1) : Math.round(mb)} MB/s`;
}

/** "7.7 GB" */
export function formatGb(bytes: number): string {
  return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
}
