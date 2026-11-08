<script lang="ts">
  import { EDITIONS, type Edition } from '../lib/api';
  import { app } from '../lib/app.svelte';
  import { EDITION_NAMES } from '../lib/format';
  import Icon from './Icon.svelte';

  function stateOf(edition: Edition): 'checking' | 'ready' | 'error' | 'missing' {
    if (!app.env) return app.envError ? 'error' : 'checking';
    return app.statusOf(edition)?.state ?? 'missing';
  }

  function describe(edition: Edition): string {
    const status = app.statusOf(edition);
    if (!app.env) return app.envError ?? 'Checking your games…';
    if (status?.state === 'ready') return `${EDITION_NAMES[edition]} is ready to randomize`;
    return status?.message ?? `${EDITION_NAMES[edition]} was not found`;
  }
</script>

<div class="picker" role="group" aria-label="Game">
  {#each EDITIONS as edition (edition)}
    {@const state = stateOf(edition)}
    <button
      data-edition={edition}
      aria-pressed={app.edition === edition}
      title={describe(edition)}
      onclick={() => (app.edition = edition)}
    >
      <span class="swatch"></span>
      <span class="name">{EDITION_NAMES[edition]}</span>
      <span class="state {state}" aria-label={state === 'ready' ? 'Ready' : state === 'checking' ? 'Checking' : 'Needs attention'}>
        {#if state === 'ready'}
          <Icon name="check" size={13} stroke={2.4} />
        {:else if state === 'checking'}
          <Icon name="spinner" size={13} stroke={2.2} class="spin" />
        {:else}
          <Icon name="alert" size={13} stroke={2.1} />
        {/if}
      </span>
    </button>
  {/each}
</div>

<style>
  .picker {
    display: inline-flex;
    gap: 2px;
    padding: 3px;
    border-radius: 13px;
    background: var(--ink);
    box-shadow: inset 0 0 0 1px var(--line);
  }

  button {
    display: flex;
    align-items: center;
    gap: 9px;
    height: 34px;
    padding: 0 10px 0 12px;
    border-radius: 10px;
    color: var(--text-3);
    transition:
      background-color 200ms var(--ease),
      color 200ms var(--ease),
      box-shadow 200ms var(--ease);
  }
  button:hover:not([aria-pressed='true']) {
    color: var(--text-2);
    background: rgb(255 255 255 / 0.03);
  }
  button[aria-pressed='true'] {
    background: var(--accent-soft);
    color: var(--text);
    box-shadow: inset 0 0 0 1px var(--accent-line);
  }

  .swatch {
    width: 9px;
    height: 9px;
    border-radius: 3px;
    rotate: 45deg;
    background: var(--accent);
    opacity: 0.55;
    transition: opacity 200ms var(--ease);
  }
  button[aria-pressed='true'] .swatch {
    opacity: 1;
    box-shadow: 0 0 10px var(--accent);
  }

  .name {
    font: 600 15px/1 var(--font-display);
    font-stretch: 90%;
    letter-spacing: -0.01em;
  }

  .state {
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    border-radius: 50%;
  }
  .state.ready {
    color: var(--ok);
    background: color-mix(in oklab, var(--ok) 14%, transparent);
  }
  .state.error {
    color: var(--warn);
    background: color-mix(in oklab, var(--warn) 14%, transparent);
  }
  .state.missing {
    color: var(--text-3);
    background: rgb(255 255 255 / 0.06);
  }
  .state.checking {
    color: var(--text-3);
  }
</style>
