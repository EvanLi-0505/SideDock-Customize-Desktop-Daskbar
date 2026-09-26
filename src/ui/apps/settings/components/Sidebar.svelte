<script lang="ts">
  import Icon from "@shared/components/Icon.svelte";
  import { t, type TranslationKey } from "@shared/i18n/index.svelte.ts";
  import { api } from "@shared/ipc.ts";
  import { PAGES, type PageId } from "../pages.ts";

  let { active, onselect }: { active: PageId; onselect: (id: PageId) => void } = $props();

  let version = $state("");
  api.getAppInfo().then((info) => (version = info.version));
</script>

<nav class="sidebar">
  {#each PAGES as page, i (page.id)}
    {#if i > 0 && PAGES[i - 1]!.group !== page.group}
      <div class="divider"></div>
    {/if}
    <button type="button" class="nav-item" class:active={active === page.id} onclick={() => onselect(page.id)}>
      <Icon name={page.icon} size={18} />
      <span>{t(page.label as TranslationKey)}</span>
    </button>
  {/each}
  <div class="spacer"></div>
  <div class="footer">SideDock v{version}</div>
</nav>

<style>
  .sidebar {
    width: 244px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 12px 10px;
    border-right: 1px solid var(--border);
    background: var(--sidebar-bg);
    overflow-y: auto;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    border-radius: var(--radius-m);
    font-weight: 700;
    font-size: 14.5px;
    color: var(--fg);
    text-align: left;
    transition:
      background-color var(--dur-fast) ease,
      color var(--dur-fast) ease;

    &:hover:not(.active) {
      background: var(--surface-hover);
    }
    &.active {
      background: var(--accent);
      color: var(--accent-contrast);
    }
  }

  .divider {
    height: 1px;
    margin: 8px 6px;
    background: var(--border-strong);
  }

  .spacer {
    flex: 1;
  }

  .footer {
    padding: 8px 12px;
    font-size: 12px;
    color: var(--fg-muted);
  }
</style>
