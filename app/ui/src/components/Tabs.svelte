<script lang="ts">
  import Icon from './Icon.svelte';

  let {
    tabs,
    active,
    label,
    compact = false,
    onselect,
  }: {
    tabs: { label: string; changed: boolean; locked: boolean }[];
    active: number;
    label: string;
    compact?: boolean;
    onselect: (index: number) => void;
  } = $props();

  /** A lock on every tab says nothing the rows below do not already say. */
  const locks = $derived(!tabs.every((tab) => tab.locked));
</script>

<div class="tabs" class:compact role="tablist" aria-label={label}>
  {#each tabs as tab, index (tab.label)}
    <button
      role="tab"
      aria-selected={index === active}
      class:locked={locks && tab.locked}
      title={locks && tab.locked ? 'Locked by another setting' : undefined}
      onclick={() => onselect(index)}
    >
      {#if locks && tab.locked}<Icon name="lock" size={12} />{/if}
      {tab.label}
      {#if tab.changed}<span class="dot" title="Changed from the default"></span>{/if}
    </button>
  {/each}
</div>

<style>
  .tabs {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    border-bottom: 1px solid var(--line);
  }

  button {
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 36px;
    padding: 0 12px;
    color: var(--text-3);
    font-size: 13.5px;
    font-weight: 540;
    white-space: nowrap;
    transition: color 140ms var(--ease);
  }
  button:hover,
  button[aria-selected='true'] {
    color: var(--text);
  }
  button::after {
    content: '';
    position: absolute;
    left: 10px;
    right: 10px;
    bottom: -1px;
    height: 2px;
    border-radius: 2px;
    background: var(--accent);
    scale: 0 1;
    transition: scale 200ms var(--ease);
  }
  button[aria-selected='true']::after {
    scale: 1 1;
  }
  button.locked {
    color: var(--text-4);
  }
  button.locked[aria-selected='true'] {
    color: var(--text-2);
  }

  .compact {
    display: inline-flex;
    gap: 2px;
    padding: 3px;
    border: 0;
    border-radius: var(--r-m);
    background: var(--ink);
    box-shadow: inset 0 0 0 1px var(--line-2);
  }
  .compact button {
    height: 26px;
    padding: 0 10px;
    border-radius: var(--r-s);
    font-size: 12.5px;
    transition:
      background-color 140ms var(--ease),
      color 140ms var(--ease);
  }
  .compact button::after {
    content: none;
  }
  .compact button:hover:not([aria-selected='true']) {
    background: rgb(255 255 255 / 0.04);
  }
  .compact button[aria-selected='true'] {
    background: var(--surface-4);
    box-shadow:
      0 0 0 1px var(--line-2),
      0 1px 2px rgb(0 0 0 / 0.35);
  }

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
  }
</style>
