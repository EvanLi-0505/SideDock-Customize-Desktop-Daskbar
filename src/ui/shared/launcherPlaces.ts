// Shortcuts of the launcher's bottom bar (like the folders next to the power button of the
// Windows start menu). Keep in sync with `LAUNCHER_PLACES` in src/background/state/settings.rs.
import type { IconName } from "./components/Icon.svelte";
import type { TranslationKey } from "./i18n/index.svelte.ts";
import type { LauncherPlaceId } from "./types.ts";

export interface LauncherPlace {
  id: LauncherPlaceId;
  icon: IconName;
  label: TranslationKey;
}

export const LAUNCHER_PLACES: LauncherPlace[] = [
  { id: "settings", icon: "Settings", label: "place.settings" },
  { id: "explorer", icon: "FolderOpen", label: "place.explorer" },
  { id: "documents", icon: "FileText", label: "place.documents" },
  { id: "downloads", icon: "Download", label: "place.downloads" },
  { id: "music", icon: "Music", label: "place.music" },
  { id: "pictures", icon: "Image", label: "place.pictures" },
  { id: "videos", icon: "Video", label: "place.videos" },
  { id: "network", icon: "Network", label: "place.network" },
  { id: "user-folder", icon: "CircleUserRound", label: "place.user-folder" },
];

export function launcherPlace(id: LauncherPlaceId): LauncherPlace {
  return LAUNCHER_PLACES.find((p) => p.id === id)!;
}
