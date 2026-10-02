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
    widgets/             webview windows: dock (+ geometry, auto-hide, backdrop), popup,
                         tooltip, overlay (launcher / window switcher), settings
    windows_api/         thin Win32/WinRT wrappers (AppBar, hooks, shell, icons, media...)
  ui/                    Svelte 5 frontend (Vite, multi-page)
    shared/              types mirrored from Rust, IPC, i18n, themes, components
    apps/<widget>/       one app per webview: dock, popup, tooltip, overlay, settings
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
* The frosted glass theme puts a small native window right below each dock's bar
  (`widgets/dock/backdrop.rs`): a webview cannot blur what is behind its window. It draws
  the system's blurred host backdrop through Windows.UI.Composition, clipped to the bar's
  rounded rectangle. The page reports the bar shape (`dock_set_backdrop`), the backend
  loop keeps the window in place.
* Auto-hide runs in the same backend loop so it keeps working while a webview is
  throttled. A hidden dock slides its content out and becomes click-through.
* Overflow (`ui/apps/dock/fit.ts`) and magnification (`ui/apps/dock/wave.ts`) only write
  CSS variables and transforms, so the page never re-layouts per animation frame.
* Global shortcuts (`modules/hotkeys.rs`) use `RegisterHotKey` on a dedicated
  message-loop thread. The only keyboard hook is the optional Win key takeover: it runs
  in an elevated helper process (`SideDock.exe --win-key-helper`,
  `modules/win_key_helper.rs`) started through the on-demand scheduled task
  "SideDock Win Key", because a non-elevated process cannot keep the Windows 11 start
  menu closed. `modules/win_key.rs` registers / removes the task (one UAC prompt,
  `--win-key-register` / `--win-key-unregister`), starts the helper while the option is
  on and receives its messages on a message-only window. The helper exits with SideDock.
* The overlay (`widgets/overlay.rs`) is one full-screen window shared by the launcher and
  the window switcher, created hidden at startup and moved to the cursor's monitor on
  each opening. Launcher apps come from `shell:AppsFolder` (`modules/start_apps.rs`);
  their icons are extracted in the background after each scan so the launcher only ever
  reads the icon cache. Switcher previews are DWM thumbnails placed from rects computed
  by the page.
* Icons (`sdicon://`) already in memory are answered synchronously; the others are
  extracted by a small MTA worker pool (`utils::run_in_pool`).
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
