import { api, Events, on } from "../ipc.ts";
import { i18n } from "../i18n/index.svelte.ts";
import type { AppSettings, ThemeMode } from "../types.ts";

let settings = $state<AppSettings | null>(null);
let systemAccent = $state("#3b82f6");
let prefersDark = $state(window.matchMedia("(prefers-color-scheme: dark)").matches);
/** a widget with its own theme (the toolbar, popups opened by it); null = global */
let themeOverride = $state<(() => ThemeMode | null) | null>(null);

window
  .matchMedia("(prefers-color-scheme: dark)")
  .addEventListener("change", (e) => (prefersDark = e.matches));

function schemeOf(theme: ThemeMode): "dark" | "light" {
  if (theme === "dark") return "dark";
  if (theme === "light") return "light";
  return prefersDark ? "dark" : "light";
}

export const settingsState = {
  /** Only access after `initSettings()` resolved. */
  get value(): AppSettings {
    return settings!;
  },
  get ready(): boolean {
    return settings !== null;
  },
  /** the theme this page uses (its override, or the global one) */
  get theme(): ThemeMode {
    return themeOverride?.() ?? settings?.theme ?? "glass";
  },
  get scheme(): "dark" | "light" {
    return schemeOf(settingsState.theme);
  },
  get accent(): string {
    return settings?.accentColor || systemAccent;
  },
};

/** Makes this page follow another theme than the global one (null: back to global). */
export function setThemeOverride(get: (() => ThemeMode | null) | null) {
  themeOverride = get;
}

function applyTheme() {
  if (!settings) return;
  const root = document.documentElement;
  const theme = settingsState.theme;
  const glass = theme === "glass" || theme === "clear";
  root.dataset.theme = glass ? theme : "solid";
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
