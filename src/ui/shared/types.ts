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
}

export interface AppSettings {
  language: Language;
  theme: ThemeMode;
  accentColor: string;
  autostart: boolean;
  crashRecovery: boolean;
  dock: DockSettings;
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

export type DockItem = AppDockItem | SeparatorDockItem | ModuleDockItem;

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
  isIconic: boolean;
  isZoomed: boolean;
  isFullscreen: boolean;
  lastForegroundAt: number;
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

export interface SystemState {
  power: PowerStatus | null;
  keyboard: KeyboardLayout[];
  recycleBin: RecycleBinInfo | null;
  media: MediaInfo | null;
  bluetooth: boolean | null;
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
