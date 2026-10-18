// Mirrors of the Rust types (src/background/state, modules, widgets).
// Keep both sides in sync when changing a field.

export type Language = "zh-CN" | "en";
/** glass = frosted glass (玻璃·毛), clear = clear glass (玻璃·清) */
export type ThemeMode = "glass" | "clear" | "dark" | "light" | "system";

export type DockMode = "FullWidth" | "MinContent";
export type DockSide = "Left" | "Right" | "Top" | "Bottom";
export type HideMode = "Never" | "Always" | "OnOverlap";
export type TemporalItemsVisibility = "All" | "OnMonitor";
export type PinnedItemsVisibility = "Always" | "WhenPrimary";
export type MiddleClickAction = "CloseApp" | "OpenNewInstance";
export type DockMonitors = "Primary" | "All";
export type OverflowMode = "ShrinkThenStack" | "ShrinkOnly" | "Collapse";

export interface DockSettings {
  enabled: boolean;
  monitors: DockMonitors;
  mode: DockMode;
  position: DockSide;
  hideMode: HideMode;
  delayToShow: number;
  delayToHide: number;
  temporalItemsVisibility: TemporalItemsVisibility;
  pinnedItemsVisibility: PinnedItemsVisibility;
  size: number;
  margin: number;
  padding: number;
  spaceBetweenItems: number;
  showWindowTitle: boolean;
  showInstanceCounter: boolean;
  splitWindows: boolean;
  showEndTask: boolean;
  middleClickAction: MiddleClickAction;
  hideOnFullscreen: boolean;
  magnification: boolean;
  magnificationScale: number;
  magnificationRange: number;
  showLabels: boolean;
  overflowMode: OverflowMode;
  /** bar background opacity (percent) for the solid themes */
  backgroundOpacity: number;
}

export type StartMenuMode = "Native" | "SideDock";

/** Win key takeover (elevated helper started through a scheduled task). */
export type WinKeyStatus = "off" | "needsAuthorization" | "starting" | "active" | "failed";

export interface WinKeyState {
  status: WinKeyStatus;
  /** the "SideDock Win Key" scheduled task exists */
  taskRegistered: boolean;
}

/** All = the user's layout followed by every other app; Pinned = only the layout */
export type LauncherContent = "All" | "Pinned";

/** Keep in sync with LAUNCHER_PLACES in src/background/state/settings.rs */
export type LauncherPlaceId =
  | "settings"
  | "explorer"
  | "documents"
  | "downloads"
  | "music"
  | "pictures"
  | "videos"
  | "network"
  | "user-folder";

export interface LauncherSettings {
  startMenu: StartMenuMode;
  takeOverWinKey: boolean;
  iconSize: number;
  content: LauncherContent;
  /** shortcuts of the launcher's bottom bar, in order */
  places: LauncherPlaceId[];
  /** the search also lists files of the user's folders */
  searchFiles: boolean;
  /** web search offered for what is typed */
  searchEngine: SearchEngine;
}

export type SearchEngine = "Bing" | "Google" | "Baidu" | "DuckDuckGo" | "None";

/** A file or folder found by the launcher search. */
export interface FileHit {
  name: string;
  path: string;
  /** the folder it is in */
  folder: string;
  isDir: boolean;
  /** a recent item (a shortcut): no folder of its own to reveal */
  recent: boolean;
}

export type ToolbarPosition = "Top" | "Bottom";

/** Keep in sync with TOOLBAR_ITEMS in src/background/state/settings.rs */
export type ToolbarItemId =
  | "start"
  | "app-name"
  | "media"
  | "tray"
  | "keyboard"
  | "bluetooth"
  | "quick-settings"
  | "battery"
  | "notifications"
  | "clock"
  | "task-manager"
  | "show-desktop"
  | "cpu"
  | "memory"
  | "net-speed"
  /** a custom button: `custom:<button id>` */
  | `custom:${string}`;

export type ButtonKind = "open" | "command" | "registry";
export type CommandShell = "cmd" | "powershell";
/** keep: window stays open, close: closes when done, hidden: no window */
export type RunMode = "keep" | "close" | "hidden";

/** A button the user adds to the toolbar. */
export interface CustomButton {
  id: string;
  name: string;
  kind: ButtonKind;
  /** program, file, folder or URL (open); key path (registry) */
  target: string;
  /** command line arguments, programs only */
  args: string;
  /** one or several lines (command) */
  command: string;
  shell: CommandShell;
  runMode: RunMode;
  /** run the command as administrator */
  elevated: boolean;
  /** ask before running */
  confirm: boolean;
  /** icon name of the icon set, "" = the target's own icon / the kind's icon */
  icon: string;
}

export interface ToolbarItems {
  left: ToolbarItemId[];
  center: ToolbarItemId[];
  right: ToolbarItemId[];
}

