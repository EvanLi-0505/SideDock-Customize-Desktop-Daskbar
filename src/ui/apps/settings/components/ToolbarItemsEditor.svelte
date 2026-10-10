<!-- Which items the toolbar shows and where: drag the chips between its left, center and
     right sections, or into "Not shown". -->
<script lang="ts">
  import Icon from "@shared/components/Icon.svelte";
  import { t } from "@shared/i18n/index.svelte.ts";
  import { TOOLBAR_SECTIONS, toolbarItemMeta, unusedToolbarItems } from "@shared/toolbarItems.ts";
  import { insertionIndex } from "@shared/trayDrag.ts";
  import type { ToolbarItemId } from "@shared/types.ts";
  import { editor } from "../state/editor.svelte.ts";

  type Zone = (typeof TOOLBAR_SECTIONS)[number] | "unused";
  const ZONES: Zone[] = [...TOOLBAR_SECTIONS, "unused"];

  const items = $derived(editor.value.toolbar.items);
  const listOf = (zone: Zone): ToolbarItemId[] => (zone === "unused" ? unusedToolbarItems(items) : items[zone]);
  /** the zone's items without the dragged one (insertion indexes refer to these) */
  const staying = (zone: Zone) => listOf(zone).filter((x) => x !== drag?.id);

  let drag = $state<{ id: ToolbarItemId; x: number; y: number; dx: number; dy: number } | null>(null);
  let target = $state<{ zone: Zone; index: number } | null>(null);
  let press: { id: ToolbarItemId; x: number; y: number; dx: number; dy: number; pointer: number } | null = null;
  const zoneEls: Partial<Record<Zone, HTMLElement>> = {};

  function down(e: PointerEvent, id: ToolbarItemId) {
    if (e.button !== 0) return;
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    press = { id, x: e.clientX, y: e.clientY, dx: e.clientX - r.left, dy: e.clientY - r.top, pointer: e.pointerId };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function move(e: PointerEvent) {
    if (!press || e.pointerId !== press.pointer) return;
    if (!drag && Math.hypot(e.clientX - press.x, e.clientY - press.y) < 4) return;
    drag = { id: press.id, x: e.clientX, y: e.clientY, dx: press.dx, dy: press.dy };
    target = null;
    for (const zone of ZONES) {
      const el = zoneEls[zone];
      const r = el?.getBoundingClientRect();
      if (!el || !r || e.clientX < r.left || e.clientX > r.right || e.clientY < r.top || e.clientY > r.bottom) {
        continue;
      }
      const chips = Array.from(el.querySelectorAll<HTMLElement>(".chip[data-id]")).filter(
        (c) => c.dataset.id !== press!.id,
      );
      target = { zone, index: insertionIndex(chips, e.clientX, e.clientY) };
    }
  }

  function up(e: PointerEvent) {
    if (!press || e.pointerId !== press.pointer) return;
    const id = press.id;
    press = null;
    if (drag && target) {
      const { zone, index } = target;
      editor.update((s) => {
        for (const section of TOOLBAR_SECTIONS) {
          s.toolbar.items[section] = s.toolbar.items[section].filter((i) => i !== id);
        }
        if (zone !== "unused") s.toolbar.items[zone].splice(index, 0, id);
      });
    }
    drag = null;
    target = null;
  }

  function label(zone: Zone): string {
    return t(`toolbar.section.${zone}`);
  }
</script>

<div class="editor">
  {#each ZONES as zone (zone)}
    <div class="zone" class:unused={zone === "unused"} class:over={target?.zone === zone}>
      <span class="zone-label">{label(zone)}</span>
      <div class="chips" bind:this={zoneEls[zone]}>
        {#each listOf(zone) as id (id)}
          {@const meta = toolbarItemMeta(id)}
          {#if target?.zone === zone && id !== drag?.id && target.index === staying(zone).indexOf(id)}
            <span class="marker"></span>
          {/if}
          <button
            type="button"
            class="chip"
            class:dragging={drag?.id === id}
            data-id={id}
            onpointerdown={(e) => down(e, id)}
            onpointermove={move}
            onpointerup={up}
            onpointercancel={up}
          >
            <Icon name={meta.icon} size={14} />
            {t(meta.label)}
          </button>
        {/each}
        {#if target?.zone === zone && target.index >= staying(zone).length}
          <span class="marker"></span>
        {/if}
        {#if !listOf(zone).length}
          <span class="empty">{t("toolbar.section.empty")}</span>
        {/if}
      </div>
    </div>
  {/each}
</div>

{#if drag}
  {@const meta = toolbarItemMeta(drag.id)}
  <div class="chip ghost" style:left="{drag.x - drag.dx}px" style:top="{drag.y - drag.dy}px">
    <Icon name={meta.icon} size={14} />
    {t(meta.label)}
  </div>
{/if}

<style>
  .editor {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 10px;
  }

  .zone {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px;
    border-radius: var(--radius-m);
    border: 1px dashed var(--border-strong);
    background: var(--surface-2);
    transition: border-color var(--dur-fast) ease;

    &.over {
      border-color: var(--accent);
    }
    &.unused {
      grid-column: 1 / -1;
    }
  }

  .zone-label {
    color: var(--fg-muted);
    font-size: 12px;
    font-weight: 600;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    min-height: 32px;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 5px 10px;
    border-radius: 999px;
    border: 1px solid var(--border-strong);
    background: var(--surface);
    color: var(--fg);
    font-size: 12.5px;
    cursor: grab;
    touch-action: none;
    user-select: none;

    &.dragging {
      opacity: 0.35;
    }
    &.ghost {
      position: fixed;
      z-index: 100;
      pointer-events: none;
      box-shadow: var(--shadow-m);
      cursor: grabbing;
    }
  }

  .marker {
    width: 2px;
    height: 24px;
    border-radius: 1px;
    background: var(--accent);
  }

  .empty {
    color: var(--fg-disabled);
    font-size: 12px;
  }
</style>
