# Changelog

## 1.0.0 - 2026-09-26

First public release.

### Dock

- Dock ported from Seelen UI (SeelenWeg) that **keeps the native Windows taskbar**: it can
  sit on the left, right, top or bottom while the taskbar stays where it is.
- Full width or fit-content, margin/padding/item size, auto hide (never / always / on
  overlap) with show/hide delays, hidden while a fullscreen app is focused.
- Pinned apps, files and shortcuts (drag & drop from Explorer, file picker, one-click import
  from the Windows taskbar); running windows grouped per app with open/focus indicators,
  window counter, split windows, window titles.
- Left / center / right groups, custom separators, drag to reorder, lock.
- Click to focus/minimize, window list for apps with several windows, middle click to close
  or open a new instance, context menus (pin, open file location, run as administrator,
  close, end task).
- Primary monitor only or every monitor.

### Modules

- Start menu, show desktop, recycle bin, clock & calendar, keyboard layout selector,
  media player, power & battery, bluetooth, network, notifications, task manager.

### Settings

- Home page with dock status, theme picker, language, start with Windows and storage cards.
- Themes: glass (frosted), glass (clear), dark, light and follow system; custom accent color.
- Chinese and English.
- Cards to open and clean logs, user data, icon cache and configuration.

### Reliability

- Portable: everything is stored in `data/` next to the executable, nothing in the user
  profile (the only exception is the optional autostart registry value).
- Single instance: launching again only focuses the running instance.
- Crash guardian frees the reserved screen space and restarts SideDock after an unexpected
  exit (stops after 3 crashes in 2 minutes, never during logoff/shutdown).
- Atomic configuration writes with automatic backup recovery; supervised background threads.
- Shell hosts (Start menu, search, action center) are never treated as app windows; the dock
  repairs itself if another program hides it.
