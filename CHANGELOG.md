# Changelog

## 2.3.0 - 2026-10-17

### Extra docks

- Up to 3 extra docks besides the main one (Settings > "Extra docks"), each on an edge of
  its own, on the primary monitor. They hold the apps and files you pin and the modules
  you add, never running apps. A click brings the app's most recent window to the front,
  or starts it; the context menu removes the item.
- Modules: the same ones as on the main dock (start menu, clock, media, power, network...),
  from the dock's right-click "Modules" menu or as switches on the settings page. Each
  dock has its own; their system data is read while any enabled dock shows them.
- Each has its own size, width mode, auto-hide mode and delays. "Never hide" reserves
  screen space, with the same warning as the main dock; the default is "when a window
  overlaps it".
- Each also has its own magnification (on / off, largest scale, icons affected on each
  side, app names) and its own "when items do not fit" behavior (stack, shrink, collapse).
  A dock without magnification uses a narrower window.
- Pin from the settings page, by dropping files on the dock, or with "Pin to extra dock"
  in the launcher's context menu. Items are kept in `data/config/aux_dock_items.json`.
- Layout: left and right docks span the full height, top and bottom docks fit between
  them. With a single extra dock and the main dock at the top or bottom, the main dock
  keeps its whole edge. Moving the main dock onto an extra dock's edge swaps the two;
  picking another extra dock's edge swaps those two.
- A floating toolbar now keeps its whole edge: docks on that edge are placed inside it
  instead of under it.
- An auto-hidden extra dock gives its memory back while hidden.

### Window switcher

- Minimized windows now show a preview: their last frame before they were minimized,
  tagged "minimized", in the proportions they are restored to (there used to be an icon
  placeholder). The picture does not move: Windows stops drawing minimized windows.
- Now playing: the app that plays media shows the track and previous / play-pause / next
  buttons next to its window title, so a minimized video can be paused or skipped
  without switching to it.

### Toolbar: command and registry buttons

- Custom buttons have a kind: "Open" (as before), "Command" and "Registry".
- Command: a CMD or PowerShell command, several lines allowed. Run it in a window that
  stays open, closes when done, or hidden; optionally as administrator and after a
  confirmation. Multi-line CMD commands run from a batch file in
  `data/runtime/commands/`.
- Registry: a registry path (`HKCU\...`, `HKEY_CURRENT_USER\...`, `Computer\...` or with
  `/`) that Registry Editor opens at. This writes Registry Editor's own "last key" value
  (`HKCU\Software\Microsoft\Windows\CurrentVersion\Applets\Regedit\LastKey`), the only
  way to open it at a path; see the README. If Registry Editor is already open it is
  brought to the front, with a note to close it first.
- Errors of custom buttons are shown in a small notice instead of being ignored.

### Fixed

- With several docks or toolbars (several monitors, or extra docks), each window also
  received the messages meant for the others: placement, auto-hide state, pointer leave
  and popup menu actions. A dock could take another dock's edge or width mode, and a
  menu action could reach the wrong dock. Each window now only listens to its own.
- The frosted glass backdrop (and the area that takes the mouse) could stay behind when
  a dock changed edges: a bar that fits its content keeps its size but moves inside the
  resized window, and only size changes were measured. Window resizes are measured too.
- Keyboard layout selector (dock module and toolbar): picking a language often did
  nothing. The request went to the application while SideDock's popup had the focus, and
  Windows puts the input language back when a window returns to the front. The
  application now gets the focus back first, and the request goes to the window that
  takes its keyboard input (the core window of Store apps included).
- The keyboard item followed Win+Space and other switches up to a second or more late,
  and not at all in some apps: it read the layout of the top-level window's thread once
  a second. It now reads the thread that has the keyboard focus, every 150 ms (a few
  cheap calls; the list is only rebuilt when something changes).
- The keyboard item now also follows the Chinese / English mode of Chinese IMEs (Shift in
  Microsoft Pinyin, Sogou...): it shows "英" in English mode, like the Windows taskbar, and
  "A" for Japanese and Korean IMEs typing Latin letters. Asked from the IME's own window,
  with a 50 ms limit so a hung application cannot hold SideDock up.

## 2.2.0 - 2026-10-15

### Toolbar: CPU, memory and network speed

- Three new toolbar items: CPU usage, memory usage (the tooltip shows used / total) and
  network speed (download and upload, stacked when the toolbar is tall enough). A click
  opens Task Manager. Fixed-width digits keep the toolbar from shifting as they change.
- Read with plain system calls once a second, and only while one of these items is on
  the toolbar. Network speed counts the hardware adapters only (virtual and filter
  interfaces would count the same traffic twice).
- Add them in Settings > "Toolbar & taskbar" > Items (they start under "Not shown").

### Toolbar: custom buttons

- Settings > "Toolbar & taskbar" > "Custom buttons": add buttons that open a program,
  file, folder or web address, with a name, optional arguments and an icon (the target's
  own icon by default, or one of 24 icons). Up to 24 buttons.
- A new button is placed right after the start button; drag it elsewhere like any item.
- The page only asks for a button by its id: what it opens is always read from the saved
  settings.

### Dock

- The "never hide" auto-hide option now says that it reserves screen space, and that
  turning it on or off or adjusting the dock may make Windows rearrange the desktop icons.

## 2.1.0 - 2026-10-12

### Launcher bottom bar

