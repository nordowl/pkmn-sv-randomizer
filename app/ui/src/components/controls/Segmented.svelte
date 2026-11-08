<script lang="ts">
  import type { ChoiceDef } from '../../lib/api';

  let {
    choices,
    value,
    label,
    onchange,
  }: { choices: ChoiceDef[]; value: string; label: string; onchange: (value: string) => void } = $props();
</script>

<div class="segmented" role="group" aria-label={label}>
  {#each choices as choice (choice.value)}
    <button aria-pressed={choice.value === value} title={choice.help || undefined} onclick={() => onchange(choice.value)}>
      {choice.label}
    </button>
  {/each}
</div>

<style>
  .segmented {
    display: inline-flex;
    flex-wrap: wrap;
    gap: 2px;
    padding: 3px;
    border-radius: var(--r-m);
    background: var(--ink);
    box-shadow: inset 0 0 0 1px var(--line-2);
  }

  button {
    height: 28px;
    padding: 0 12px;
    border-radius: var(--r-s);
    color: var(--text-2);
    font-size: 13px;
    font-weight: 520;
    white-space: nowrap;
    transition:
      background-color 140ms var(--ease),
      color 140ms var(--ease),
      box-shadow 140ms var(--ease);
  }
  button:hover:not([aria-pressed='true']) {
    color: var(--text);
    background: rgb(255 255 255 / 0.04);
  }
  button[aria-pressed='true'] {
    background: var(--surface-4);
    color: var(--text);
    box-shadow:
      0 0 0 1px var(--line-2),
      0 1px 2px rgb(0 0 0 / 0.35);
  }
</style>
