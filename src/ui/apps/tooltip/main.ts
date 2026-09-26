import "@shared/styles/reset.css";
import "@shared/styles/theme.css";
import "./tooltip.css";
import { api, Events, on } from "@shared/ipc.ts";
import { initSettings } from "@shared/state/settings.svelte.ts";

// plain DOM: the tooltip is too small to deserve a component tree
await initSettings();

const bubble = document.createElement("div");
bubble.className = "tooltip";
document.getElementById("root")!.append(bubble);

await on<{ token: number; text: string }>(Events.TooltipRender, ({ token, text }) => {
  bubble.textContent = text;
  requestAnimationFrame(() => {
    const r = bubble.getBoundingClientRect();
    api.tooltipReady(token, Math.ceil(r.width), Math.ceil(r.height));
  });
});
