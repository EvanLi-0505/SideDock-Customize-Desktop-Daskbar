<script lang="ts">
  import { i18n, t } from "@shared/i18n/index.svelte.ts";
  import { api } from "@shared/ipc.ts";
  import { settingsState } from "@shared/state/settings.svelte.ts";
  import { openMenu, openPopup, tooltip } from "./overlays.ts";

  let { twoLines }: { twoLines: boolean } = $props();

  const seconds = $derived(settingsState.value.toolbar.showSeconds);

  let now = $state(new Date());
  $effect(() => {
    // ticks aligned to the second / minute boundary
    const step = seconds ? 1000 : 60_000;
    let timer: ReturnType<typeof setTimeout>;
    const tick = () => {
      now = new Date();
      timer = setTimeout(tick, step - (Date.now() % step) + 20);
    };
    tick();
    return () => clearTimeout(timer);
  });

  const time = $derived(
    now.toLocaleTimeString(i18n.language, {
      hour: "2-digit",
      minute: "2-digit",
      second: seconds ? "2-digit" : undefined,
      hour12: false,
    }),
  );
  const date = $derived(now.toLocaleDateString(i18n.language, { year: "numeric", month: "numeric", day: "numeric" }));
  const shortDate = $derived(now.toLocaleDateString(i18n.language, { month: "short", day: "numeric", weekday: "short" }));
  const fullDate = $derived(
    now.toLocaleDateString(i18n.language, { weekday: "long", year: "numeric", month: "long", day: "numeric" }),
  );

  let el: HTMLButtonElement | null = $state(null);
</script>

<button
  type="button"
  class="tb-button clock"
  class:two-lines={twoLines}
  bind:this={el}
  use:tooltip={() => fullDate}
  onclick={() => openPopup(el!, "calendar", null)}
  oncontextmenu={(e) =>
    openMenu(e, [{ type: "item", id: "settings", label: t("calendar.settings"), icon: "Settings" }], () =>
      api.shellAction("date-settings"),
    )}
>
  {#if twoLines}
    <span class="clock-time">{time}</span>
    <span class="clock-date">{date}</span>
  {:else}
    <span class="clock-date">{shortDate}</span>
    <span class="clock-time">{time}</span>
  {/if}
</button>
