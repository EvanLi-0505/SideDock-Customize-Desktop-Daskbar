// Icons and tooltips of the live network / volume status (toolbar item, dock module).

import type { IconName } from "./components/Icon.svelte";
import { t } from "./i18n/index.svelte.ts";
import type { NetworkStatus, VolumeStatus } from "./types.ts";

export function networkIcon(status: NetworkStatus | null): IconName {
  // unknown yet: the plain Wi-Fi icon, as before the first reading
  if (!status) return "Wifi";
  switch (status.kind) {
    case "none":
      return "WifiOff";
    case "ethernet":
      return "EthernetPort";
    default: {
      const bars = status.signal ?? 5;
      if (bars <= 1) return "WifiZero";
      if (bars === 2) return "WifiLow";
      if (bars === 3) return "WifiHigh";
      return "Wifi";
    }
  }
}

export function networkTip(status: NetworkStatus | null): string {
  if (!status) return t("module.network");
  if (status.kind === "none") return t("network.disconnected");
  const name = status.name || t(`network.${status.kind}`);
  return status.internet ? name : `${name} · ${t("network.noInternet")}`;
}

export function volumeIcon(status: VolumeStatus | null): IconName {
  if (!status) return "Volume2";
  if (status.muted) return "VolumeX";
  if (status.level === 0) return "Volume";
  return status.level < 34 ? "Volume1" : "Volume2";
}

export function volumeTip(status: VolumeStatus | null): string {
  if (!status) return t("volume.noDevice");
  return status.muted ? t("volume.muted") : t("volume.level", { level: status.level });
}
