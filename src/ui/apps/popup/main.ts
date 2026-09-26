import { mount } from "svelte";
import "@shared/styles/reset.css";
import "@shared/styles/theme.css";
import "./popup.css";
import { initSettings } from "@shared/state/settings.svelte.ts";
import App from "./App.svelte";

window.addEventListener("contextmenu", (e) => e.preventDefault());
window.addEventListener("keydown", (e) => {
  if (e.key === "Escape") {
    import("@shared/ipc.ts").then(({ api }) => api.popupClose());
  }
});

await initSettings();
mount(App, { target: document.getElementById("root")! });
