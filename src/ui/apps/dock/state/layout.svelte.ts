import { settingsState } from "@shared/state/settings.svelte.ts";
import type { DockSettings, Placement } from "@shared/types.ts";

export const layout = {
  get settings(): DockSettings {
    return settingsState.value.dock;
  },
  get side(): "left" | "right" | "top" | "bottom" {
    return settingsState.value.dock.position.toLowerCase() as "left" | "right" | "top" | "bottom";
  },
  get horizontal(): boolean {
    const p = settingsState.value.dock.position;
    return p === "Top" || p === "Bottom";
  },
  /** where popups and tooltips open relative to the dock */
  get placement(): Placement {
    switch (settingsState.value.dock.position) {
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
      const root = document.documentElement.style;
      root.setProperty("--config-margin", `${margin}px`);
      root.setProperty("--config-padding", `${padding}px`);
      root.setProperty("--config-item-size", `${size}px`);
      root.setProperty("--config-space-between-items", `${spaceBetweenItems}px`);
      root.setProperty("--dock-alpha", `${backgroundOpacity / 100}`);
    });
  });
}
