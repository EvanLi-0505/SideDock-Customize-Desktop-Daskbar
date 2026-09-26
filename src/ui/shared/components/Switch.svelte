<script lang="ts">
  interface Props {
    checked: boolean;
    disabled?: boolean;
    onchange?: (value: boolean) => void;
    label?: string;
  }
  let { checked, disabled = false, onchange, label }: Props = $props();
</script>

<button
  type="button"
  role="switch"
  aria-checked={checked}
  aria-label={label}
  class="switch"
  class:checked
  {disabled}
  onclick={() => onchange?.(!checked)}
>
  <span class="thumb"></span>
</button>

<style>
  .switch {
    position: relative;
    flex-shrink: 0;
    width: 40px;
    height: 22px;
    border-radius: 11px;
    background: var(--surface-pressed);
    border: 1px solid var(--border-strong);
    transition:
      background-color var(--dur) var(--ease),
      border-color var(--dur) var(--ease);

    &.checked {
      background: var(--accent);
      border-color: var(--accent);
    }
    &:disabled {
      opacity: 0.5;
      cursor: default;
    }
  }

  .thumb {
    position: absolute;
    top: 50%;
    left: 3px;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--fg-secondary);
    transform: translateY(-50%);
    transition:
      left var(--dur) var(--ease),
      width var(--dur) var(--ease),
      background-color var(--dur) var(--ease);

    .checked & {
      left: 21px;
      background: #fff;
    }
    .switch:active:not(:disabled) & {
      width: 18px;
    }
  }
</style>