/** Which tray icons are on the toolbar and which are in the overflow flyout (by key). */
export interface TrayPrefs {
  inline: string[];
  overflow: string[];
  newIconsInline: boolean;
}

export interface ToolbarSettings {
  enabled: boolean;
  monitors: DockMonitors;
  position: ToolbarPosition;
  /** the toolbar never reserves screen space: it floats, hides, or takes the taskbar's place */
  hideMode: HideMode;
  delayToShow: number;
  delayToHide: number;
  /** CSS px */
  height: number;
  /** in the hidden taskbar's place: exactly the taskbar's height */
  matchTaskbarHeight: boolean;
  /** lower than the hidden taskbar: fill the rest of its space */
  fillGap: FillGap;
  /** content size, percent */
  scale: number;
  /** null = same theme as the dock */
  theme: ThemeMode | null;
  backgroundOpacity: number;
  items: ToolbarItems;
  tray: TrayPrefs;
  showSeconds: boolean;
  customButtons: CustomButton[];
}

export interface TaskbarSettings {
  /** hide the native Windows taskbar (its reserved space is kept) */
  hideNative: boolean;
}

export interface AppSettings {
  language: Language;
  theme: ThemeMode;
  accentColor: string;
  autostart: boolean;
  crashRecovery: boolean;
  dock: DockSettings;
  launcher: LauncherSettings;
  toolbar: ToolbarSettings;
  taskbar: TaskbarSettings;
  /** the three auxiliary docks, enabled or not */
  auxDocks: AuxDockSettings[];
  /** folder for configuration backups; empty = `backups` next to SideDock.exe */
  backupDir: string;
  /** action id -> accelerator ("Ctrl+Alt+Backquote"), "" = disabled */
  shortcuts: Record<string, string>;
}

export type ShortcutStatus = "active" | "disabled" | "invalid" | "conflict" | "suspended";

export interface Relaunch {
  command: string;
  args: string | null;
  workingDir: string | null;
}

export interface AppDockItem {
  type: "App";
  id: string;
  displayName: string;
  path: string;
  umid: string | null;
  pinned: boolean;
  preventPinning: boolean;
  relaunch: Relaunch | null;
}

export interface SeparatorDockItem {
  type: "Separator";
  id: string;
}

export interface ModuleDockItem {
  type: "Module";
  id: string;
  module: ModuleId;
}

/** A folder stack: several pinned apps behind one icon, opened as a grid. */
export interface StackDockItem {
  type: "Stack";
  id: string;
  name: string;
  items: AppDockItem[];
}

export type DockItem = AppDockItem | SeparatorDockItem | ModuleDockItem | StackDockItem;

export interface DockItems {
  isReorderDisabled: boolean;
  left: DockItem[];
  center: DockItem[];
  right: DockItem[];
}

export type ModuleId =
  | "start-menu"
  | "show-desktop"
  | "recycle-bin"
  | "clock"
  | "keyboard"
  | "media"
  | "power"
  | "bluetooth"
  | "network"
  | "notifications"
  | "task-manager";

