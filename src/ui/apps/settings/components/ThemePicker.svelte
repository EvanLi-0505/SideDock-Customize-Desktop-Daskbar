<script lang="ts">
  import Icon from "@shared/components/Icon.svelte";
  import { t } from "@shared/i18n/index.svelte.ts";
  import type { ThemeMode } from "@shared/types.ts";
  import { editor } from "../state/editor.svelte.ts";

  const themes: { id: ThemeMode; icon: string }[] = [
    { id: "glass", icon: "Sparkles" },
    { id: "clear", icon: "Droplet" },
    { id: "dark", icon: "Moon" },
    { id: "light", icon: "Sun" },
    { id: "system", icon: "Monitor" },
  ];
</script>

<div class="themes" role="radiogroup">
  {#each themes as theme (theme.id)}
    <button
      type="button"
      role="radio"
      aria-checked={editor.value.theme === theme.id}
      class="theme-card"
      class:active={editor.value.theme === theme.id}
      onclick={() => editor.update((s) => (s.theme = theme.id))}
    >
      <div class="preview preview-{theme.id}">
        <div class="preview-dock">
          <span></span><span></span><span></span>
        </div>
        <div class="preview-window"></div>
      </div>
      <div class="theme-label">
        <Icon name={theme.icon} size={14} />
        {t(`theme.${theme.id}`)}
      </div>
    </button>
  {/each}
</div>

<style>
  .themes {
    display: grid;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    gap: 12px;
  }

  .theme-card {
    padding: 8px;
    border-radius: var(--radius-l);
    border: 2px solid transparent;
    background: var(--surface-2);
    transition:
      border-color var(--dur) var(--ease),
      transform var(--dur-fast) ease;

    &:hover {
      transform: translateY(-2px);
    }
    &.active {
      border-color: var(--accent);
    }
  }

  .preview {
    position: relative;
    height: 84px;
    border-radius: var(--radius-m);
    overflow: hidden;
    display: flex;
    gap: 8px;
    padding: 8px;
  }

  .preview-glass {
    background:
      radial-gradient(circle at 20% 20%, #9ec5ff, transparent 60%),
      radial-gradient(circle at 80% 70%, #f7b2d9, transparent 55%),
      #cfd9f5;
  }
  .preview-clear {
    background:
      radial-gradient(circle at 25% 30%, #7fd3ff, transparent 55%),
      radial-gradient(circle at 75% 75%, #b8a6ff, transparent 55%),
      #a9c9f2;
  }
  .preview-dark {
    background: linear-gradient(135deg, #1b1e24, #2a2f38);
  }
  .preview-light {
    background: linear-gradient(135deg, #f7f8fb, #e6e9f0);
  }
  .preview-system {
    background: linear-gradient(135deg, #f4f5f8 50%, #1f2329 50%);
  }

  .preview-dock {
    width: 18px;
    border-radius: 7px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 4px 0;

    span {
      width: 10px;
      height: 10px;
      border-radius: 3px;
      background: var(--accent);
      opacity: 0.85;
    }
  }

  .preview-window {
    flex: 1;
    border-radius: 6px;
  }

  .preview-glass .preview-dock,
  .preview-glass .preview-window {
    background: rgb(255 255 255 / 0.55);
    border: 1px solid rgb(255 255 255 / 0.8);
  }
  .preview-clear .preview-dock,
  .preview-clear .preview-window {
    background: rgb(255 255 255 / 0.22);
    border: 1px solid rgb(255 255 255 / 0.85);
    box-shadow: inset 0 1px 0 rgb(255 255 255 / 0.9);
  }
  .preview-dark .preview-dock,
  .preview-dark .preview-window {
    background: #333944;
  }
  .preview-light .preview-dock,
  .preview-light .preview-window {
    background: #ffffff;
    box-shadow: 0 1px 3px rgb(0 0 0 / 0.1);
  }
  .preview-system .preview-dock {
    background: #ffffff;
  }
  .preview-system .preview-window {
    background: linear-gradient(135deg, #ffffff 50%, #333944 50%);
  }

  .theme-label {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding-top: 8px;
    font-weight: 700;
    font-size: 13px;
  }
</style>
