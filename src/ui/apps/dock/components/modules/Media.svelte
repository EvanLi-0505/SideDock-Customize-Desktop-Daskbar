<!-- Port of Seelen UI `weg/components/items/MediaSession.svelte` -->
<script lang="ts">
  import Icon from "@shared/components/Icon.svelte";
  import { t } from "@shared/i18n/index.svelte.ts";
  import { api } from "@shared/ipc.ts";
  import type { ModuleDockItem } from "@shared/types.ts";
  import { systemState } from "../../state/system.svelte.ts";
  import { showItemMenu } from "../../menus.svelte.ts";
  import { tooltip } from "../../overlays.ts";

  let { item }: { item: ModuleDockItem } = $props();

  const media = $derived(systemState.system.media);
  const tip = $derived(media ? `${media.title}${media.artist ? ` - ${media.artist}` : ""}` : t("media.notPlaying"));

  function command(e: MouseEvent, cmd: "togglePlayPause" | "next" | "previous") {
    e.stopPropagation();
    api.mediaCommand(cmd);
  }
</script>

<div class="weg-item-overlay">
  <div
    role="group"
    class="weg-item weg-item-large media-session-container"
    use:tooltip={() => tip}
    oncontextmenu={(e) => showItemMenu(e, item)}
  >
    {#if media?.thumbnail}
      <img class="media-session-bg" src={media.thumbnail} alt="" />
    {/if}
    <div class="media-session">
      <div class="media-session-thumb">
        {#if media?.thumbnail}
          <img src={media.thumbnail} alt="" />
        {:else}
          <Icon name="Music" size="60%" />
        {/if}
      </div>
      <div class="media-session-actions">
        <button type="button" disabled={!media} onclick={(e) => command(e, "previous")}>
          <Icon name="SkipBack" size={14} />
        </button>
        <button type="button" disabled={!media} onclick={(e) => command(e, "togglePlayPause")}>
          <Icon name={media?.playing ? "Pause" : "Play"} size={14} />
        </button>
        <button type="button" disabled={!media} onclick={(e) => command(e, "next")}>
          <Icon name="SkipForward" size={14} />
        </button>
      </div>
    </div>
  </div>
</div>
