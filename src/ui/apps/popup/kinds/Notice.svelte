<!-- A short message next to the element that caused it (e.g. a custom button that could
     not run). Closes like any popup. -->
<script lang="ts">
  import Icon from "@shared/components/Icon.svelte";
  import type { PopupProps } from "../registry.ts";

  let { data }: PopupProps = $props();
</script>

<div class="popup-surface notice">
  <Icon name="CircleAlert" size={16} />
  <span>{data?.text ?? ""}</span>
</div>

<style>
  .notice {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    max-width: 320px;
    padding: 10px 12px;
    font-size: 13px;
    line-height: 1.45;
  }
</style>
