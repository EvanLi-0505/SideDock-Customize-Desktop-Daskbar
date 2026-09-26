// Typed wrappers around the backend commands (src/background/commands.rs) and events.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AnchorRect,
  AppInfo,
  AppSettings,
  ClearResult,
  DataKind,
  DockInfo,
  DockItems,
  FocusedApp,
  Placement,
  StorageUsage,
  SystemColors,
  SystemState,
  UserAppWindow,
} from "./types.ts";

export const Events = {
  SettingsChanged: "settings-changed",
  DockItemsChanged: "dock-items-changed",
  WindowsChanged: "windows-changed",
  FocusChanged: "focus-changed",
  SystemState: "system-state-changed",
  DockInfo: "dock-info",
  DockHidden: "dock-hidden",
  PopupRender: "popup-render",
  PopupClosed: "popup-closed",
  PopupAction: "popup-action",
  TooltipRender: "tooltip-render",
} as const;

export function on<T>(event: string, handler: (payload: T) => void): Promise<UnlistenFn> {
  return listen<T>(event, (e) => handler(e.payload));
}

/** Logs command failures instead of throwing, for fire-and-forget UI actions. */
function fire(promise: Promise<unknown>): void {
  promise.catch((err) => console.error(err));
}

export const api = {
  // settings & state
  getSettings: () => invoke<AppSettings>("get_settings"),
  saveSettings: (settings: AppSettings) => invoke<AppSettings>("save_settings", { settings }),
  resetSettings: (section: "dock" | "all") => invoke<AppSettings>("reset_settings", { section }),
  getDockItems: () => invoke<DockItems>("get_dock_items"),
  saveDockItems: (items: DockItems, source: string) =>
    invoke<DockItems>("save_dock_items", { items, source }),
  resetDockItems: () => invoke<DockItems>("reset_dock_items"),
  pinPaths: (paths: string[]) => invoke<number>("dock_pin_paths", { paths }),
  importTaskbarPins: () => invoke<number>("dock_import_taskbar_pins"),

  // dock widget
  dockReady: () => fire(invoke("dock_ready")),
  dockGetInfo: () => invoke<DockInfo | null>("dock_get_info"),
  dockSetContentLength: (length: number) => fire(invoke("dock_set_content_length", { length })),
  dockSetDragging: (dragging: boolean) => fire(invoke("dock_set_dragging", { dragging })),

  // windows
  getWindows: () => invoke<UserAppWindow[]>("get_windows"),
  getFocused: () => invoke<FocusedApp>("get_focused"),
  windowToggle: (hwnd: number) => fire(invoke("window_toggle", { hwnd })),
  windowFocus: (hwnd: number) => fire(invoke("window_focus", { hwnd })),
  windowClose: (hwnd: number) => fire(invoke("window_close", { hwnd })),
  windowKill: (hwnd: number) => fire(invoke("window_kill", { hwnd })),

  // shell
  launch: (program: string, args: string | null, workingDir: string | null, elevated = false) =>
    fire(invoke("launch", { program, args, workingDir, elevated })),
  revealPath: (path: string) => fire(invoke("reveal_path", { path })),
  shellAction: (action: string) => fire(invoke("shell_action", { action })),

  // system modules
  getSystemState: () => invoke<SystemState>("get_system_state"),
  setKeyboardLayout: (id: string) => fire(invoke("set_keyboard_layout", { id })),
  setBluetooth: (enabled: boolean) => invoke("set_bluetooth", { enabled }),
  mediaCommand: (command: "togglePlayPause" | "next" | "previous") =>
    fire(invoke("media_command", { command })),
  powerAction: (action: "lock" | "signOut" | "sleep" | "hibernate" | "restart" | "shutdown") =>
    fire(invoke("power_action", { action })),
  emptyRecycleBin: () => fire(invoke("empty_recycle_bin")),

  // popup & tooltip
  popupOpen: (request: {
    kind: string;
    data?: unknown;
    anchor: AnchorRect;
    placement: Placement;
    /** "start" opens like a native context menu at the anchor, "center" centers on it */
    align?: "center" | "start";
    key?: string;
  }) => fire(invoke("popup_open", { request })),
  popupReady: (token: number, width: number, height: number) =>
    fire(invoke("popup_ready", { token, width, height })),
  popupResize: (token: number, width: number, height: number) =>
    fire(invoke("popup_resize", { token, width, height })),
  popupAction: (kind: string, action: string, value: unknown, close: boolean) =>
    fire(invoke("popup_action", { payload: { kind, action, value: value ?? null }, close })),
  popupClose: () => fire(invoke("popup_close")),
  tooltipShow: (text: string, anchor: AnchorRect, placement: Placement) =>
    fire(invoke("tooltip_show", { text, anchor, placement })),
  tooltipReady: (token: number, width: number, height: number) =>
    fire(invoke("tooltip_ready", { token, width, height })),
  tooltipHide: () => fire(invoke("tooltip_hide")),

  // app
  openSettings: () => fire(invoke("open_settings")),
  settingsReady: () => fire(invoke("settings_ready")),
  getAppInfo: () => invoke<AppInfo>("get_app_info"),
  getSystemColors: () => invoke<SystemColors>("get_system_colors"),
  getStorageUsage: () => invoke<StorageUsage>("get_storage_usage"),
  openDataDir: (kind: DataKind) => invoke("open_data_dir", { kind }),
  clearData: (kind: DataKind) => invoke<ClearResult>("clear_data", { kind }),
  restartApp: () => fire(invoke("restart_app")),
  quitApp: () => fire(invoke("quit_app")),
};

/** Icon url served by the backend `sdicon` protocol. */
export function iconUrl(path: string | null | undefined, umid?: string | null): string {
  const params = new URLSearchParams();
  if (path) params.set("path", path);
  if (umid) params.set("umid", umid);
  return `http://sdicon.localhost/?${params.toString()}`;
}

export function bootstrap() {
  return (
    window.__SIDEDOCK__ ?? {
      label: "unknown",
      widget: "unknown",
      monitorId: null,
    }
  );
}
