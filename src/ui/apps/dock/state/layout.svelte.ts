import { settingsState } from "@shared/state/settings.svelte.ts";
import type { DockSettings, Placement } from "@shared/types.ts";
import { systemState } from "./system.svelte.ts";

export const layout = {
  /** this dock's settings: an auxiliary dock gets its own from the backend */
  get settings(): DockSettings {
    return systemState.dock?.settings ?? settingsState.value.dock;
  },
  /** an auxiliary dock: pinned items only, no running apps, no modules */
  get aux(): boolean {
    return !!systemState.dock?.aux;
  },
  get side(): "left" | "right" | "top" | "bottom" {
    return layout.settings.position.toLowerCase() as "left" | "right" | "top" | "bottom";
  },
  get horizontal(): boolean {
    const p = layout.settings.position;
    return p === "Top" || p === "Bottom";
  },
  /** where popups and tooltips open relative to the dock */
  get placement(): Placement {
    switch (layout.settings.position) {
      case "Left":
        return "right";
      case "Right":
        return "left";
      case "Top":
        return "below";
      default:
        return "above";
    }
  },
};

/** Pushes the size settings to CSS variables (same names as Seelen UI). */
export function applyLayoutVariables(): void {
  $effect.root(() => {
    $effect(() => {
      const { size, padding, margin, spaceBetweenItems, backgroundOpacity } = layout.settings;
      const root = document.documentElement;
      root.style.setProperty("--config-margin", `${margin}px`);
      root.style.setProperty("--config-padding", `${padding}px`);
      root.style.setProperty("--config-item-size", `${size}px`);
      root.style.setProperty("--config-space-between-items", `${spaceBetweenItems}px`);
      root.style.setProperty("--dock-alpha", `${backgroundOpacity / 100}`);
      // auxiliary docks show no running indicators (dock.css)
      if (layout.aux) root.dataset.aux = "";
      else delete root.dataset.aux;
    });
  });
}
