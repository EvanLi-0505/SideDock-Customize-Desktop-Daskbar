// Context menus of the dock. Port of Seelen UI `weg/appMenu.ts`, `dockMenu.ts`
// and `generalMenu.ts`.

import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { api, iconUrl } from "@shared/ipc.ts";
import { t } from "@shared/i18n/index.svelte.ts";
import type { MenuEntry } from "@shared/menu.ts";
import { MODULES } from "@shared/modules.ts";
import type { AppDockItem, DockItem, ModuleId, UserAppWindow } from "@shared/types.ts";
import { openMenu, openPopup } from "./overlays.ts";
import { dockActions, dockState, isHardcodedSeparator } from "./state/items.svelte.ts";
import { layout } from "./state/layout.svelte.ts";

// ---------------- launching ----------------

export function launchItem(item: AppDockItem, elevated = false): void {
  if (item.relaunch) {
    api.launch(item.relaunch.command, item.relaunch.args, item.relaunch.workingDir, elevated);
    return;
  }
  const lower = item.path.toLowerCase();
  const packaged =
    !lower || lower.includes("\\windowsapps\\") || lower.endsWith("applicationframehost.exe");
  if (item.umid && packaged) {
    api.launch(`shell:AppsFolder\\${item.umid}`, null, null, false);
  } else if (item.path) {
    api.launch(item.path, null, null, elevated);
  }
}

/**
 * Primary click on an app: focus/minimize its window, list its windows, or launch it.
 * `anchor` is the element popups (window list) are attached to.
 */
export function activateApp(item: AppDockItem, windows: UserAppWindow[], anchor: Element): void {
  if (windows.length > 1) {
    openPopup(anchor, "windows", { title: item.displayName, windows: $state.snapshot(windows) }, {
      key: `windows:${item.id}`,
    });
    return;
  }
  const win = windows[0];
  if (win) api.windowToggle(win.hwnd);
  else launchItem(item);
}

// ---------------- app item ----------------

export function showAppMenu(e: MouseEvent, item: AppDockItem, windows: UserAppWindow[]): void {
  const entries: MenuEntry[] = [];

  if (!item.preventPinning) {
    entries.push(
      item.pinned
        ? { type: "item", id: "unpin", label: t("app_menu.unpin"), icon: "PinOff" }
        : { type: "item", id: "pin", label: t("app_menu.pin"), icon: "Pin" },
      { type: "separator" },
    );
  }

  entries.push({
    type: "item",
    id: "run",
    label: item.displayName,
    image: iconUrl(item.relaunch?.command || item.path, item.umid),
  });
  if (item.path) {
    entries.push(
      { type: "item", id: "open_location", label: t("app_menu.open_file_location"), icon: "FolderOpen" },
      { type: "item", id: "run_as", label: t("app_menu.run_as"), icon: "ShieldCheck" },
    );
  }

  if (windows.length) {
    entries.push(
      { type: "separator" },
      {
        type: "item",
        id: "close",
        label: windows.length > 1 ? t("app_menu.close_multiple") : t("app_menu.close"),
        icon: "X",
        danger: true,
      },
    );
    if (layout.settings.showEndTask) {
      entries.push({
        type: "item",
        id: "kill",
        label: windows.length > 1 ? t("app_menu.kill_multiple") : t("app_menu.kill"),
        icon: "XCircle",
        danger: true,
      });
    }
  }

  openMenu(e, entries, (action) => {
    switch (action) {
      case "pin":
        dockActions.pin(item.id);
        break;
      case "unpin":
        if (windows.length) dockActions.unpin(item.id);
        else dockActions.remove(item.id);
        break;
      case "run":
        launchItem(item);
        break;
      case "run_as":
        launchItem(item, true);
        break;
      case "open_location":
        api.revealPath(item.path);
        break;
      case "close":
        windows.forEach((w) => api.windowClose(w.hwnd));
        break;
      case "kill":
        windows.forEach((w) => api.windowKill(w.hwnd));
        break;
    }
  });
}

// ---------------- dock background ----------------

async function pinCustomFile() {
  const files = await openDialog({
    title: t("dock_menu.add_file"),
    multiple: true,
    directory: false,
  });
  const list = Array.isArray(files) ? files : files ? [files] : [];
  if (list.length) await api.pinPaths(list);
}

export function showDockMenu(e: MouseEvent): void {
  const cursor = { x: e.clientX, y: e.clientY };
  const modules: MenuEntry[] = MODULES.map((m) => ({
    type: "item" as const,
    id: `module:${m.id}`,
    label: t(`module.${m.id}` as never),
    icon: m.icon,
    checked: dockActions.hasModule(m.id),
  }));

  const entries: MenuEntry[] = [
    { type: "submenu", id: "modules", label: t("dock_menu.modules"), icon: "LayoutDashboard", items: modules },
    { type: "separator" },
    { type: "item", id: "add-file", label: t("dock_menu.add_file"), icon: "FilePlus2" },
    {
      type: "item",
      id: "add-separator",
      label: t("dock_menu.add_separator"),
      icon: layout.horizontal ? "SeparatorVertical" : "SeparatorHorizontal",
    },
    { type: "separator" },
    {
      type: "item",
      id: "reorder",
      label: t(dockState.isReorderDisabled ? "dock_menu.reorder_enable" : "dock_menu.reorder_disable"),
      icon: dockState.isReorderDisabled ? "LockOpen" : "Lock",
    },
    { type: "item", id: "task-manager", label: t("dock_menu.task_manager"), icon: "SquareActivity" },
    { type: "item", id: "settings", label: t("dock_menu.settings"), icon: "Settings" },
  ];

  openMenu(e, entries, (action, value) => {
    if (action.startsWith("module:")) {
      dockActions.toggleModule(action.slice(7) as ModuleId, !!(value as { checked?: boolean })?.checked);
      return;
    }
    switch (action) {
      case "add-file":
        pinCustomFile().catch(console.error);
        break;
      case "add-separator":
        dockActions.addSeparatorNear(cursor, layout.horizontal);
        break;
      case "reorder":
        dockState.isReorderDisabled = !dockState.isReorderDisabled;
        break;
      case "task-manager":
        api.shellAction("task-manager");
        break;
      case "settings":
        api.openSettings();
        break;
    }
  });
}

// ---------------- modules & separators ----------------

export function showItemMenu(e: MouseEvent, item: DockItem, extra: MenuEntry[] = [], onExtra?: (action: string) => void): void {
  if (item.type === "Separator" && isHardcodedSeparator(item)) return;
  const entries: MenuEntry[] = [...extra];
  if (extra.length) entries.push({ type: "separator" });
  entries.push({
    type: "item",
    id: "remove",
    label: item.type === "Separator" ? t("item_menu.remove_separator") : t("item_menu.remove"),
    icon: "Trash2",
    danger: true,
  });
  openMenu(e, entries, (action) => {
    if (action === "remove") dockActions.remove(item.id);
    else onExtra?.(action);
  });
}
