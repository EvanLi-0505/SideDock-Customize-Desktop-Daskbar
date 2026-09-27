<script lang="ts">
  interface Props {
    value: number;
    min: number;
    max: number;
    step: number;
    /** how the current value is shown next to the track */
    format?: (value: number) => string;
    disabled?: boolean;
    onchange: (value: number) => void;
  }
  let { value, min, max, step, format = String, disabled = false, onchange }: Props = $props();

  // live preview while dragging, commit on release
  let draft = $state<number | null>(null);
  const shown = $derived(draft ?? value);
  const decimals = $derived((String(step).split(".")[1] ?? "").length);
  const percent = $derived(((shown - min) / (max - min)) * 100);

  function read(e: Event) {
    return Number(Number((e.currentTarget as HTMLInputElement).value).toFixed(decimals));
  }
</script>

<div class="slider" class:disabled>
  <input
    type="range"
    {min}
    {max}
    {step}
    {disabled}
    value={shown}
    style:--percent="{percent}%"
    oninput={(e) => (draft = read(e))}
    onchange={(e) => {
      const v = read(e);
      draft = null;
      if (v !== value) onchange(v);
    }}
  />
  <span class="value">{format(shown)}</span>
</div>

<style>
  .slider {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 240px;

    &.disabled {
      opacity: 0.5;
    }
  }

  input {
    flex: 1;
    height: 4px;
    appearance: none;
    border-radius: 2px;
    background: linear-gradient(
      to right,
      var(--accent) var(--percent),
      var(--surface-pressed) var(--percent)
    );
    cursor: pointer;

    &::-webkit-slider-thumb {
      appearance: none;
      width: 16px;
      height: 16px;
      border-radius: 50%;
      background: #fff;
      border: 2px solid var(--accent);
      box-shadow: var(--shadow-s);
      transition: transform var(--dur-fast) ease;
    }
    &:active::-webkit-slider-thumb {
      transform: scale(1.15);
    }
    &:disabled {
      cursor: default;
    }
  }

  .value {
    min-width: 42px;
    text-align: right;
    font-weight: 700;
    font-size: 13px;
    font-variant-numeric: tabular-nums;
  }
</style>
