<script lang="ts">
  import type { AppDockItem } from "@shared/types.ts";
  import { getWindowsForItem } from "../../state/items.svelte.ts";
  import { layout } from "../../state/layout.svelte.ts";
  import { systemState } from "../../state/system.svelte.ts";
  import AppButton from "./AppButton.svelte";

  let { item }: { item: AppDockItem } = $props();

  const windows = $derived(getWindowsForItem(item, systemState.windows));
  const split = $derived(layout.settings.splitWindows && windows.length > 1);
</script>

{#if split}
  <div class="weg-split-items">
    {#each windows as win (win.hwnd)}
      <AppButton {item} windows={[win]} />
    {/each}
  </div>
{:else}
  <AppButton {item} {windows} />
{/if}
