<script lang="ts">
  import { i18n, t } from "@shared/i18n/index.svelte.ts";
  import { api } from "@shared/ipc.ts";
  import type { ModuleDockItem } from "@shared/types.ts";
  import { layout } from "../../state/layout.svelte.ts";
  import { showItemMenu } from "../../menus.ts";
  import { openPopup, tooltip } from "../../overlays.ts";
  import { shouldSuppressClick } from "../../sortable.svelte.ts";

  let { item }: { item: ModuleDockItem } = $props();

  let now = $state(new Date());
  $effect(() => {
    // align ticks to the minute boundary
    let timer: ReturnType<typeof setTimeout>;
    const tick = () => {
      now = new Date();
      timer = setTimeout(tick, 60_000 - (Date.now() % 60_000) + 50);
    };
    tick();
    return () => clearTimeout(timer);
  });

  const hours = $derived(String(now.getHours()).padStart(2, "0"));
  const minutes = $derived(String(now.getMinutes()).padStart(2, "0"));
  const fullDate = $derived(
    now.toLocaleDateString(i18n.language, { weekday: "long", year: "numeric", month: "long", day: "numeric" }),
  );

  let el: HTMLDivElement | null = $state(null);
</script>

<div class="weg-item-overlay" bind:this={el}>
  <div
    role="button"
    tabindex="-1"
    class="weg-item weg-module weg-module-text"
    use:tooltip={() => fullDate}
    onclick={() => !shouldSuppressClick() && openPopup(el!, "calendar", null, { key: "calendar" })}
    oncontextmenu={(e) =>
      showItemMenu(e, item, [{ type: "item", id: "settings", label: t("calendar.settings"), icon: "Settings" }], () =>
        api.shellAction("date-settings"),
      )}
    onkeydown={() => {}}
  >
    {#if layout.horizontal}
      <span class="clock-inline">{hours}:{minutes}</span>
    {:else}
      <span class="clock-stack"><span>{hours}</span><span>{minutes}</span></span>
    {/if}
  </div>
</div>
