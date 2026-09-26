<!-- Card grouping settings rows, same visual structure as Seelen UI's SettingsGroup -->
<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    title?: string;
    header?: Snippet;
    children: Snippet;
  }
  let { title, header, children }: Props = $props();
</script>

<section class="group">
  {#if header}
    <div class="group-header">{@render header()}</div>
  {:else if title}
    <h2 class="group-title">{title}</h2>
  {/if}
  <div class="group-body" class:nested={!!(title || header)}>
    {@render children()}
  </div>
</section>

<style>
  .group {
    padding: 14px 18px;
    border-radius: var(--radius-l);
    background: var(--card-bg);
    border: 1px solid var(--border);
  }

  .group-title {
    font-size: 16px;
    font-weight: 700;
    margin-bottom: 6px;
  }

  .group-header {
    margin-bottom: 6px;
  }

  .group-body.nested {
    border-left: 2px solid var(--border-strong);
    padding-left: 16px;
    margin-left: 2px;
  }
</style>
