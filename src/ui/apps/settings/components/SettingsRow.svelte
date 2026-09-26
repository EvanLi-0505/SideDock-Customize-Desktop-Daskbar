<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    label: string;
    hint?: string;
    strong?: boolean;
    children: Snippet;
  }
  let { label, hint, strong = false, children }: Props = $props();
</script>

<div class="row">
  <div class="row-text">
    <span class="row-label" class:strong>{label}</span>
    {#if hint}<span class="row-hint">{hint}</span>{/if}
  </div>
  <div class="row-control">{@render children()}</div>
</div>

<style>
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 24px;
    min-height: 48px;
    padding: 6px 0;
  }

  .row-text {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .row-label {
    font-size: 15px;

    &.strong {
      font-weight: 700;
      font-size: 16px;
    }
  }

  .row-hint {
    font-size: 12.5px;
    color: var(--fg-muted);
  }

  .row-control {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 8px;
  }
</style>
