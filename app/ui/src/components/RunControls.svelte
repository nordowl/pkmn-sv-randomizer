<script lang="ts">
  import { dismissable } from '../lib/actions';
  import { app, blockReason } from '../lib/app.svelte';
  import { plural, rollSeed } from '../lib/format';
  import { run } from '../lib/run.svelte';
  import BatchPanel from './BatchPanel.svelte';
  import Icon from './Icon.svelte';

  let batchOpen = $state(false);
  /** Digits shown while a rolled seed settles, so the roll reads as a roll. */
  let scramble = $state<string | null>(null);

  const running = $derived(run.phase.is === 'running');
  const block = $derived(app.block);
  const blocked = $derived(!!block && !running);
  const batchCount = $derived(app.batch.enabled ? app.batch.count : 0);
  const finished = $derived(run.done + run.failed);

  function roll() {
    const target = rollSeed();
    if (matchMedia('(prefers-reduced-motion: reduce)').matches) {
      app.seed = target;
      return;
    }
    const started = performance.now();
    const frame = (now: number) => {
      const t = (now - started) / 380;
      if (t >= 1) {
        scramble = null;
        app.seed = target;
        return;
      }
      const settled = Math.floor(t * t * target.length);
      const noise = Array.from({ length: target.length - settled }, () => Math.floor(Math.random() * 10)).join('');
      scramble = target.slice(0, settled) + noise;
      requestAnimationFrame(frame);
    };
    requestAnimationFrame(frame);
  }

  function press() {
    batchOpen = false;
    if (running) run.visible = true;
    else run.review();
  }

  const title = $derived.by(() => {
    if (running) return 'Show the run';
    if (block) return blockReason(block, app.catalog);
    return batchCount ? `Make ${plural(batchCount, 'seed')}` : 'Make one seed with these settings';
  });
</script>

<div class="run-controls">
  <div class="seed" class:rolling={scramble !== null}>
    <label class="caption" for="seed-input">Seed</label>
    <input
      id="seed-input"
      class="mono num"
      placeholder="Random"
      maxlength="64"
      spellcheck="false"
      autocomplete="off"
      title="The same seed and settings always make the same game. Leave it empty for a random one."
      value={scramble ?? app.seed}
      oninput={(event) => (app.seed = event.currentTarget.value)}
    />
    {#if app.seed && scramble === null}
      <button class="clear" aria-label="Clear the seed" title="Clear the seed" onclick={() => (app.seed = '')}>
        <Icon name="x" size={13} stroke={2} />
      </button>
    {/if}
    <button class="dice" aria-label="Roll a seed" title="Roll a seed" onclick={roll}>
      <Icon name="dice" size={18} />
    </button>
  </div>

  <div class="go" class:blocked use:dismissable={() => (batchOpen = false)}>
    <button class="randomize" disabled={blocked} {title} onclick={press}>
      {#if running}
        <Icon name="spinner" size={16} stroke={2.2} class="spin" />
        <span class="num">{finished} of {run.rows.length}</span>
      {:else}
        <Icon name="play" size={15} stroke={2} />
        <span>Randomize</span>
        {#if batchCount}<span class="count num">×{batchCount}</span>{/if}
      {/if}
    </button>
    <button
      class="more"
      aria-label="Batch options"
      title="Batch options"
      aria-expanded={batchOpen}
      disabled={running}
      onclick={() => (batchOpen = !batchOpen)}
    >
      <Icon name="chevronDown" size={16} stroke={2} />
    </button>
    {#if batchOpen}
      <div class="popover batch-pop">
        <BatchPanel />
      </div>
    {/if}
  </div>
</div>

<style>
  .run-controls {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .seed {
    display: flex;
    align-items: center;
    height: 40px;
    padding: 0 4px 0 12px;
    border-radius: 12px;
    background: var(--ink);
    box-shadow: inset 0 0 0 1px var(--line-2);
    transition: box-shadow 140ms var(--ease);
  }
  .seed:focus-within {
    box-shadow:
      inset 0 0 0 1px var(--accent),
      0 0 0 3px var(--accent-softer);
  }

  .caption {
    padding-right: 10px;
    margin-right: 10px;
    border-right: 1px solid var(--line-2);
    color: var(--text-3);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    cursor: text;
  }

  input {
    width: 124px;
    height: 100%;
    border: 0;
    outline: none;
    background: none;
    color: var(--text);
    font-size: 14px;
    letter-spacing: 0.02em;
  }
  input::placeholder {
    color: var(--text-3);
    font-family: var(--font-ui);
    letter-spacing: 0;
  }
  .rolling input {
    color: var(--accent-text);
  }

  .clear,
  .dice {
    display: grid;
    place-items: center;
    border-radius: 9px;
    color: var(--text-3);
    transition:
      color 140ms var(--ease),
      background-color 140ms var(--ease),
      scale 160ms var(--ease);
  }
  .clear {
    width: 24px;
    height: 24px;
  }
  .dice {
    width: 32px;
    height: 32px;
    margin-left: 2px;
  }
  .clear:hover,
  .dice:hover {
    color: var(--text);
    background: rgb(255 255 255 / 0.06);
  }
  .dice:active {
    scale: 0.92;
  }
  .rolling .dice {
    color: var(--accent-text);
  }

  .go {
    position: relative;
    display: flex;
    border-radius: 12px;
    box-shadow:
      0 0 0 1px color-mix(in oklab, var(--accent) 55%, black),
      0 8px 24px -10px color-mix(in oklab, var(--accent) 80%, transparent);
  }

  .randomize,
  .more {
    display: flex;
    align-items: center;
    height: 40px;
    background: var(--accent);
    color: var(--on-accent);
    box-shadow: inset 0 1px 0 rgb(255 255 255 / 0.28);
    transition:
      background-color 140ms var(--ease),
      opacity 140ms var(--ease),
      scale 160ms var(--ease);
  }
  .randomize {
    gap: 8px;
    min-width: 136px;
    justify-content: center;
    padding: 0 16px 0 14px;
    border-radius: 12px 0 0 12px;
    font-size: 14.5px;
    font-weight: 650;
    letter-spacing: -0.005em;
  }
  .more {
    justify-content: center;
    width: 34px;
    border-radius: 0 12px 12px 0;
    border-left: 1px solid color-mix(in oklab, var(--accent) 60%, black);
  }
  .randomize:hover:not(:disabled),
  .more:hover:not(:disabled) {
    background: color-mix(in oklab, var(--accent) 86%, white);
  }
  .randomize:active:not(:disabled) {
    scale: 0.97;
  }
  .randomize:disabled,
  .more:disabled {
    opacity: 0.45;
  }
  .go.blocked {
    box-shadow: none;
  }
  .go.blocked .more {
    background: color-mix(in oklab, var(--accent) 45%, var(--surface-3));
    border-left-color: transparent;
  }
  .go.blocked .randomize:disabled {
    opacity: 1;
    background: color-mix(in oklab, var(--accent) 45%, var(--surface-3));
    color: color-mix(in oklab, var(--on-accent) 70%, var(--accent));
    box-shadow: none;
  }

  .count {
    padding: 1px 6px;
    border-radius: 6px;
    background: rgb(0 0 0 / 0.16);
    font-size: 12.5px;
  }

  .batch-pop {
    top: calc(100% + 10px);
    right: 0;
    width: 380px;
  }
</style>