- Like the Windows start menu: the signed-in user on the left (account settings, lock,
  sign out), shortcuts next to the power button (Settings, File Explorer, Documents,
  Downloads, Music, Pictures, Videos, Network, personal folder) and the power button
  (sleep, shut down, restart). Shut down, restart and sign out need a second click.
- Settings > "Start & switcher" > "Launcher bottom bar" chooses the shortcuts. Folders
  open where they really are (redirected to another drive or OneDrive included).

### Live network and volume icons

- The toolbar's network & volume item (and the dock's network module) now show the real
  state: Wi-Fi with its signal strength, wired, no connection, connected without
  internet; volume level and mute. The tooltip names the network and the volume.
- Wheel over the item: volume up / down by 2% (like over the taskbar's volume icon);
  middle click: mute.
- Fixed: the toolbar's battery, keyboard, Bluetooth and media items were only refreshed
  while the dock showed the same module.

### Window switcher

- A click on an app card (left) or on one of the small windows below the stage now only
  selects it: it moves to the stage. A click on the stage, or a double click on a card or
  a small window, switches to it. (A single click used to switch at once.)

### Toolbar

- The tray overflow flyout ("^") is as wide as its icons, 5 per row at most; more icons
  add rows.

### Memory

- The popup, tooltip and launcher windows are kept hidden so they open instantly; after
  30 seconds without use WebView2 may now release their memory (measured: about 120 MB
  less working set). They get it back before they render again; the launcher still
  opens in a few tens of milliseconds.

## 2.0.0 - 2026-10-11

### Fancy toolbar

- New toolbar inspired by Seelen UI's fancy toolbar: start button, name of the app in the
  foreground, media controls, keyboard layout, Bluetooth, network & volume (opens the
  Windows quick settings), battery, clock and calendar (optional seconds), notifications
  and a "show desktop" sliver.
- **System tray icons inline**, side by side. Click, double click, right click and middle
  click reach the application exactly as on the taskbar. The icons are captured with a
  small hook DLL (`sidedock_tray.dll`) loaded into Explorer's taskbar thread, the same
  technique as Seelen UI; it only forwards a copy of the `Shell_NotifyIcon` messages and
  is unloaded when the tray is turned off.
- Tray icons keep the size the application made them (24 px on a 150% display) and are
  shown pixel for pixel like on the taskbar, instead of being stretched and scaled back.
- Explorer's own status icons (volume, network, power...) are left out: on Windows 11
  they are white on transparent and ignore clicks; the toolbar's quick settings, battery
  and notification items replace them. "Safely remove hardware" stays.
- The network & volume and the notification buttons toggle the Windows flyouts (they send
  Win+A / Win+N): a second click closes the flyout instead of reopening it.
- A toolbar floating above the visible taskbar sits where those flyouts open; they are
  above every ordinary window, so the toolbar's items move aside while one covers its
  end and stay visible and clickable.
- You choose which icons are on the toolbar and which go to the overflow flyout ("^"):
  drag them like on the Windows taskbar (into the flyout, back onto the toolbar, or along
  the toolbar to reorder), switch them one by one in the settings, show them all, or copy
  the Windows taskbar's own choice with one click.
- **The toolbar never changes the desktop layout.** It never reserves screen space (any
  change of the work area makes Explorer rearrange the desktop icons). At the bottom with
  the Windows taskbar hidden it takes the taskbar's place; anywhere else it floats over
  windows and can hide when a window overlaps it, or always (it comes back at the screen
  edge). Show / hide delays are configurable. Hidden while a fullscreen app is in the
  foreground.
- Customizable like Seelen UI's: items are dragged between the left, center and right
  sections or hidden; height, content size (80-140%), primary or every monitor.
- Its own theme: same as the dock, or any of the five themes with its own opacity. Popups
  opened from the toolbar follow it. The Home page says which theme is the dock's.

### Launcher: choose what it shows, collections

- Settings > "Start & switcher": the launcher shows every app, or only the apps you added.
  Any app can be removed from the launcher; search and the new "All apps" view still find
  it and can add it back.
- Collections (folders like in the Windows 11 start menu): right-click an app > "New
  collection" and name it, or drop an app onto another one. Drop apps on a collection to
  add them; open it to start, reorder or drag apps out, click its title to rename,
  right-click to ungroup.
- Tiles are dragged to reorder them; holding one at the left / right screen edge turns
  the page. The arrangement is kept in `data/config/launcher_layout.json`.

### Hide the Windows taskbar without changing the desktop layout

- Optional. Only the taskbar windows are hidden; their auto-hide state and the screen
  area they reserve are left alone, so desktop icons and maximized windows never move,
  on 16:9, 16:10 or any other screen. (Seelen UI switches the taskbar to auto-hide, which
  resizes the work area and rearranges the desktop.)
- Turning the option off always brings the taskbar back, however fast it is switched: the
  restore no longer trusts the taskbar's current visibility (a pending "hide" could land
  after it) and is checked again for a few seconds.
- The taskbar is found with `FindWindowEx`: while quick settings is open Explorer moves it
  to a window band `EnumWindows` does not list, where it could be neither hidden nor
  restored.
- Hidden again right away if Explorer shows it (or restarts). Shown again when SideDock
  exits, when the option is turned off and, through the guardian, after a crash.
- The start button choice (Windows start menu or SideDock launcher) sits next to it, on
  the Home page and on the new "Toolbar & taskbar" settings page.

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
