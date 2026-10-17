<!-- A dock folder stack, opened: its apps in a grid, its name editable, apps can be moved
     back out onto the dock. -->
<script lang="ts">
  import Icon from "@shared/components/Icon.svelte";
  import { t } from "@shared/i18n/index.svelte.ts";
  import type { PopupProps } from "../registry.ts";

  interface Entry {
    id: string;
    label: string;
    icon: string;
    running: boolean;
    attention: boolean;
  }

  let { data, emit }: PopupProps = $props();
  const items = $derived((data?.items ?? []) as Entry[]);
  // svelte-ignore state_referenced_locally
  let name = $state<string>(data?.name ?? "");
  // svelte-ignore state_referenced_locally
  let committed = data?.name ?? "";

  function rename() {
    const next = name.trim();
    if (!next || next === committed) {
      name = committed;
      return;
    }
    committed = next;
    emit("rename", next);
  }
</script>

<div class="popup-surface stack">
  <input
    class="stack-name"
    type="text"
    spellcheck="false"
    maxlength="40"
    title={t("stack.rename")}
    bind:value={name}
    onblur={rename}
    onkeydown={(e) => {
      if (e.key === "Enter") (e.currentTarget as HTMLInputElement).blur();
      if (e.key === "Escape") {
        name = committed;
        (e.currentTarget as HTMLInputElement).blur();
      }
    }}
  />
  <div class="grid" style:grid-template-columns="repeat({Math.min(4, Math.max(2, items.length))}, 84px)">
    {#each items as item (item.id)}
      <div class="cell">
        <button
          type="button"
          class="app"
          class:attention={item.attention}
          title={item.label}
          onclick={() => emit("open", item.id, true)}
        >
          <img src={item.icon} alt="" />
          <span class="name">{item.label}</span>
          {#if item.running}<span class="dot"></span>{/if}
        </button>
        <button type="button" class="out" title={t("stack.moveOut")} onclick={() => emit("move-out", item.id, true)}>
          <Icon name="LogOut" size={12} />
        </button>
      </div>
    {/each}
  </div>
</div>

<style>
  .stack {
    padding: 8px;
    max-width: 380px;
  }

  .stack-name {
    width: 100%;
    margin-bottom: 6px;
    padding: 6px 8px;
    border: 1px solid transparent;
    border-radius: var(--radius-s);
    background: transparent;
    color: var(--fg);
    font-size: 13px;
    font-weight: 700;
    text-align: center;
    outline: none;

    &:hover {
      border-color: var(--border);
    }
    &:focus {
      border-color: var(--accent);
      background: var(--surface-2);
    }
  }

  .grid {
    display: grid;
    gap: 4px;
    /* fixed cap: vh would follow the popup window, which is sized from this content */
    max-height: 340px;
    overflow-y: auto;
  }

  .cell {
    position: relative;

    &:hover .out {
      opacity: 1;
    }
  }

  .app {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 10px 4px 8px;
    border-radius: var(--radius-m);
    color: var(--fg);
    transition: background-color var(--dur-fast) ease;

    &:hover {
      background: var(--surface-hover);
    }

    &.attention {
      background: color-mix(in srgb, #f59e0b 22%, transparent);
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

  .out {
    position: absolute;
    top: 4px;
    left: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    border-radius: 10px;
    background: var(--surface-2);
    color: var(--fg-secondary);
    opacity: 0;
    transition: opacity var(--dur-fast) ease;

    &:hover {
      color: var(--fg);
      background: var(--surface-hover);
    }
  }
</style>
