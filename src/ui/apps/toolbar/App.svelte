<!--
  The toolbar: a full-width bar at the top or bottom of a monitor, or in the place of the
  hidden Windows taskbar. Layout after Seelen UI's fancy toolbar: three sections whose
  items are chosen in Settings > Toolbar, with every notification area icon available.
-->
<script lang="ts">
  import { t } from "@shared/i18n/index.svelte.ts";
  import { api } from "@shared/ipc.ts";
  import { settingsState } from "@shared/state/settings.svelte.ts";
  import { TOOLBAR_SECTIONS } from "@shared/toolbarItems.ts";
  import Item from "./Item.svelte";
  import { openMenu } from "./overlays.ts";
  import { toolbar } from "./state.svelte.ts";

  const settings = $derived(settingsState.value.toolbar);

  let height = $state(window.innerHeight);
  /** filling the hidden taskbar's space: the items keep the toolbar's own height */
  const filled = $derived(!!toolbar.info?.filled);
  const contentHeight = $derived(filled ? (toolbar.info?.contentHeight ?? height) : height);
  /** like the Windows clock: time and date on two lines when there is room */
  const twoLines = $derived(contentHeight >= 40 * (settings.scale / 100));

  function barMenu(e: MouseEvent) {
    openMenu(
      e,
      [
        { type: "item", id: "task-manager", label: t("module.task-manager"), icon: "SquareActivity" },
        { type: "separator" },
        { type: "item", id: "settings", label: t("toolbar.settings"), icon: "Settings" },
      ],
      (action) => {
        if (action === "task-manager") api.shellAction("task-manager");
        else if (action === "settings") api.openSettings();
      },
    );
  }
</script>

<svelte:window bind:innerHeight={height} />

<div
  class="toolbar"
  class:edge-top={toolbar.info?.edge === "top"}
  class:edge-bottom={toolbar.info?.edge !== "top"}
  class:filled
  style:--tb-content-h="{contentHeight}px"
  style:--tb-scale={settings.scale / 100}
  style:--covered-left="{toolbar.info?.coveredLeft ?? 0}px"
  style:--covered-right="{toolbar.info?.coveredRight ?? 0}px"
  role="toolbar"
  tabindex="-1"
  oncontextmenu={barMenu}
>
  <div class="toolbar-bg"></div>
  {#each TOOLBAR_SECTIONS as section (section)}
    <div class="section {section}">
      {#each settings.items[section] as id (id)}
        <Item {id} {twoLines} />
      {/each}
    </div>
  {/each}
</div>
