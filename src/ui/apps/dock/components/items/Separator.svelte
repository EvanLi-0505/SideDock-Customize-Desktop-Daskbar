<!-- Port of Seelen UI `weg/components/items/Separator.svelte` -->
<script lang="ts">
  import type { SeparatorDockItem } from "@shared/types.ts";
  import { isHardcodedSeparator } from "../../state/items.svelte.ts";
  import { showItemMenu } from "../../menus.svelte.ts";

  let { item }: { item: SeparatorDockItem } = $props();
  const hardcoded = $derived(isHardcodedSeparator(item));
</script>

<div
  role="separator"
  class="weg-separator"
  class:visible={!hardcoded}
  oncontextmenu={(e) => {
    if (!hardcoded) showItemMenu(e, item);
  }}
></div>
