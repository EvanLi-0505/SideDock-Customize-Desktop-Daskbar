<script lang="ts">
  import { t } from "@shared/i18n/index.svelte.ts";
  import { api } from "@shared/ipc.ts";
  import type { ModuleDockItem } from "@shared/types.ts";
  import { systemState } from "../../state/system.svelte.ts";
  import { showItemMenu } from "../../menus.ts";
  import { openPopup, tooltip } from "../../overlays.ts";
  import { shouldSuppressClick } from "../../sortable.svelte.ts";

  let { item }: { item: ModuleDockItem } = $props();

  const active = $derived(systemState.system.keyboard.find((k) => k.active) ?? systemState.system.keyboard[0]);
  let el: HTMLDivElement | null = $state(null);
</script>

<div class="weg-item-overlay" bind:this={el}>
  <div
    role="button"
    tabindex="-1"
    class="weg-item weg-module weg-module-text"
    use:tooltip={() => (active ? `${active.displayName}${active.layoutName ? ` · ${active.layoutName}` : ""}` : t("module.keyboard"))}
    onclick={() => !shouldSuppressClick() && openPopup(el!, "keyboard", $state.snapshot(systemState.system.keyboard), { key: "keyboard" })}
    oncontextmenu={(e) =>
      showItemMenu(e, item, [{ type: "item", id: "settings", label: t("keyboard.more"), icon: "Settings" }], () =>
        api.shellAction("keyboard-settings"),
      )}
    onkeydown={() => {}}
  >
    <span class="keyboard-label">{active?.shortLabel ?? "EN"}</span>
  </div>
</div>
