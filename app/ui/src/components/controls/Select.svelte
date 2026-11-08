<script lang="ts">
  import type { ChoiceDef } from '../../lib/api';
  import Icon from '../Icon.svelte';

  let {
    choices,
    value,
    label,
    onchange,
  }: { choices: ChoiceDef[]; value: string; label: string; onchange: (value: string) => void } = $props();
</script>

<div class="select">
  <select class="field" {value} aria-label={label} onchange={(event) => onchange(event.currentTarget.value)}>
    {#each choices as choice (choice.value)}
      <option value={choice.value}>{choice.label}</option>
    {/each}
  </select>
  <Icon name="chevronDown" size={15} class="chevron" />
</div>

<style>
  .select {
    position: relative;
    display: inline-flex;
  }

  select {
    appearance: none;
    min-width: 200px;
    padding-right: 34px;
    cursor: pointer;
  }

  option {
    background: var(--base-2);
  }

  .select :global(.chevron) {
    position: absolute;
    right: 11px;
    top: 50%;
    translate: 0 -50%;
    color: var(--text-3);
    pointer-events: none;
  }
</style>
