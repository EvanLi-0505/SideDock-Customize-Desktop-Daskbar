// Context menu description sent as JSON to the popup window.

export type MenuEntry =
  | {
      type: "item";
      id: string;
      label: string;
      icon?: string;
      /** image url used instead of an icon (e.g. the app icon) */
      image?: string;
      danger?: boolean;
      disabled?: boolean;
      /** checkbox items toggle in place and keep the menu open */
      checked?: boolean;
    }
  | { type: "separator" }
  | { type: "submenu"; id: string; label: string; icon?: string; items: MenuEntry[] };