export interface Rect {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

export interface UserAppWindow {
  hwnd: number;
  title: string;
  appName: string;
  umid: string | null;
  path: string | null;
  monitor: number;
  rect: Rect | null;
  /** size it is restored to, while minimized */
  restoredRect: Rect | null;
  isIconic: boolean;
  isZoomed: boolean;
  isFullscreen: boolean;
  lastForegroundAt: number;
  /** asks for attention (flashing taskbar button) until brought to the front */
  attention: boolean;
}

export interface FocusedApp {
  hwnd: number;
  title: string;
  path: string | null;
  isOwn: boolean;
}

export interface MonitorInfo {
  id: number;
  rect: Rect;
  workArea: Rect;
  scaleFactor: number;
  isPrimary: boolean;
}

export interface DockInfo {
  label: string;
  monitor: MonitorInfo;
  rect: Rect;
  hidden: boolean;
  /** id of the auxiliary dock, null for the main dock */
  aux: string | null;
  /** the settings this dock runs with (an auxiliary dock's are derived from the main one's) */
  settings: DockSettings;
}

/** An auxiliary dock: pinned items only, on an edge of its own, on the primary monitor. */
export interface AuxDockSettings {
  id: string;
  enabled: boolean;
  position: DockSide;
  mode: DockMode;
  size: number;
  hideMode: HideMode;
  delayToShow: number;
  delayToHide: number;
  magnification: boolean;
  magnificationScale: number;
  magnificationRange: number;
  showLabels: boolean;
  overflowMode: OverflowMode;
}

export interface KeyboardLayout {
  id: string;
  locale: string;
  displayName: string;
  layoutName: string;
  shortLabel: string;
  active: boolean;
}

export interface PowerStatus {
  hasBattery: boolean;
  percentage: number;
  charging: boolean;
  pluggedIn: boolean;
  saver: boolean;
}

export interface RecycleBinInfo {
  items: number;
  size: number;
}

export interface MediaInfo {
  title: string;
  artist: string;
  playing: boolean;
  sourceApp: string;
  thumbnail: string | null;
}

export interface NetworkStatus {
  kind: "none" | "wifi" | "ethernet" | "cellular";
  /** reaches the internet, not only the local network */
  internet: boolean;
  /** 0-5 bars, wireless only */
  signal: number | null;
  name: string;
}

export interface VolumeStatus {
  /** 0-100 */
  level: number;
  muted: boolean;
}

export interface PerfStatus {
  /** 0-100, null until two samples were taken */
  cpu: number | null;
  memory: { percent: number; used: number; total: number } | null;
  /** bytes per second, null until two samples were taken */
  net: { down: number; up: number } | null;
}

export interface SystemState {
  power: PowerStatus | null;
  keyboard: KeyboardLayout[];
  recycleBin: RecycleBinInfo | null;
  media: MediaInfo | null;
  bluetooth: boolean | null;
  network: NetworkStatus | null;
  /** null = no playback device */
  volume: VolumeStatus | null;
  /** CPU, memory, network speed (toolbar items) */
  perf: PerfStatus | null;
}

export type Placement = "right" | "left" | "above" | "below";

export interface AnchorRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface AppInfo {
  version: string;
  exe: string;
  dataDir: string;
  previousSessionCrashed: boolean;
  debug: boolean;
}

export interface StorageUsage {
  logs: number;
  userdata: number;
  cache: number;
  config: number;
}

export type DataKind = "logs" | "userdata" | "cache" | "config" | "root";

/** A configuration backup file (`SideDock-backup-<time>.json`). */
export interface BackupEntry {
  path: string;
  name: string;
  /** local time it was made */
  created: string;
  appVersion: string;
  size: number;
  /** made automatically right before a restore */
  automatic: boolean;
}

export interface BackupInfo {
  /** where backups go */
  dir: string;
  /** `backups` next to SideDock.exe */
  defaultDir: string;
  custom: boolean;
  /** the folder exists (created by the first backup, may be deleted by hand) */
  dirExists: boolean;
  /** newest first */
  entries: BackupEntry[];
}

export interface ClearResult {
  skipped: number;
  restartRequired: boolean;
}

export interface SystemColors {
  accent: string;
  darkMode: boolean;
}

export interface Bootstrap {
  label: string;
  widget: string;
  monitorId: number | null;
}

declare global {
  interface Window {
    __SIDEDOCK__?: Bootstrap;
  }
}

// ---------------- overlay (launcher / switcher) ----------------

/** An app of the Start menu "All apps" list (`shell:AppsFolder`). */
export interface StartApp {
  id: string;
  name: string;
  path: string | null;
  packaged: boolean;
}

export type OverlayMode = "launcher" | "switcher";

/** A live window preview drawn by DWM over the overlay page, in CSS px. */
export interface ThumbnailRequest {
  key: string;
  hwnd: number;
  x: number;
  y: number;
  width: number;
  height: number;
  /** visible part from the top-left (0..1), for previews partly covered by a card */
  visibleWidth?: number;
  visibleHeight?: number;
}

// ---------------- toolbar & system tray ----------------

export interface ToolbarInfo {
  label: string;
  monitor: MonitorInfo;
  /** popups open on the other side */
  edge: "top" | "bottom";
  /** sits in the hidden Windows taskbar's place */
  replacesTaskbar: boolean;
  /** CSS px at its ends covered by a Windows flyout (quick settings, notifications) */
  coveredLeft: number;
  coveredRight: number;
  /** the background fills the hidden taskbar's whole space; the items keep `contentHeight` */
  filled: boolean;
  contentHeight: number;
}

/** Whether a toolbar lower than the hidden taskbar fills the rest of its space. */
export type FillGap = "Off" | "WhenNeeded" | "Always";

/** A notification area icon of another application. */
export interface TrayIcon {
  id: string;
  /** identity kept in the settings */
  key: string;
  tooltip: string;
  /** changes whenever the image changes */
  iconVersion: string;
  /** on the toolbar (else in the overflow flyout) */
  inline: boolean;
}

/** Cursor position of a tray icon drag, in the receiving window's CSS px. */
export interface TrayDrag {
  phase: "move" | "end" | "cancel";
  key: string;
  x: number;
  y: number;
  inside: boolean;
}

// ---------------- launcher layout ----------------

export type LauncherEntry =
  | { type: "App"; id: string }
  | { type: "Folder"; id: string; name: string; apps: string[] };

export interface LauncherLayout {
  items: LauncherEntry[];
  /** removed from the launcher (still found by search) */
  removed: string[];
}

export type TrayAction = "left" | "double" | "right" | "middle";
