<script lang="ts">
  import type { ChoiceDef } from '../../lib/api';

  let {
    choices,
    value,
    name,
    label,
    onchange,
  }: { choices: ChoiceDef[]; value: string; name: string; label: string; onchange: (value: string) => void } = $props();
</script>

<div class="cards" role="radiogroup" aria-label={label}>
  {#each choices as choice (choice.value)}
    <label class="card" class:selected={choice.value === value}>
      <input type="radio" {name} value={choice.value} checked={choice.value === value} onchange={() => onchange(choice.value)} />
      <span class="title">{choice.label}</span>
      {#if choice.help}<span class="desc">{choice.help}</span>{/if}
    </label>
  {/each}
</div>

<style>
  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(190px, 1fr));
    gap: 8px;
  }

  .card {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 11px 13px 12px;
    border-radius: var(--r-m);
    background: var(--surface-2);
    box-shadow: inset 0 0 0 1px var(--line);
    cursor: pointer;
    transition:
      background-color 140ms var(--ease),
      box-shadow 140ms var(--ease);
  }
  .card:hover:not(.selected) {
    background: var(--surface-3);
    box-shadow: inset 0 0 0 1px var(--line-2);
  }
  .card.selected {
    background: var(--accent-softer);
    box-shadow: inset 0 0 0 1.5px var(--accent-line);
  }
  .card:has(input:focus-visible) {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  input {
    position: absolute;
    opacity: 0;
    pointer-events: none;
  }

  .title {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text);
    font-size: 13.5px;
    font-weight: 600;
  }
  .title::before {
    content: '';
    flex: none;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    box-shadow: inset 0 0 0 1.5px var(--text-4);
    transition: box-shadow 140ms var(--ease);
  }
  .selected .title::before {
    box-shadow:
      inset 0 0 0 1.5px var(--accent),
      inset 0 0 0 4px var(--surface-2),
      inset 0 0 0 7px var(--accent);
  }

  .desc {
    color: var(--text-3);
    font-size: 12.5px;
    line-height: 1.45;
  }
  .selected .desc {
    color: var(--text-2);
  }
</style>
