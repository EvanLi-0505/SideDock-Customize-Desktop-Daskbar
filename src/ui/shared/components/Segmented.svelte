<script lang="ts" generics="T extends string">
  import Icon from "./Icon.svelte";

  interface Props {
    value: T;
    options: { value: T; label?: string; icon?: string; title?: string }[];
    onchange: (value: T) => void;
  }
  let { value, options, onchange }: Props = $props();
</script>

<div class="segmented" role="radiogroup">
  {#each options as option (option.value)}
    <button
      type="button"
      role="radio"
      aria-checked={option.value === value}
      title={option.title ?? option.label}
      class:active={option.value === value}
      onclick={() => option.value !== value && onchange(option.value)}
    >
      {#if option.icon}<Icon name={option.icon} size={16} />{/if}
      {#if option.label}<span>{option.label}</span>{/if}
    </button>
  {/each}
</div>

<style>
  .segmented {
    display: inline-flex;
    padding: 2px;
    gap: 2px;
    border-radius: var(--radius-s);
    background: var(--surface-2);
    border: 1px solid var(--border-strong);

    button {
      display: flex;
      align-items: center;
      gap: 6px;
      height: 28px;
      padding: 0 10px;
      border-radius: 5px;
      font-weight: 600;
      font-size: 13px;
      color: var(--fg-secondary);
      transition:
        background-color var(--dur-fast) ease,
        color var(--dur-fast) ease;

      &:hover:not(.active) {
        background: var(--surface-hover);
      }
      &.active {
        background: var(--accent);
        color: var(--accent-contrast);
      }
    }
  }
</style>
