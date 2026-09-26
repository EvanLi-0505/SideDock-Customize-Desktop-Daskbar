<script lang="ts">
  import Icon from "@shared/components/Icon.svelte";
  import type { MenuEntry } from "@shared/menu.ts";
  import type { PopupProps } from "../registry.ts";

  let { data, placement, emit }: PopupProps = $props();

  // the menu is rendered once per popup token
  // svelte-ignore state_referenced_locally
  let items = $state<MenuEntry[]>($state.snapshot(data.items) as MenuEntry[]);
  let openSub = $state<string | null>(null);

  /** height left on screen in the direction the popup grows (the backend caps it there) */
  let maxHeight = $state<number | null>(null);
  function availableHeight(): number {
    const availTop = (screen as Screen & { availTop?: number }).availTop ?? 0;
    const availBottom = availTop + screen.availHeight;
    return placement === "above"
      ? window.screenY + window.innerHeight - availTop - 8
      : availBottom - window.screenY - 8;
  }

  const submenu = $derived(
    items.find((i): i is Extract<MenuEntry, { type: "submenu" }> => i.type === "submenu" && i.id === openSub),
  );

  function openSubmenu(id: string | null) {
    if (id) maxHeight = Math.max(120, availableHeight());
    openSub = id;
  }

  function activate(entry: MenuEntry) {
    if (entry.type === "separator") return;
    if (entry.type === "submenu") {
      openSubmenu(openSub === entry.id ? null : entry.id);
      return;
    }
    if (entry.disabled) return;
    if (entry.checked !== undefined) {
      entry.checked = !entry.checked;
      emit(entry.id, { checked: entry.checked }, false);
      return;
    }
    emit(entry.id, null, true);
  }
</script>

{#snippet list(entries: MenuEntry[])}
  <div class="menu" style:max-height={maxHeight ? `${maxHeight}px` : null}>
    {#each entries as entry, i (i)}
      {#if entry.type === "separator"}
        <div class="menu-separator"></div>
      {:else}
        <button
          type="button"
          class="menu-item"
          class:danger={entry.type === "item" && entry.danger}
          class:open={entry.type === "submenu" && openSub === entry.id}
          disabled={entry.type === "item" && entry.disabled}
          onmouseenter={() => entry.type === "submenu" && openSubmenu(entry.id)}
          onclick={() => activate(entry)}
        >
          <span class="menu-icon">
            {#if entry.type === "item" && entry.checked !== undefined}
              <span class="menu-check" class:checked={entry.checked}>
                {#if entry.checked}<Icon name="Check" size={12} strokeWidth={3} />{/if}
              </span>
            {:else if entry.type === "item" && entry.image}
              <img src={entry.image} alt="" />
            {:else if entry.icon}
              <Icon name={entry.icon} size={16} />
            {/if}
          </span>
          {#if entry.type === "item" && entry.checked !== undefined && entry.icon}
            <span class="menu-icon muted"><Icon name={entry.icon} size={16} /></span>
          {/if}
          <span class="menu-label">{entry.label}</span>
          {#if entry.type === "submenu"}
            <Icon name="ChevronRight" size={14} class="menu-chevron" />
          {/if}
        </button>
      {/if}
    {/each}
  </div>
{/snippet}

<!-- one surface for both columns: with the acrylic backdrop the whole window is visible -->
<div class="menu-root popup-surface placement-{placement}">
  {@render list(items)}
  {#if submenu}
    <div class="menu-divider"></div>
    {@render list(submenu.items)}
  {/if}
</div>

<style>
  /* the submenu opens away from the dock and the main column never moves */
  .menu-root {
    display: flex;
    align-items: flex-start;

    &.placement-left {
      flex-direction: row-reverse;
    }
    &.placement-above {
      align-items: flex-end;
    }
  }

  .menu-divider {
    align-self: stretch;
    width: 1px;
    margin: 8px 0;
    background: var(--border-strong);
  }

  .menu {
    overflow-y: auto;
    min-width: 220px;
    max-width: 320px;
    padding: 6px;
    display: flex;
    flex-direction: column;
  }

  .menu-separator {
    height: 1px;
    margin: 5px 8px;
    background: var(--border-strong);
  }

  .menu-item {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 8px 10px;
    border-radius: var(--radius-s);
    text-align: left;
    font-weight: 600;
    font-size: 13.5px;
    color: var(--fg);
    transition: background-color var(--dur-fast) ease;

    &:hover:not(:disabled),
    &.open {
      background: var(--surface-hover);
    }
    &:active:not(:disabled) {
      background: var(--surface-pressed);
    }
    &:disabled {
      color: var(--fg-disabled);
      cursor: default;
    }
    &.danger {
      color: var(--danger);
    }
  }

  .menu-icon {
    width: 18px;
    height: 18px;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    color: var(--fg-secondary);

    img {
      width: 18px;
      height: 18px;
      object-fit: contain;
    }
    &.muted {
      color: var(--fg-muted);
    }
  }

  .danger .menu-icon {
    color: var(--danger);
  }

  .menu-check {
    width: 16px;
    height: 16px;
    border-radius: 4px;
    border: 1.5px solid var(--border-strong);
    display: flex;
    align-items: center;
    justify-content: center;
    color: white;

    &.checked {
      background: var(--accent);
      border-color: var(--accent);
    }
  }

  .menu-label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  :global(.menu-chevron) {
    color: var(--fg-muted);
  }
</style>
