import { mount } from "svelte";
import "@shared/styles/reset.css";
import "@shared/styles/theme.css";
import "./overlay.css";
import { initSettings } from "@shared/state/settings.svelte.ts";
import { initStartApps } from "./state/apps.svelte.ts";
import App from "./App.svelte";

window.addEventListener("contextmenu", (e) => e.preventDefault());

await initSettings();
await initStartApps();
mount(App, { target: document.getElementById("root")! });
