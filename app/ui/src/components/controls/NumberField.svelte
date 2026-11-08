<script lang="ts">
  import type { NumberKind } from '../../lib/options';
  import { fitNumber } from '../../lib/options';
  import Stepper from './Stepper.svelte';

  let { kind, value, label, onchange }: { kind: NumberKind; value: number; label: string; onchange: (value: number) => void } =
    $props();

  const stops = $derived((kind.max - kind.min) / (kind.step > 0 ? kind.step : 1));
  const slider = $derived(stops >= 4 && stops <= 400);
  const fill = $derived(kind.max > kind.min ? ((value - kind.min) / (kind.max - kind.min)) * 100 : 0);
</script>

<div class="number">
  {#if slider}
    <input
      type="range"
      min={kind.min}
      max={kind.max}
      step={kind.step}
      {value}
      aria-label={label}
      style:--fill="{fill}%"
      oninput={(event) => onchange(fitNumber(Number(event.currentTarget.value), kind))}
    />
  {/if}
  <Stepper {value} min={kind.min} max={kind.max} step={kind.step} unit={kind.unit} {label} {onchange} />
</div>

<style>
  .number {
    display: flex;
    align-items: center;
    gap: 14px;
  }

  input[type='range'] {
    appearance: none;
    width: 150px;
    height: 22px;
    margin: 0;
    background: transparent;
    cursor: pointer;
  }
  input[type='range']::-webkit-slider-runnable-track {
    height: 4px;
    border-radius: 999px;
    background: linear-gradient(to right, var(--accent) var(--fill), var(--surface-4) var(--fill));
  }
  input[type='range']::-webkit-slider-thumb {
    appearance: none;
    width: 16px;
    height: 16px;
    margin-top: -6px;
    border-radius: 50%;
    background: #f4f1f8;
    box-shadow:
      0 0 0 1px rgb(0 0 0 / 0.3),
      0 1px 3px rgb(0 0 0 / 0.5);
    transition: scale 140ms var(--ease);
  }
  input[type='range']:active::-webkit-slider-thumb {
    scale: 1.12;
  }
  input[type='range']:focus-visible {
    outline: none;
  }
  input[type='range']:focus-visible::-webkit-slider-thumb {
    box-shadow:
      0 0 0 3px var(--accent-soft),
      0 0 0 1px var(--accent);
  }
</style>
