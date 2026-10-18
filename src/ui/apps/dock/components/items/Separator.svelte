<!-- Port of Seelen UI `weg/components/items/Separator.svelte` -->
<script lang="ts">
  import type { SeparatorDockItem } from "@shared/types.ts";
  import { t } from "@shared/i18n/index.svelte.ts";
  import { isHardcodedSeparator, isRunningAnchor } from "../../state/items.svelte.ts";
  import { showItemMenu } from "../../menus.svelte.ts";
  import { tooltip } from "../../overlays.ts";

  let { item }: { item: SeparatorDockItem } = $props();
  const hardcoded = $derived(isHardcodedSeparator(item));
  const anchor = $derived(isRunningAnchor(item));
</script>

<div
  role="separator"
  class="weg-separator"
  class:visible={!hardcoded}
  class:running-anchor={anchor}
  use:tooltip={() => (anchor ? t("dock.runningAnchor") : null)}
  oncontextmenu={(e) => {
    if (!hardcoded) showItemMenu(e, item);
  }}
></div>
