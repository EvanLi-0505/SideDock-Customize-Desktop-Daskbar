import { api, Events, on } from "@shared/ipc.ts";
import type { DockInfo, FocusedApp, SystemState, UserAppWindow } from "@shared/types.ts";

let dockInfo = $state<DockInfo | null>(null);
let windows = $state<UserAppWindow[]>([]);
let focused = $state<FocusedApp | null>(null);
let system = $state<SystemState>({
  power: null,
  keyboard: [],
  recycleBin: null,
  media: null,
  bluetooth: null,
});
let hidden = $state(false);

export const systemState = {
  get dock() {
    return dockInfo;
  },
  get windows() {
    return windows;
  },
  get focused() {
    return focused;
  },
  get system() {
    return system;
  },
  get hidden() {
    return hidden;
  },
};

export async function initSystemState(): Promise<void> {
  // subscribe first so no event is lost between the fetch and the listen
  await Promise.all([
    on<DockInfo>(Events.DockInfo, (info) => {
      dockInfo = info;
      hidden = info.hidden;
    }),
    on<UserAppWindow[]>(Events.WindowsChanged, (list) => (windows = list)),
    on<FocusedApp>(Events.FocusChanged, (f) => (focused = f)),
    on<SystemState>(Events.SystemState, (s) => (system = s)),
    on<{ hidden: boolean }>(Events.DockHidden, (p) => (hidden = p.hidden)),
  ]);
  const [info, list, focus, sys] = await Promise.all([
    api.dockGetInfo(),
    api.getWindows(),
    api.getFocused(),
    api.getSystemState(),
  ]);
  if (info) {
    dockInfo = info;
    hidden = info.hidden;
  }
  windows = list;
  focused = focus;
  system = sys;
}
