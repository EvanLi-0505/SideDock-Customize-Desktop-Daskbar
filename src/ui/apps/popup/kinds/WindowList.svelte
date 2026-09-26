<!-- Multiple windows of one app (Seelen UI shows them in the "weg-preview" widget). -->
<script lang="ts">
  import Icon from "@shared/components/Icon.svelte";
  import { api, iconUrl } from "@shared/ipc.ts";
  import type { UserAppWindow } from "@shared/types.ts";
  import type { PopupProps } from "../registry.ts";

  let { data, close }: PopupProps = $props();

  // svelte-ignore state_referenced_locally
  let windows = $state<UserAppWindow[]>(data.windows);

  function focus(w: UserAppWindow) {
    api.windowFocus(w.hwnd);
    close();
  }

  function closeWindow(e: MouseEvent, w: UserAppWindow) {
    e.stopPropagation();
    api.windowClose(w.hwnd);
    windows = windows.filter((x) => x.hwnd !== w.hwnd);
    if (!windows.length) close();
  }
</script>

<div class="popup-surface windows">
  <div class="popup-header">{data.title}</div>
  {#each windows as w (w.hwnd)}
    <div
      role="button"
      tabindex="0"
      class="window-row"
      class:minimized={w.isIconic}
      onclick={() => focus(w)}
      onauxclick={(e) => e.button === 1 && closeWindow(e, w)}
      onkeydown={(e) => e.key === "Enter" && focus(w)}
    >
      <img src={iconUrl(w.path, w.umid)} alt="" />
      <span class="window-title">{w.title}</span>
      <button type="button" class="window-close" onclick={(e) => closeWindow(e, w)}>
        <Icon name="X" size={14} />
      </button>
    </div>
  {/each}
</div>

<style>
  .windows {
    width: 340px;
    padding: 6px;
  }

  .window-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 10px;
    border-radius: var(--radius-s);
    font-weight: 600;
    font-size: 13.5px;
    transition: background-color var(--dur-fast) ease;

    &:hover {
      background: var(--surface-hover);
    }
    &.minimized .window-title {
      color: var(--fg-muted);
    }

    img {
      width: 20px;
      height: 20px;
      object-fit: contain;
    }
  }

  .window-title {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .window-close {
    display: flex;
    padding: 4px;
    border-radius: 6px;
    color: var(--fg-muted);
    opacity: 0;

    .window-row:hover & {
      opacity: 1;
    }
    &:hover {
      background: var(--danger-soft);
      color: var(--danger);
    }
  }
</style>
