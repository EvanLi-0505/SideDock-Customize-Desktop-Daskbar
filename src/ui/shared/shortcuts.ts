// Global shortcuts: action catalog and accelerator helpers.
// Key names must match `key_code` in src/background/modules/hotkeys.rs, and the
// defaults `SHORTCUT_DEFAULTS` in src/background/state/settings.rs.

export interface ShortcutAction {
  id: string;
  defaultAccelerator: string;
}

export const SHORTCUT_ACTIONS: ShortcutAction[] = [
  { id: "toggle-dock", defaultAccelerator: "Ctrl+Alt+Shift+D" },
  { id: "open-settings", defaultAccelerator: "Ctrl+Alt+Shift+S" },
  { id: "open-launcher", defaultAccelerator: "Alt+Shift+Space" },
  { id: "window-switcher", defaultAccelerator: "Alt+Backquote" },
];

export function defaultShortcuts(): Record<string, string> {
  return Object.fromEntries(SHORTCUT_ACTIONS.map((a) => [a.id, a.defaultAccelerator]));
}

const NAMED_CODES: Record<string, string> = {
  Space: "Space",
  Tab: "Tab",
  Enter: "Enter",
  Backspace: "Backspace",
  Delete: "Delete",
  Insert: "Insert",
  Home: "Home",
  End: "End",
  PageUp: "PageUp",
  PageDown: "PageDown",
  ArrowLeft: "Left",
  ArrowUp: "Up",
  ArrowRight: "Right",
  ArrowDown: "Down",
  Backquote: "Backquote",
  Minus: "Minus",
  Equal: "Equal",
  BracketLeft: "BracketLeft",
  BracketRight: "BracketRight",
  Backslash: "Backslash",
  Semicolon: "Semicolon",
  Quote: "Quote",
  Comma: "Comma",
  Period: "Period",
  Slash: "Slash",
};

/** Physical key (layout independent) -> token, or null for modifiers / unsupported keys. */
function keyToken(code: string): string | null {
  let m = /^Key([A-Z])$/.exec(code);
  if (m) return m[1]!;
  m = /^Digit([0-9])$/.exec(code);
  if (m) return m[1]!;
  m = /^F([0-9]{1,2})$/.exec(code);
  if (m && Number(m[1]) >= 1 && Number(m[1]) <= 24) return `F${m[1]}`;
  m = /^Numpad([0-9])$/.exec(code);
  if (m) return `Numpad${m[1]}`;
  return NAMED_CODES[code] ?? null;
}

/** Builds `Ctrl+Alt+Backquote` from a keydown event; null while only modifiers are held. */
export function acceleratorFromEvent(e: KeyboardEvent): string | null {
  const key = keyToken(e.code);
  if (!key) return null;
  const parts: string[] = [];
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  if (e.metaKey) parts.push("Win");
  parts.push(key);
  return parts.join("+");
}

/** Same rule as the backend: needs Ctrl, Alt or Win, unless it is a function key. */
export function isValidAccelerator(accelerator: string): boolean {
  const parts = accelerator.split("+");
  const key = parts.at(-1) ?? "";
  if (/^F([1-9]|1[0-9]|2[0-4])$/.test(key)) return true;
  return parts.some((p) => p === "Ctrl" || p === "Alt" || p === "Win");
}

const DISPLAY: Record<string, string> = {
  Backquote: "`",
  Minus: "-",
  Equal: "=",
  BracketLeft: "[",
  BracketRight: "]",
  Backslash: "\\",
  Semicolon: ";",
  Quote: "'",
  Comma: ",",
  Period: ".",
  Slash: "/",
  Left: "←",
  Up: "↑",
  Right: "→",
  Down: "↓",
};

/** `Ctrl+Alt+Backquote` -> ["Ctrl", "Alt", "`"] for key caps. */
export function acceleratorKeys(accelerator: string): string[] {
  if (!accelerator) return [];
  return accelerator.split("+").map((p) => DISPLAY[p] ?? p.replace(/^Numpad/, "Num "));
}
