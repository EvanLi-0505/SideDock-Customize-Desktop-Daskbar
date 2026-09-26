<script lang="ts">
  import { t } from "@shared/i18n/index.svelte.ts";
  import { api } from "@shared/ipc.ts";
  import type { ModuleDockItem } from "@shared/types.ts";
  import { systemState } from "../../state/system.svelte.ts";
  import { showItemMenu } from "../../menus.ts";
  import { tooltip } from "../../overlays.ts";
  import { shouldSuppressClick } from "../../sortable.svelte.ts";

  let { item }: { item: ModuleDockItem } = $props();
  const count = $derived(systemState.system.recycleBin?.items ?? 0);
</script>

<div class="weg-item-overlay">
  <div
    role="button"
    tabindex="-1"
    class="weg-item weg-module"
    use:tooltip={() => `${t("module.recycle-bin")} · ${count ? t("recycle.items", { count }) : t("recycle.empty")}`}
    onclick={() => !shouldSuppressClick() && api.shellAction("recycle-bin")}
    oncontextmenu={(e) =>
      showItemMenu(
        e,
        item,
        [
          { type: "item", id: "open", label: t("item_menu.open_bin"), icon: "FolderOpen" },
          { type: "item", id: "empty", label: t("item_menu.empty_bin"), icon: "Trash2", disabled: count === 0 },
        ],
        (action) => (action === "empty" ? api.emptyRecycleBin() : api.shellAction("recycle-bin")),
      )}
    onkeydown={() => {}}
  >
    <!-- simple bin drawing that fills when the bin has items -->
    <svg class="weg-module-bin" viewBox="0 0 24 24" aria-hidden="true">
      <path class="bin-lid" d="M4 6.5h16M9.5 6.5V4.8c0-.7.5-1.3 1.2-1.3h2.6c.7 0 1.2.6 1.2 1.3v1.7" />
      <path class="bin-body" d="M5.8 6.5l1 12.7c.1 1 .9 1.8 1.9 1.8h6.6c1 0 1.8-.8 1.9-1.8l1-12.7" />
      {#if count > 0}
        <path class="bin-paper" d="M8.5 11.5l2.2-1.6 1.6 1.8 1.9-2.2 1.4 1.9-.6 6.4H9.1z" />
      {/if}
    </svg>
  </div>
</div>
