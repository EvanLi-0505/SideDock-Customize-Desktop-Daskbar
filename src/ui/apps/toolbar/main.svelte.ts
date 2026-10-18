import { mount } from "svelte";
import "@shared/styles/reset.css";
import "@shared/styles/theme.css";
import "./toolbar.css";
import { api } from "@shared/ipc.ts";
import { initSettings, setThemeOverride, settingsState } from "@shared/state/settings.svelte.ts";
import App from "./App.svelte";
import { initOverlays } from "./overlays.ts";
import { initToolbarState } from "./state.svelte.ts";
import { installIconRetry } from "@shared/iconRetry.ts";

installIconRetry();

// the toolbar never shows the browser context menu
window.addEventListener("contextmenu", (e) => e.preventDefault());

await initSettings();
// its own theme (Settings > Toolbar), or the dock's
setThemeOverride(() => settingsState.value.toolbar.theme);
await initToolbarState();
await initOverlays();

$effect.root(() => {
  $effect(() => {
    document.documentElement.style.setProperty(
      "--dock-alpha",
      `${settingsState.value.toolbar.backgroundOpacity / 100}`,
    );
  });
});

mount(App, { target: document.getElementById("root")! });

requestAnimationFrame(() => api.toolbarReady());
