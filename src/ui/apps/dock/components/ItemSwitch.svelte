<script lang="ts">
  import type { DockItem } from "@shared/types.ts";
  import AppItem from "./items/AppItem.svelte";
  import Separator from "./items/Separator.svelte";
  import ModuleItem from "./items/ModuleItem.svelte";
  import StackItem from "./items/StackItem.svelte";

  let { item }: { item: DockItem } = $props();
</script>

{#if item.type === "App"}
  <AppItem {item} />
{:else if item.type === "Separator"}
  <Separator {item} />
{:else if item.type === "Module"}
  <ModuleItem {item} />
{:else if item.type === "Stack"}
  <StackItem {item} />
{/if}
