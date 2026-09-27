# Architecture

SideDock follows the layout of Seelen UI: a Rust/Tauri backend that owns every
interaction with Windows, and several small web apps ("widgets"), each running in its
own WebView2 window.

```
src/
  Cargo.toml, build.rs, tauri.conf.json, capabilities/
  background/            Rust backend
    main.rs              entry: portable paths, logger, guardian mode, panic hook
    app.rs               Tauri builder, lifecycle, settings side effects, event helpers
    commands.rs          every IPC command (all async, off the UI thread)
    session.rs           session lock, crash guardian, panic hook
    paths.rs, logger.rs  portable data folder, rolling file logs
    tray.rs              tray icon on the native taskbar
    state/               persisted state (settings, dock items) with atomic writes
    modules/             OS watchers: app windows, system poller, autostart, icon cache
    widgets/             webview windows: dock (+ geometry, auto-hide), popup, tooltip, settings
    windows_api/         thin Win32/WinRT wrappers (AppBar, hooks, shell, icons, media...)
  ui/                    Svelte 5 frontend (Vite, multi-page)
    shared/              types mirrored from Rust, IPC, i18n, themes, components
    apps/<widget>/       one app per webview: dock, popup, tooltip, settings
  static/icons/          app icons (logo.svg is the source)
scripts/package.mjs      portable zip packaging
```

## Data flow

* The backend is the single source of truth. Every change is broadcast as an event
  (`settings-changed`, `dock-items-changed`, `windows-changed`, `system-state-changed`,
  ...); each webview keeps a reactive copy.
* The dock never moves its own window: the backend's dock worker (`widgets/dock/mod.rs`)
  sizes/positions the window and registers the shell AppBar. All geometry changes are
  serialized through one worker thread.
* The dock window covers the whole edge and is deeper than the visible bar, to leave room
  for magnified icons. The page reports the bar rect (`dock_set_hitbox`); the backend
  hit-tests the cursor every frame (`widgets/dock/autohide.rs`) and keeps the window
  click-through everywhere else.
* Auto-hide runs in the same backend loop so it keeps working while a webview is
  throttled. A hidden dock slides its content out and becomes click-through.
* Overflow (`ui/apps/dock/fit.ts`) and magnification (`ui/apps/dock/wave.ts`) only write
  CSS variables and transforms, so the page never re-layouts per animation frame.
* Global shortcuts (`modules/hotkeys.rs`) use `RegisterHotKey` on a dedicated
  message-loop thread; no keyboard hook is installed.
* Popups (context menus, window list, calendar, keyboard, bluetooth, power) share one
  reusable window. The requesting widget sends JSON describing the popup; actions are sent
  back to it with `popup-action`.

## Adding features

* **A dock module**: add the id to `MODULES` in `state/dock_items.rs` and
  `ui/shared/modules.ts`, render it in `ui/apps/dock/components/items/ModuleItem.svelte`,
  add the strings to both locale files.
* **A popup**: create a component in `ui/apps/popup/kinds/` and register it in
  `ui/apps/popup/registry.ts`. No backend change is needed.
* **A new widget** (e.g. a window switcher): add `ui/apps/<name>/index.html` (picked up
  automatically by Vite), a module under `background/widgets/` using `WidgetWindow`, and
  its commands in `commands.rs`.
* **A global shortcut**: add the action to `SHORTCUT_DEFAULTS` (`state/settings.rs`) and
  `SHORTCUT_ACTIONS` (`ui/shared/shortcuts.ts`), handle it in `app::run_shortcut`, add
  its `shortcut.<id>` label to both locale files.
* **A settings page**: add a component in `ui/apps/settings/pages/` and one entry in
  `ui/apps/settings/pages.ts`.

## Reliability rules

* Never write to the user profile: every path comes from `paths::get()`; every webview is
  created through `WidgetWindow::builder`, which pins the WebView2 profile to `data/`.
* Background threads are started with `utils::spawn_supervised` (restart on panic).
* Commands that launch things are throttled with `utils::throttle`.
* Persisted files are written with `state::storage::save_json` (atomic + backup).
