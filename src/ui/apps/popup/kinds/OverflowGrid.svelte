<!-- Apps that did not fit on the dock (Collapse overflow mode). -->
<script lang="ts">
  import type { PopupProps } from "../registry.ts";

  interface Entry {
    id: string;
    label: string;
    icon: string;
    running: boolean;
  }

  let { data, emit }: PopupProps = $props();
  const items = $derived((data?.items ?? []) as Entry[]);
</script>

<div class="popup-surface overflow">
  <div class="popup-header">{data?.title}</div>
  <div class="grid">
    {#each items as item (item.id)}
      <button type="button" class="app" title={item.label} onclick={() => emit("activate", item.id, true)}>
        <img src={item.icon} alt="" />
        <span class="name">{item.label}</span>
        {#if item.running}<span class="dot"></span>{/if}
      </button>
    {/each}
  </div>
</div>

<style>
  .overflow {
    padding: 8px;
    max-width: 380px;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(4, 84px);
    gap: 4px;
    /* fixed cap: vh would follow the popup window, which is sized from this content */
    max-height: 340px;
    overflow-y: auto;
  }

  .app {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 10px 4px 8px;
    border-radius: var(--radius-m);
    color: var(--fg);
    transition: background-color var(--dur-fast) ease;

    &:hover {
      background: var(--surface-hover);
    }

    img {
      width: 36px;
      height: 36px;
      object-fit: contain;
    }
  }

  .name {
    width: 100%;
    font-size: 11.5px;
    font-weight: 600;
    text-align: center;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dot {
    position: absolute;
    top: 8px;
    right: 14px;
    width: 6px;
    height: 6px;
    border-radius: 3px;
    background: var(--accent);
  }
</style>
