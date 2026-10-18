import { mount } from "svelte";
import "@shared/styles/reset.css";
import "@shared/styles/theme.css";
import "./overlay.css";
import { initSettings } from "@shared/state/settings.svelte.ts";
import { initStartApps } from "./state/apps.svelte.ts";
import { initLayout } from "./state/layout.svelte.ts";
import App from "./App.svelte";
import { installIconRetry } from "@shared/iconRetry.ts";

installIconRetry();

window.addEventListener("contextmenu", (e) => e.preventDefault());

await initSettings();
await initStartApps();
await initLayout();
mount(App, { target: document.getElementById("root")! });
