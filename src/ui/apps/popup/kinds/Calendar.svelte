<script lang="ts">
  import Icon from "@shared/components/Icon.svelte";
  import { i18n, t } from "@shared/i18n/index.svelte.ts";
  import { api } from "@shared/ipc.ts";
  import type { PopupProps } from "../registry.ts";

  let { close }: PopupProps = $props();

  const today = new Date();
  let view = $state(new Date(today.getFullYear(), today.getMonth(), 1));
  let now = $state(new Date());

  $effect(() => {
    const timer = setInterval(() => (now = new Date()), 1000);
    return () => clearInterval(timer);
  });

  // Monday first, like Windows in most locales
  const weekdays = $derived(
    Array.from({ length: 7 }, (_, i) =>
      new Date(2024, 0, 1 + i).toLocaleDateString(i18n.language, { weekday: "narrow" }),
    ),
  );

  const days = $derived.by(() => {
    const first = new Date(view.getFullYear(), view.getMonth(), 1);
    const offset = (first.getDay() + 6) % 7;
    return Array.from({ length: 42 }, (_, i) => new Date(view.getFullYear(), view.getMonth(), 1 - offset + i));
  });

  const title = $derived(view.toLocaleDateString(i18n.language, { year: "numeric", month: "long" }));
  const time = $derived(now.toLocaleTimeString(i18n.language, { hour: "2-digit", minute: "2-digit", second: "2-digit" }));
  const longDate = $derived(
    now.toLocaleDateString(i18n.language, { weekday: "long", year: "numeric", month: "long", day: "numeric" }),
  );

  const isToday = (d: Date) =>
    d.getFullYear() === today.getFullYear() && d.getMonth() === today.getMonth() && d.getDate() === today.getDate();

  function shift(months: number) {
    view = new Date(view.getFullYear(), view.getMonth() + months, 1);
  }
</script>

<div class="popup-surface calendar">
  <div class="clock">
    <div class="clock-time">{time}</div>
    <div class="clock-date">{longDate}</div>
  </div>

  <div class="calendar-head">
    <span class="calendar-title">{title}</span>
    <div class="calendar-nav">
      <button type="button" onclick={() => shift(-1)}><Icon name="ChevronLeft" /></button>
      <button type="button" title={t("calendar.today")} onclick={() => (view = new Date(today.getFullYear(), today.getMonth(), 1))}>
        <Icon name="House" />
      </button>
      <button type="button" onclick={() => shift(1)}><Icon name="ChevronRight" /></button>
    </div>
  </div>

  <div class="grid">
    {#each weekdays as w, i (i)}
      <span class="weekday">{w}</span>
    {/each}
    {#each days as d (d.getTime())}
      <span class="day" class:other={d.getMonth() !== view.getMonth()} class:today={isToday(d)}>
        {d.getDate()}
      </span>
    {/each}
  </div>

  <button
    type="button"
    class="popup-link"
    onclick={() => {
      api.shellAction("date-settings");
      close();
    }}
  >
    {t("calendar.settings")}
  </button>
</div>

<style>
  .calendar {
    width: 320px;
    padding: 16px;
  }

  .clock {
    padding: 2px 4px 14px;
    border-bottom: 1px solid var(--border);
    margin-bottom: 12px;
  }

  .clock-time {
    font-family: var(--font-display);
    font-size: 30px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    letter-spacing: -0.5px;
  }

  .clock-date {
    color: var(--accent);
    font-weight: 600;
  }

  .calendar-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 8px;
    padding: 0 4px;
  }

  .calendar-title {
    font-weight: 700;
    font-size: 15px;
  }

  .calendar-nav {
    display: flex;
    gap: 2px;

    button {
      display: flex;
      padding: 6px;
      border-radius: 6px;
      color: var(--fg-secondary);

      &:hover {
        background: var(--surface-hover);
      }
    }
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    gap: 2px;
    text-align: center;
  }

  .weekday {
    font-size: 12px;
    font-weight: 700;
    color: var(--fg-muted);
    padding: 6px 0;
  }

  .day {
    aspect-ratio: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 8px;
    font-size: 13px;
    font-variant-numeric: tabular-nums;

    &.other {
      color: var(--fg-disabled);
    }
    &.today {
      background: var(--accent);
      color: var(--accent-contrast);
      font-weight: 700;
    }
  }
</style>
