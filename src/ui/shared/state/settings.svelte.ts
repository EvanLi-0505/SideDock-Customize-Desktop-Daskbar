import { api, Events, on } from "../ipc.ts";
import { i18n } from "../i18n/index.svelte.ts";
import type { AppSettings } from "../types.ts";

let settings = $state<AppSettings | null>(null);
let systemAccent = $state("#3b82f6");
let prefersDark = $state(window.matchMedia("(prefers-color-scheme: dark)").matches);

window
  .matchMedia("(prefers-color-scheme: dark)")
  .addEventListener("change", (e) => (prefersDark = e.matches));

export const settingsState = {
  /** Only access after `initSettings()` resolved. */
  get value(): AppSettings {
    return settings!;
  },
  get ready(): boolean {
    return settings !== null;
  },
  get scheme(): "dark" | "light" {
    const theme = settings?.theme ?? "glass";
    if (theme === "dark") return "dark";
    if (theme === "light") return "light";
    return prefersDark ? "dark" : "light";
  },
  get accent(): string {
    return settings?.accentColor || systemAccent;
  },
};

function applyTheme() {
  if (!settings) return;
  const root = document.documentElement;
  const glass = settings.theme === "glass" || settings.theme === "clear";
  root.dataset.theme = glass ? settings.theme : "solid";
  // shared rules for both glass variants (native backdrop, transparent body...)
  if (glass) root.dataset.glass = "";
  else delete root.dataset.glass;
  root.dataset.scheme = settingsState.scheme;
  root.style.setProperty("--accent", settingsState.accent);
  i18n.language = settings.language;
}

export async function initSettings(): Promise<void> {
  const [initial, colors] = await Promise.all([api.getSettings(), api.getSystemColors()]);
  systemAccent = colors.accent;
  settings = initial;
  await on<AppSettings>(Events.SettingsChanged, (next) => {
    settings = next;
  });
  $effect.root(() => {
    $effect(applyTheme);
  });
}
