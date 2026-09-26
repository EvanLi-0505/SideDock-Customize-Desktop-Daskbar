<script lang="ts">
  interface Props {
    value: number;
    min?: number;
    max?: number;
    step?: number;
    disabled?: boolean;
    onchange: (value: number) => void;
  }
  let { value, min = 0, max = 10000, step = 1, disabled = false, onchange }: Props = $props();

  function commit(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const parsed = Number(input.value);
    if (!Number.isFinite(parsed)) {
      input.value = String(value);
      return;
    }
    const clamped = Math.min(max, Math.max(min, Math.round(parsed / step) * step));
    input.value = String(clamped);
    if (clamped !== value) onchange(clamped);
  }
</script>

<input
  class="number-input"
  type="number"
  {min}
  {max}
  {step}
  {disabled}
  {value}
  onchange={commit}
  onkeydown={(e) => e.key === "Enter" && (e.currentTarget as HTMLInputElement).blur()}
/>

<style>
  .number-input {
    width: 120px;
    height: 32px;
    padding: 0 10px;
    border-radius: var(--radius-s);
    border: 1px solid var(--border-strong);
    background: var(--surface);
    font-weight: 600;
    font-size: 13px;
    user-select: text;
    outline: none;
    transition: border-color var(--dur-fast) ease;

    &:hover:not(:disabled),
    &:focus {
      border-color: var(--accent);
    }
    &:disabled {
      opacity: 0.5;
    }
  }
</style>
