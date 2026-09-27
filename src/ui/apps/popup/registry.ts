// Popup kinds. Adding a popup = adding a component here; the backend never needs to change.
// A future window switcher / "mac-like" task view can live here as another kind.

import type { Component } from "svelte";
import type { Placement } from "@shared/types.ts";
import ContextMenu from "./kinds/ContextMenu.svelte";
import WindowList from "./kinds/WindowList.svelte";
import Calendar from "./kinds/Calendar.svelte";
import KeyboardSelector from "./kinds/KeyboardSelector.svelte";
import BluetoothPanel from "./kinds/BluetoothPanel.svelte";
import PowerPanel from "./kinds/PowerPanel.svelte";
import OverflowGrid from "./kinds/OverflowGrid.svelte";

export interface PopupProps {
  data: any;
  /** side of the anchor the popup opened on; it only grows away from the dock */
  placement: Placement;
  /** forwards an action to the widget that opened the popup */
  emit: (action: string, value?: unknown, close?: boolean) => void;
  close: () => void;
}

export const POPUPS: Record<string, Component<PopupProps>> = {
  menu: ContextMenu,
  windows: WindowList,
  calendar: Calendar,
  keyboard: KeyboardSelector,
  bluetooth: BluetoothPanel,
  power: PowerPanel,
  overflow: OverflowGrid,
};
