<!-- Full-screen overlay: launcher (SideDock start menu) or window switcher.
     The backend moves the hidden window, asks for a view (overlay-show) and shows the
     window once the view reports ready, so an old frame is never visible. -->
<script lang="ts">
  import { onMount, tick } from "svelte";
  import { api, Events, on } from "@shared/ipc.ts";
  import type { OverlayMode } from "@shared/types.ts";
  import Launcher from "./components/Launcher.svelte";
  import Switcher from "./components/Switcher.svelte";

  let mode = $state<OverlayMode | null>(null);
  // a new session per opening resets search, selection and scroll
  let session = $state(0);

  onMount(() => {
    const shown = on<{ token: number; mode: OverlayMode }>(Events.OverlayShow, async (p) => {
      mode = p.mode;
      session++;
      await tick();
      requestAnimationFrame(() => api.overlayReady(p.token));
    });
    const hidden = on(Events.OverlayHidden, () => (mode = null));
    return () => {
      shown.then((f) => f());
      hidden.then((f) => f());
    };
  });
</script>

{#key session}
  {#if mode === "launcher"}
    <Launcher />
  {:else if mode === "switcher"}
    <Switcher />
  {/if}
{/key}
