<script lang="ts">
  import type { ModuleDockItem } from "@shared/types.ts";
  import Clock from "../modules/Clock.svelte";
  import Keyboard from "../modules/Keyboard.svelte";
  import Media from "../modules/Media.svelte";
  import Power from "../modules/Power.svelte";
  import Bluetooth from "../modules/Bluetooth.svelte";
  import RecycleBin from "../modules/RecycleBin.svelte";
  import ShellModule from "../modules/ShellModule.svelte";

  let { item }: { item: ModuleDockItem } = $props();
</script>

{#if item.module === "clock"}
  <Clock {item} />
{:else if item.module === "keyboard"}
  <Keyboard {item} />
{:else if item.module === "media"}
  <Media {item} />
{:else if item.module === "power"}
  <Power {item} />
{:else if item.module === "bluetooth"}
  <Bluetooth {item} />
{:else if item.module === "recycle-bin"}
  <RecycleBin {item} />
{:else}
  <ShellModule {item} />
{/if}
