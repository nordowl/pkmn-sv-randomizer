<script lang="ts">
  import { fitNumber } from '../../lib/options';
  import Icon from '../Icon.svelte';

  let {
    value,
    min,
    max,
    step = 1,
    unit = '',
    label,
    disabled = false,
    onchange,
  }: {
    value: number;
    min: number;
    max: number;
    step?: number;
    unit?: string;
    label: string;
    disabled?: boolean;
    onchange: (value: number) => void;
  } = $props();

  let draft = $state<string | null>(null);
  const digits = $derived(Math.max(String(min).length, String(max).length, 2));
  const fit = (n: number) => fitNumber(n, { min, max, step, default: value });

  function nudge(by: number) {
    const next = fit(value + by);
    if (next !== value) onchange(next);
  }

  function commit() {
    if (draft === null) return;
    const parsed = Number.parseInt(draft, 10);
    draft = null;
    if (Number.isFinite(parsed)) {
      const next = fit(parsed);
      if (next !== value) onchange(next);
    }
  }

  function key(event: KeyboardEvent) {
    if (event.key === 'Enter') (event.currentTarget as HTMLInputElement).blur();
    else if (event.key === 'Escape') draft = null;
    else if (event.key === 'ArrowUp' || event.key === 'ArrowDown') {
      event.preventDefault();
      draft = null;
      nudge(event.key === 'ArrowUp' ? step : -step);
    }
  }
</script>

<div class="stepper" class:disabled>
  <button class="step" aria-label="Less" disabled={disabled || value <= min} onclick={() => nudge(-step)}>
    <Icon name="minus" size={14} />
  </button>
  <label class="value">
    <input
      class="num"
      inputmode="numeric"
      aria-label={label}
      value={draft ?? String(value)}
      style:width="{digits + 0.5}ch"
      {disabled}
      oninput={(event) => (draft = event.currentTarget.value)}
      onblur={commit}
      onkeydown={key}
    />
    {#if unit}<span class="unit">{unit}</span>{/if}
  </label>
  <button class="step" aria-label="More" disabled={disabled || value >= max} onclick={() => nudge(step)}>
    <Icon name="plus" size={14} />
  </button>
</div>

<style>
  .stepper {
    display: inline-flex;
    align-items: center;
    height: 34px;
    border-radius: var(--r-m);
    background: var(--ink);
    box-shadow: inset 0 0 0 1px var(--line-2);
    transition: box-shadow 140ms var(--ease);
  }
  .stepper:focus-within {
    box-shadow:
      inset 0 0 0 1px var(--accent),
      0 0 0 3px var(--accent-softer);
  }
  .stepper.disabled {
    opacity: 0.5;
  }

  .step {
    display: grid;
    place-items: center;
    width: 30px;
    height: 100%;
    color: var(--text-2);
    border-radius: var(--r-m);
    transition:
      color 120ms var(--ease),
      background-color 120ms var(--ease);
  }
  .step:hover:not(:disabled) {
    color: var(--text);
    background: rgb(255 255 255 / 0.05);
  }
  .step:disabled {
    color: var(--text-4);
  }

  .value {
    display: flex;
    align-items: baseline;
    justify-content: center;
    gap: 3px;
    padding: 0 2px;
    cursor: text;
  }

  input {
    min-width: 0;
    padding: 0;
    border: 0;
    background: none;
    outline: none;
    text-align: right;
    font: 560 14px/34px var(--font-ui);
  }

  .unit {
    color: var(--text-3);
    font-size: 12.5px;
    white-space: nowrap;
  }
</style>
