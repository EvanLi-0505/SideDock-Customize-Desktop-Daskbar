<script lang="ts" module>
  // per-icon imports keep builds fast (the barrel file compiles ~1900 icons)
  import Battery from "@lucide/svelte/icons/battery";
  import BatteryCharging from "@lucide/svelte/icons/battery-charging";
  import BatteryFull from "@lucide/svelte/icons/battery-full";
  import BatteryLow from "@lucide/svelte/icons/battery-low";
  import BatteryMedium from "@lucide/svelte/icons/battery-medium";
  import Bell from "@lucide/svelte/icons/bell";
  import Bluetooth from "@lucide/svelte/icons/bluetooth";
  import BluetoothOff from "@lucide/svelte/icons/bluetooth-off";
  import Check from "@lucide/svelte/icons/check";
  import ChevronLeft from "@lucide/svelte/icons/chevron-left";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import CircleAlert from "@lucide/svelte/icons/circle-alert";
  import Clock from "@lucide/svelte/icons/clock";
  import Database from "@lucide/svelte/icons/database";
  import Droplet from "@lucide/svelte/icons/droplet";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import FileCode2 from "@lucide/svelte/icons/file-code-2";
  import FilePlus2 from "@lucide/svelte/icons/file-plus-2";
  import FolderOpen from "@lucide/svelte/icons/folder-open";
  import HardDrive from "@lucide/svelte/icons/hard-drive";
  import House from "@lucide/svelte/icons/house";
  import Info from "@lucide/svelte/icons/info";
  import Keyboard from "@lucide/svelte/icons/keyboard";
  import LayoutDashboard from "@lucide/svelte/icons/layout-dashboard";
  import LayoutGrid from "@lucide/svelte/icons/layout-grid";
  import Layers from "@lucide/svelte/icons/layers";
  import Lock from "@lucide/svelte/icons/lock";
  import LockOpen from "@lucide/svelte/icons/lock-open";
  import LogOut from "@lucide/svelte/icons/log-out";
  import Minus from "@lucide/svelte/icons/minus";
  import Monitor from "@lucide/svelte/icons/monitor";
  import Moon from "@lucide/svelte/icons/moon";
  import Music from "@lucide/svelte/icons/music";
  import Pause from "@lucide/svelte/icons/pause";
  import PanelBottom from "@lucide/svelte/icons/panel-bottom";
  import PanelLeft from "@lucide/svelte/icons/panel-left";
  import PanelRight from "@lucide/svelte/icons/panel-right";
  import PanelTop from "@lucide/svelte/icons/panel-top";
  import Pin from "@lucide/svelte/icons/pin";
  import PinOff from "@lucide/svelte/icons/pin-off";
  import Play from "@lucide/svelte/icons/play";
  import Plus from "@lucide/svelte/icons/plus";
  import Power from "@lucide/svelte/icons/power";
  import PowerOff from "@lucide/svelte/icons/power-off";
  import Puzzle from "@lucide/svelte/icons/puzzle";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
  import Search from "@lucide/svelte/icons/search";
  import Settings from "@lucide/svelte/icons/settings";
  import ShieldCheck from "@lucide/svelte/icons/shield-check";
  import SkipBack from "@lucide/svelte/icons/skip-back";
  import SkipForward from "@lucide/svelte/icons/skip-forward";
  import Sparkles from "@lucide/svelte/icons/sparkles";
  import SquareActivity from "@lucide/svelte/icons/square-activity";
  import SeparatorHorizontal from "@lucide/svelte/icons/separator-horizontal";
  import SeparatorVertical from "@lucide/svelte/icons/separator-vertical";
  import Sun from "@lucide/svelte/icons/sun";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import Wifi from "@lucide/svelte/icons/wifi";
  import X from "@lucide/svelte/icons/x";
  import XCircle from "@lucide/svelte/icons/circle-x";

  /** Icons are referenced by name so menus can be sent as plain JSON to the popup. */
  export const ICONS = {
    Battery,
    BatteryCharging,
    BatteryFull,
    BatteryLow,
    BatteryMedium,
    Bell,
    Bluetooth,
    BluetoothOff,
    Check,
    ChevronLeft,
    ChevronRight,
    CircleAlert,
    Clock,
    Database,
    Droplet,
    ExternalLink,
    FileCode2,
    FilePlus2,
    FolderOpen,
    HardDrive,
    House,
    Info,
    Keyboard,
    LayoutDashboard,
    LayoutGrid,
    Layers,
    Lock,
    LockOpen,
    LogOut,
    Minus,
    Monitor,
    Moon,
    Music,
    Pause,
    PanelBottom,
    PanelLeft,
    PanelRight,
    PanelTop,
    Pin,
    PinOff,
    Play,
    Plus,
    Power,
    PowerOff,
    Puzzle,
    RefreshCw,
    RotateCcw,
    Search,
    Settings,
    ShieldCheck,
    SkipBack,
    SkipForward,
    Sparkles,
    SquareActivity,
    SeparatorHorizontal,
    SeparatorVertical,
    Sun,
    Trash2,
    Wifi,
    X,
    XCircle,
  } as const;

  export type IconName = keyof typeof ICONS;
</script>

<script lang="ts">
  interface Props {
    name: IconName | string;
    size?: number | string;
    strokeWidth?: number;
    class?: string;
  }

  let { name, size = 16, strokeWidth = 2, class: className = "" }: Props = $props();
  const Component = $derived(ICONS[name as IconName] ?? CircleAlert);
</script>

<Component {size} {strokeWidth} class="icon {className}" aria-hidden="true" />
