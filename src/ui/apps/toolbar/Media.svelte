<script lang="ts">
  import Icon from "@shared/components/Icon.svelte";
  import { api } from "@shared/ipc.ts";
  import { tooltip } from "./overlays.ts";
  import { toolbar } from "./state.svelte.ts";

  const media = $derived(toolbar.system.media);
  const tip = $derived(media ? `${media.title}${media.artist ? ` - ${media.artist}` : ""}` : "");

  function command(e: MouseEvent, cmd: "togglePlayPause" | "next" | "previous") {
    e.stopPropagation();
    api.mediaCommand(cmd);
  }
</script>

{#if media}
  <div class="media" role="group" use:tooltip={() => tip}>
    {#if media.thumbnail}
      <img class="media-thumb" src={media.thumbnail} alt="" />
    {:else}
      <span class="media-thumb"><Icon name="Music" size={12} /></span>
    {/if}
    <span class="media-title">{media.title}</span>
    <button type="button" class="tb-button small" onclick={(e) => command(e, "previous")}>
      <Icon name="SkipBack" size={13} />
    </button>
    <button type="button" class="tb-button small" onclick={(e) => command(e, "togglePlayPause")}>
      <Icon name={media.playing ? "Pause" : "Play"} size={13} />
    </button>
    <button type="button" class="tb-button small" onclick={(e) => command(e, "next")}>
      <Icon name="SkipForward" size={13} />
    </button>
  </div>
{/if}
