<script lang="ts" generics="T extends string">
  import Icon from "./Icon.svelte";

  interface Props {
    value: T;
    options: { value: T; label: string }[];
    onchange: (value: T) => void;
    disabled?: boolean;
  }
  let { value, options, onchange, disabled = false }: Props = $props();

  let open = $state(false);
  let root: HTMLDivElement | null = $state(null);
  const current = $derived(options.find((o) => o.value === value));

  function choose(v: T) {
    open = false;
    if (v !== value) onchange(v);
  }

  $effect(() => {
    if (!open) return;
    const onDown = (e: PointerEvent) => {
      if (root && !root.contains(e.target as Node)) open = false;
    };
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && (open = false);
    window.addEventListener("pointerdown", onDown);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("pointerdown", onDown);
      window.removeEventListener("keydown", onKey);
    };
  });
</script>

<div class="select" class:open bind:this={root}>
  <button type="button" class="select-trigger" {disabled} onclick={() => (open = !open)}>
    <span>{current?.label ?? value}</span>
    <Icon name="ChevronRight" size={14} class="select-chevron" />
  </button>
  {#if open}
    <div class="select-menu" role="listbox">
      {#each options as option (option.value)}
        <button
          type="button"
          role="option"
          aria-selected={option.value === value}
          class="select-option"
          class:selected={option.value === value}
          onclick={() => choose(option.value)}
        >
          {option.label}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .select {
    position: relative;
    min-width: 200px;
  }

  .select-trigger {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    width: 100%;
    height: 32px;
    padding: 0 10px 0 12px;
    border-radius: var(--radius-s);
    background: var(--surface);
    border: 1px solid var(--border-strong);
    font-weight: 600;
    font-size: 13px;
    transition: border-color var(--dur-fast) ease;

    &:hover:not(:disabled) {
      border-color: var(--accent);
    }
    &:disabled {
      opacity: 0.5;
      cursor: default;
    }

    :global(.select-chevron) {
      color: var(--fg-muted);
      transform: rotate(90deg);
      transition: transform var(--dur) var(--ease);
    }
  }

  .open .select-trigger {
    border-color: var(--accent);
    :global(.select-chevron) {
      transform: rotate(-90deg);
    }
  }

  .select-menu {
    position: absolute;
    top: calc(100% + 4px);
    right: 0;
    left: 0;
    z-index: 20;
    padding: 4px;
    border-radius: var(--radius-m);
    background: var(--surface);
    border: 1px solid var(--border);
    box-shadow: var(--shadow-l);
    backdrop-filter: blur(20px) saturate(1.4);
    animation: pop 140ms var(--ease);
  }

  :global(:root[data-glass]) .select-menu {
    background: color-mix(in srgb, var(--bg-solid, var(--surface)) 92%, transparent);
  }

  .select-option {
    display: block;
    width: 100%;
    padding: 7px 10px;
    border-radius: var(--radius-s);
    text-align: left;
    font-size: 13px;
    font-weight: 600;

    &:hover {
      background: var(--surface-hover);
    }
    &.selected {
      background: var(--accent-soft);
      color: var(--accent);
    }
  }

  @keyframes pop {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
  }
</style>
