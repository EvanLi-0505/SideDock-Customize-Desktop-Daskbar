<script lang="ts">
  import { tick } from "svelte";
  import { api, Events, on } from "@shared/ipc.ts";
  import type { Placement } from "@shared/types.ts";
  import { POPUPS } from "./registry.ts";

  interface RenderPayload {
    token: number;
    kind: string;
    data: unknown;
    owner: string;
    placement: Placement;
  }

  let current = $state<RenderPayload | null>(null);
  let shell: HTMLDivElement | null = $state(null);

  const Popup = $derived(current ? POPUPS[current.kind] : null);

  function measure(): { width: number; height: number } {
    const rect = shell!.getBoundingClientRect();
    return { width: Math.ceil(rect.width), height: Math.ceil(rect.height) };
  }

  on<RenderPayload>(Events.PopupRender, async (payload) => {
    current = payload;
    await tick();
    // wait for images/fonts to lay out before measuring
    requestAnimationFrame(() => {
      if (!shell || current?.token !== payload.token) return;
      const { width, height } = measure();
      api.popupReady(payload.token, width, height);
    });
  });

  // content that changes size while open (submenus, toggles) resizes the window
  $effect(() => {
    if (!shell) return;
    const el = shell;
    const observer = new ResizeObserver(() => {
      if (!current) return;
      const { width, height } = measure();
      api.popupResize(current.token, width, height);
    });
    observer.observe(el);
    return () => observer.disconnect();
  });

  function emit(action: string, value?: unknown, close = true) {
    if (!current) return;
    api.popupAction(current.kind, action, value ?? null, close);
  }
</script>

<div class="popup-shell" bind:this={shell}>
  {#if current && Popup}
    {#key current.token}
      <Popup data={current.data} placement={current.placement} {emit} close={() => api.popupClose()} />
    {/key}
  {/if}
</div>
