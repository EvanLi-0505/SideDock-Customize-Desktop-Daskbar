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
    state/               persisted state (settings, dock items, launcher layout) with
                         atomic writes; configuration backups (`backup.rs`)
    modules/             OS watchers: app windows, system poller, autostart, icon cache,
                         system tray (receiver of the Explorer hook), taskbar visibility,
                         shell flyouts (quick settings / notification center toggles),
                         custom toolbar buttons (open / command / registry), windows asking
                         for attention (shell hook), file search of the launcher (in memory)
    widgets/             webview windows: dock (+ geometry, auto-hide, backdrop), popup,
                         tooltip, overlay (launcher / window switcher), toolbar, settings;
                         memory (releases the memory of idle helper windows and of
                         auto-hidden auxiliary docks)
    windows_api/         thin Win32/WinRT wrappers (AppBar, hooks, shell, icons, media, audio,
                         network status, CPU / memory / network speed...)
  ui/                    Svelte 5 frontend (Vite, multi-page)
    shared/              types mirrored from Rust, IPC, i18n, themes, components
    apps/<widget>/       one app per webview: dock, popup, tooltip, overlay, toolbar, settings
tray-hook/               sidedock_tray.dll: WH_CALLWNDPROC hook loaded into Explorer's taskbar
                         thread, forwards Shell_NotifyIcon messages to SideDock
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
* Auxiliary docks (`auxdock-<id>-<monitor>`, primary monitor only) are ordinary dock
  windows running the same page. `AuxDockSettings::effective` derives their
  `DockSettings` from the main dock's (own edge, size, mode, auto-hide, magnification,
  overflow; no titles, counters or split windows); `DockInfo` carries them, so the page
  reads its settings from there. Their pinned items live in `config/aux_dock_items.json`
  (`dock_items::get_for` / `replace_for`, keyed by the dock id; modules allowed, running
  apps never stored); the system poller also counts the modules of enabled auxiliary
  docks (`modules/system.rs`, `active_modules`); the
  item commands pick the store from the calling window's label, or from an explicit
  `store` argument (settings page, launcher). The page shows no running apps there and
  clicks focus the most recent window or launch.
* Dock layout (`widgets/dock/mod.rs`, `bars_on` / `span`): with no auxiliary dock every
  dock uses its whole edge. With one auxiliary dock and the main dock at the top or
  bottom, the main dock keeps its edge and a side dock is shortened. Otherwise left and
  right docks span the full height and top / bottom docks fit between them. Docks are
  laid out in `base_work_area` (the work area without SideDock's own reservations),
  inside a floating toolbar's strip (`toolbar::floating_strip`); the toolbar asks the
  docks to lay out again when its strip changes. Saving settings swaps an auxiliary
  dock's edge with the main dock's when the main dock moves onto it.
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
  by the page. DWM keeps a minimized window's last frame, so minimized windows get a
  thumbnail too, sized from their restored rect (`UserAppWindow.restored_rect`, from
  `GetWindowPlacement`). The switcher polls the media session every 2 s while open and
  puts the now-playing controls on the window of the matching app (HTML cannot be drawn
  over a DWM thumbnail, so they sit in the title row).
* Custom toolbar buttons (`modules/custom_buttons.rs`) are run by id: the page never
  sends what to run. Command buttons go through `ShellExecuteW` (`runas` when
  elevated): PowerShell gets the script as `-EncodedCommand` (UTF-16LE base64), a
  multi-line CMD script is written to `data/runtime/commands/<id>.cmd` (UTF-8, `chcp
  65001`). Registry buttons set Registry Editor's `LastKey` (its only way to open at a
  path) and start it; an open Registry Editor is brought to the front instead.
* Icons (`sdicon://`) already in memory are answered synchronously; the others are
  extracted by a small MTA worker pool (`utils::run_in_pool`).
* Popups (context menus, window list, calendar, keyboard, bluetooth, power) share one
  reusable window. The requesting widget sends JSON describing the popup; actions are sent
  back to it with `popup-action`.

* Attention (`modules/attention.rs`): a hidden top-level window registered with
  `RegisterShellHookWindow` receives `HSHELL_FLASH` when a window calls `FlashWindowEx`;
  `apps::set_attention` marks the window (`UserAppWindow.attention`) until it reaches the
  foreground, and the docks style the item.
* Folder stacks are a dock item (`DockItem::Stack { name, items }`, pinned apps only,
  sanitized like the rest). The page opens them as the `stack` popup kind; stack members
  count as pinned when running apps are matched, so they get no temporal item.
* Launcher search: apps are matched in the page; arithmetic, addresses and the web search
  are computed in the page (`shared/launcherSearch.ts`, a small parser, no `eval`); files
  come from `modules/file_search.rs`, an in-memory index of the user's folders built in
  the background (bounded depth, entries and time; tool folders skipped).
* Backups (`state/backup.rs`) bundle the config files into one JSON file in `backups`
  next to the exe or a folder the user picked. Restoring backs the current files up,
  writes the backup's (keeping this computer's backup folder and autostart) and restarts.

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
  The only writes outside `data/` are the autostart Run value, the "SideDock Win Key"
  scheduled task and Registry Editor's `LastKey` (registry buttons), all listed in the
  README.
* Only docks may reserve screen space (AppBar), and only when the user picks "never
  hide" for that dock. The toolbar never does: changing the work area makes Explorer rearrange the
  desktop icons. It floats, hides, or sits in the hidden taskbar's reserved place.
* Hiding the Windows taskbar only hides its windows (`SW_HIDE`); its auto-hide state and
  reserved work area are never touched, so the desktop layout cannot change. A marker in
  `data/runtime` lets the guardian restore the taskbar after a crash. The taskbar is only
  hidden while SideDock is visibly running: once a dock or toolbar page reported ready and
  while the window thread answers a heartbeat (`app.rs` posts one to it every second);
  after `UI_TIMEOUT` without one it is shown until the thread recovers. A start that does
  not hide it shows it whatever left it hidden (`taskbar_visibility::init`).
* Code running inside Explorer (`tray-hook`) never panics, never blocks for more than a
  short timeout and always passes the message on; the receiver replies before doing any
  slow work.
* Background threads are started with `utils::spawn_supervised` (restart on panic).
* Commands that launch things are throttled with `utils::throttle`.
* Persisted files are written with `state::storage::save_json` (atomic + backup).
* Updates never break saved data: a persisted field is never renamed or removed, new
  fields have serde defaults, and backup `formatVersion` 1 stays restorable (a new
  format needs a migration). `state::settings::compat_tests`, `state::backup::tests` and
  `state::dock_items::tests` load data of older versions.
* Version numbers are `main.feature.hotfix`.
