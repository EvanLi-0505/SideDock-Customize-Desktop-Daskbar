import { mount } from "svelte";
import "@shared/styles/reset.css";
import "@shared/styles/theme.css";
import "./settings.css";
import { api } from "@shared/ipc.ts";
import { initSettings } from "@shared/state/settings.svelte.ts";

window.addEventListener("contextmenu", (e) => {
  // keep the native menu for text fields only
  if (!(e.target instanceof HTMLInputElement)) e.preventDefault();
});

await initSettings();
// imported after the settings are loaded: the editor snapshots them on import
const [{ default: App }, { dockItemsActions }] = await Promise.all([
  import("./App.svelte"),
  import("./state/dockItems.svelte.ts"),
]);
await dockItemsActions.init();

mount(App, { target: document.getElementById("root")! });
requestAnimationFrame(() => api.settingsReady());
