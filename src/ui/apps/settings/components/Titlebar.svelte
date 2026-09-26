<script lang="ts">
  import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
  import Icon from "@shared/components/Icon.svelte";
  import { t } from "@shared/i18n/index.svelte.ts";
  import { editor } from "../state/editor.svelte.ts";
  import logo from "../../../../static/icons/logo.svg";

  let { title }: { title: string } = $props();
  const win = getCurrentWebviewWindow();

  function close() {
    editor.commit();
    win.close();
  }
</script>

<header class="titlebar" data-tauri-drag-region>
  <div class="brand" data-tauri-drag-region>
    <img src={logo} alt="" />
    <span>SideDock</span>
  </div>
  <h1 class="page-title" data-tauri-drag-region>{title}</h1>
  <div class="actions">
    {#if editor.error}
      <span class="save-error" title={editor.error}><Icon name="CircleAlert" size={16} /></span>
    {/if}
    <button type="button" class="btn btn-ghost" disabled={!editor.dirty} onclick={() => editor.revert()}>
      {t("titlebar.cancel")}
    </button>
    <button type="button" class="btn btn-danger" onclick={close}>
      {t("titlebar.close")}
    </button>
    <button type="button" class="win-btn" title={t("titlebar.minimize")} onclick={() => win.minimize()}>
      <Icon name="Minus" size={16} />
    </button>
  </div>
</header>

<style>
  .titlebar {
    display: flex;
    align-items: center;
    height: 56px;
    padding: 0 12px 0 16px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 228px;
    font-family: var(--font-display);
    font-size: 20px;
    font-weight: 700;

    img {
      width: 30px;
      height: 30px;
      pointer-events: none;
    }
    span {
      pointer-events: none;
    }
  }

  .page-title {
    flex: 1;
    font-family: var(--font-display);
    font-size: 20px;
    font-weight: 700;
    pointer-events: none;
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .save-error {
    color: var(--danger);
    display: flex;
  }

  .win-btn {
    display: flex;
    padding: 8px;
    border-radius: var(--radius-s);
    color: var(--fg-secondary);

    &:hover {
      background: var(--surface-hover);
    }
  }
</style>
