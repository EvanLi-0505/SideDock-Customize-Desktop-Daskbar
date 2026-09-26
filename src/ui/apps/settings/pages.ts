// Settings navigation. Adding a feature page = one entry here.

import type { Component } from "svelte";
import type { IconName } from "@shared/components/Icon.svelte";
import Home from "./pages/Home.svelte";
import General from "./pages/General.svelte";
import Dock from "./pages/Dock.svelte";
import Modules from "./pages/Modules.svelte";
import Data from "./pages/Data.svelte";
import About from "./pages/About.svelte";

export type PageId = "home" | "general" | "dock" | "modules" | "data" | "about";

export interface PageProps {
  navigate: (id: PageId) => void;
}

export interface PageEntry {
  id: PageId;
  label: string;
  icon: IconName;
  group: number;
  // pages may ignore the props they do not need
  component: Component<any>;
}

export const PAGES: PageEntry[] = [
  { id: "home", label: "nav.home", icon: "House", group: 0, component: Home },
  { id: "general", label: "nav.general", icon: "Settings", group: 0, component: General },
  { id: "dock", label: "nav.dock", icon: "PanelLeft", group: 1, component: Dock },
  { id: "modules", label: "nav.modules", icon: "LayoutDashboard", group: 1, component: Modules },
  { id: "data", label: "nav.data", icon: "Database", group: 2, component: Data },
  { id: "about", label: "nav.about", icon: "Info", group: 2, component: About },
];
