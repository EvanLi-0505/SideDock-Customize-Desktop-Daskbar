// Live state of a toolbar: its placement, the focused window, system modules, tray icons.

import { api, Events, on, onSelf } from "@shared/ipc.ts";
import type {
  FocusedApp,
  Placement,
  SystemState,
  ToolbarInfo,
  TrayIcon,
  UserAppWindow,
} from "@shared/types.ts";

let info = $state<ToolbarInfo | null>(null);
let windows = $state<UserAppWindow[]>([]);
let focused = $state<FocusedApp | null>(null);
let system = $state<SystemState>({
  power: null,
  keyboard: [],
  recycleBin: null,
  media: null,
  bluetooth: null,
  network: null,
  volume: null,
  perf: null,
});
let tray = $state<TrayIcon[]>([]);

export const toolbar = {
  get info() {
    return info;
  },
  /** popups open away from the screen edge */
  get placement(): Placement {
    return info?.edge === "top" ? "below" : "above";
  },
  get system() {
    return system;
  },
  get tray() {
    return tray;
  },
  get inlineTray() {
    return tray.filter((i) => i.inline);
  },
  get overflowTray() {
    return tray.filter((i) => !i.inline);
  },
  /** name of the application in the foreground (none for the desktop / SideDock) */
  get appName(): string | null {
    if (!focused || focused.isOwn || !focused.hwnd) return null;
    const window = windows.find((w) => w.hwnd === focused!.hwnd);
    return window?.appName || window?.title || null;
  },
};

export async function initToolbarState(): Promise<void> {
  // subscribe first so no event is lost between the fetch and the listen
  await Promise.all([
    onSelf<ToolbarInfo>(Events.ToolbarInfo, (i) => (info = i)),
    on<UserAppWindow[]>(Events.WindowsChanged, (list) => (windows = list)),
    on<FocusedApp>(Events.FocusChanged, (f) => (focused = f)),
    on<SystemState>(Events.SystemState, (s) => (system = s)),
    on<TrayIcon[]>(Events.SystemTrayChanged, (icons) => (tray = icons)),
  ]);
  const [i, list, focus, sys, icons] = await Promise.all([
    api.toolbarGetInfo(),
    api.getWindows(),
    api.getFocused(),
    api.getSystemState(),
    api.getTrayIcons(),
  ]);
  if (i) info = i;
  windows = list;
  focused = focus;
  system = sys;
  tray = icons;
}
