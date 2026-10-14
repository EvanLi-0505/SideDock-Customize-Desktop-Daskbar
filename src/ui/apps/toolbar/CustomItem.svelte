<!-- A button the user added in Settings > Toolbar: opens its program, file, folder or
     web address. Its icon is the target's own unless one was chosen. -->
<script lang="ts">
  import Icon from "@shared/components/Icon.svelte";
  import { api, iconUrl } from "@shared/ipc.ts";
  import type { CustomButton } from "@shared/types.ts";
  import { tooltip } from "./overlays.ts";

  let { button }: { button: CustomButton } = $props();

  const isUrl = $derived(/^[a-z][a-z0-9+.-]*:(\/\/)?/i.test(button.target) && !/^[a-z]:\\/i.test(button.target));
</script>

<button
  type="button"
  class="tb-button custom"
  aria-label={button.name || button.target}
  use:tooltip={() => button.name || button.target}
  onclick={() => api.toolbarRunCustom(button.id)}
>
  {#if button.icon}
    <Icon name={button.icon} />
  {:else if isUrl}
    <Icon name="Globe" />
  {:else}
    <img src={iconUrl(button.target)} alt="" draggable="false" />
  {/if}
</button>
