<!-- A folder stack on the dock: up to four of its apps' icons; a click opens it. -->
<script lang="ts">
  import { iconUrl } from "@shared/ipc.ts";
  import type { StackDockItem } from "@shared/types.ts";
  import { getWindowsForItem, stackActions } from "../../state/items.svelte.ts";
  import { systemState } from "../../state/system.svelte.ts";
  import { activateApp, showStackMenu } from "../../menus.svelte.ts";
  import { openPopup, tooltip } from "../../overlays.ts";
  import { shouldSuppressClick } from "../../sortable.svelte.ts";

  let { item }: { item: StackDockItem } = $props();

  const members = $derived(
    item.items.map((app) => ({ app, windows: getWindowsForItem(app, systemState.windows) })),
  );
  const running = $derived(members.some((m) => m.windows.length > 0));
  const focused = $derived(members.some((m) => m.windows.some((w) => w.hwnd === systemState.focused?.hwnd)));
  const attention = $derived(members.some((m) => m.windows.some((w) => w.attention)));

  let el: HTMLDivElement | null = $state(null);

  function open() {
    if (shouldSuppressClick()) return;
    const data = {
      name: item.name,
      items: members.map((m) => ({
        id: m.app.id,
        label: m.app.displayName,
        icon: iconUrl(m.app.relaunch?.command || m.app.path, m.app.umid),
        running: m.windows.length > 0,
        attention: m.windows.some((w) => w.attention),
      })),
    };
    openPopup(el!, "stack", data, {
      key: `stack:${item.id}`,
      onAction: (action, value) => {
        if (action === "rename") {
          stackActions.rename(item.id, String(value));
        } else if (action === "move-out") {
          stackActions.moveOut(item.id, String(value));
        } else if (action === "open") {
          const member = members.find((m) => m.app.id === value);
          if (member) activateApp(member.app, member.windows, el!);
        }
      },
    });
  }
</script>

<div class="weg-item-overlay" bind:this={el}>
  <div
    role="button"
    tabindex="-1"
    class="weg-item weg-stack"
    class:attention
    use:tooltip={() => item.name}
    onclick={open}
    oncontextmenu={(e) => showStackMenu(e, item)}
    onkeydown={() => {}}
  >
    <span class="weg-stack-icons">
      {#each item.items.slice(0, 4) as app (app.id)}
        <img src={iconUrl(app.relaunch?.command || app.path, app.umid)} alt="" draggable="false" />
      {/each}
    </span>
  </div>
  <div
    class="weg-item-open-sign"
    class:weg-item-open-sign-active={running}
    class:weg-item-open-sign-focused={focused}
    class:weg-item-open-sign-attention={attention}
  ></div>
</div>

<style>
  .weg-stack-icons {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 8%;
    width: 78%;
    height: 78%;
    padding: 8%;
    border-radius: 22%;
    background: color-mix(in srgb, var(--fg, #fff) 14%, transparent);

    img {
      width: 100%;
      height: 100%;
      object-fit: contain;
      pointer-events: none;
    }
  }
</style>
