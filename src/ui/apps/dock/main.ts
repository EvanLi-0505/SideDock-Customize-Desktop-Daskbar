import { mount } from "svelte";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import "@shared/styles/reset.css";
import "@shared/styles/theme.css";
import "./styles/dock.css";
import { api } from "@shared/ipc.ts";
import { initSettings } from "@shared/state/settings.svelte.ts";
import App from "./App.svelte";
import { initItems } from "./state/items.svelte.ts";
import { initSystemState } from "./state/system.svelte.ts";
import { applyLayoutVariables } from "./state/layout.svelte.ts";
import { initOverlays } from "./overlays.ts";

// the dock never shows the browser context menu
window.addEventListener("contextmenu", (e) => e.preventDefault());

await initSettings();
await initSystemState();
await initItems();
await initOverlays();
applyLayoutVariables();

mount(App, { target: document.getElementById("root")! });

// files dropped from Explorer are pinned
getCurrentWebviewWindow().onDragDropEvent((e) => {
  if (e.payload.type === "drop" && e.payload.paths.length) {
    api.pinPaths(e.payload.paths).catch(console.error);
  }
});

requestAnimationFrame(() => api.dockReady());
