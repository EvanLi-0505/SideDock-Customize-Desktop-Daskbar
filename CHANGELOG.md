# Changelog

## 1.2.0 - 2026-09-30

### Launcher (SideDock start menu)

- New full-screen launcher inspired by macOS Launchpad: every app of the Start menu
  (classic programs and Store apps), paged grid, search by name, pinyin or initials
  ("wx" finds 微信), arrow keys / Enter / Esc, page dots, PageUp / PageDown.
- Paging built on native scrolling with one snap point per page: a touchpad swipe (either
  direction) turns exactly one page whatever its momentum, each mouse wheel notch turns
  one page.
- Opens reliably in the foreground (escalating activation like Seelen UI), so keyboard
  and wheel always reach it.
- Right-click an app: open, run as administrator, pin to the dock, open file location.
- Settings > "Start & switcher": the dock's start button opens the Windows start menu or
  the launcher. Launcher icon size.
- Optional: a single press of the Win key opens the launcher instead of the Windows
  start menu; Win + other keys (Win+E, Win+Shift+S, Win+L...) keep working. Windows 11
  only lets an elevated process keep its start menu closed, so the keyboard hook runs in
  a tiny administrator helper (same technique as Seelen UI's `win-hotkeys`). Turning the
  option on asks for administrator rights once and registers the scheduled task
  "SideDock Win Key" (no trigger, runs only while SideDock runs); turning it off removes
  the task. Everything else stays non-elevated.
- Shortcut `Alt+Shift+Space` (customizable).

### Window switcher (stage manager)

- `` Alt+` `` (customizable): live previews of every window, the selected one large in the
  center with the other windows of the same app below it, apps grouped on the left.
- Tap the shortcut: the switcher stays open, pick a window with the mouse (or Tab /
  arrows / Enter). Hold Alt and press the key repeatedly: cycle, release to switch.
  No keyboard hook is used for this.
- Many windows stack like the dock's icons: app cards on the left overlap (the selected
  one stays whole), apps with several windows show cards peeking out behind them, and
  the row of an app's windows overlaps too.
- Mouse wheel / touchpad: over the stage it steps through the windows of the selected
  app, over the app cards it steps through the apps (one step per notch or swipe).

### Themes

- Frosted glass dock now uses a real system blur under the bar (Windows.UI.Composition,
  same rounded corners as the bar), with a darker, less milky tint. The blur follows the
  bar when the magnification wave grows it and hides with the dock. Windows 10 falls back
  to the acrylic accent.
- Clear glass is now a transparent pane with glass optics instead of a lighter tint: a lit
  rim, glowing edges, a diagonal sheen and a soft reflection that follows the cursor.
- New "Dock opacity" slider under the theme cards for the dark, light and system themes.

### Dock

- Separators are clearly visible in every theme: white on dark, black on light, an etched
  line on frosted glass and a black + white pair on clear glass.
- Sharper icons (extracted at 128 px), including magnified dock icons.
- Store apps that do not tag their windows (e.g. Claude) get their icon and are grouped
  with their pinned item.

### Fixes

- Switching from a glass theme to a solid one no longer flashes the settings window.
- The settings window shrinks to fit small or high-scaling screens (e.g. 1920x1080 at
  150%) instead of opening taller than the screen.

## 1.1.0 - 2026-09-27

### Dock

- macOS-like magnification: icons under the cursor zoom past the bar and their
  neighbours grow in a smooth wave that follows the cursor; the app name is shown next to
  the magnified icon. Zoom, range and labels are configurable.
- Crowded docks never scroll anymore. Icons shrink first, then (configurable):
  stack like cards (each icon slides under the next one), keep shrinking, or collapse the
  last apps into a "More" button with a grid popup.
- Only the visible bar receives the mouse; the rest of the dock window is click-through.

### Shortcuts

- New "Shortcuts" settings page with global shortcuts (show/hide the dock, open settings):
  record a combination, conflicts with Windows or other programs are reported, restore
  defaults. Shortcuts are paused while recording so the keys reach the page.

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
