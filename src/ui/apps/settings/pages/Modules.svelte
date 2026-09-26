<script lang="ts">
  import Icon from "@shared/components/Icon.svelte";
  import Switch from "@shared/components/Switch.svelte";
  import { t, type TranslationKey } from "@shared/i18n/index.svelte.ts";
  import { MODULES } from "@shared/modules.ts";
  import { dockItemsActions, dockItemsState } from "../state/dockItems.svelte.ts";
</script>

<div class="stack">
  <div class="description">{t("modules.description")}</div>
  <div class="grid">
    {#each MODULES as module (module.id)}
      {@const enabled = dockItemsState.hasModule(module.id)}
      <div class="module" class:enabled>
        <span class="module-icon"><Icon name={module.icon} size={20} /></span>
        <div class="module-text">
          <span class="module-name">{t(`module.${module.id}` as TranslationKey)}</span>
          <span class="module-desc">{t(`moduleDesc.${module.id}` as TranslationKey)}</span>
        </div>
        <Switch
          checked={enabled}
          label={t("modules.onDock")}
          onchange={(v) => dockItemsActions.toggleModule(module.id, v)}
        />
      </div>
    {/each}
  </div>
</div>

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    gap: 12px;
  }

  .module {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 14px 16px;
    border-radius: var(--radius-l);
    background: var(--card-bg);
    border: 1px solid var(--border);
    transition: border-color var(--dur) var(--ease);

    &.enabled {
      border-color: color-mix(in srgb, var(--accent) 45%, transparent);
    }
  }

  .module-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 40px;
    height: 40px;
    border-radius: 12px;
    background: var(--surface-2);
    color: var(--fg-secondary);

    .enabled & {
      background: var(--accent-soft);
      color: var(--accent);
    }
  }

  .module-text {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .module-name {
    font-weight: 700;
  }

  .module-desc {
    font-size: 12.5px;
    color: var(--fg-muted);
  }
</style>
