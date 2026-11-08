<script lang="ts">
  import { onMount } from 'svelte';
  import { EDITIONS, type Edition, type SeedEntry } from '../lib/api';
  import { app } from '../lib/app.svelte';
  import { EDITION_NAMES, plural, when } from '../lib/format';
  import { library } from '../lib/library.svelte';
  import Dialog from './Dialog.svelte';
  import Icon from './Icon.svelte';

  const FILTERS: (Edition | 'all')[] = ['all', ...EDITIONS];
  let filter = $state<Edition | 'all'>('all');

  onMount(() => {
    void library.refresh();
  });

  const seeds = $derived(library.seeds?.state === 'ready' ? library.seeds.value : []);
  const shown = $derived(filter === 'all' ? seeds : seeds.filter((seed) => seed.edition === filter));
  const env = $derived(app.env);

  function playBlock(edition: Edition): string | null {
    if (!env) return 'Checking for Eden…';
    if (!env.eden.exe.exists) return 'Eden was not found. Choose eden.exe in App settings.';
    const status = app.statusOf(edition);
    if (status?.state !== 'ready') return `${EDITION_NAMES[edition]} is not ready. See App settings.`;
    return null;
  }

  const activeFor = (edition: Edition) => seeds.find((seed) => seed.active && seed.edition === edition) ?? null;

  function activateBlock(seed: SeedEntry): string | null {
    if (!seed.edition) return 'This seed is not marked as Scarlet or Violet.';
    if (env && !env.eden.load_dir.path) return 'Eden’s mod folder is unknown. Set it in App settings.';
    return null;
  }
</script>

<article class="library">
  <header>
    <div>
      <h1>Library</h1>
      <p class="meta">
        {#if library.seeds?.state === 'ready'}{plural(seeds.length, 'seed')}<span class="sep">·</span>newest first{:else}Your seeds{/if}
      </p>
    </div>
    {#if env}
      <button class="btn ghost sm" onclick={() => library.open(env.library_dir)}>
        <Icon name="folder" size={14} />Open library folder
      </button>
    {/if}
  </header>

  <div class="games">
    {#each EDITIONS as edition (edition)}
      {@const active = activeFor(edition)}
      {@const blocked = playBlock(edition)}
      {@const status = app.statusOf(edition)}
      <section class="game" data-edition={edition}>
        <div class="game-head">
          <span class="swatch"></span>
          <h2>{EDITION_NAMES[edition]}</h2>
          {#if status?.state === 'ready'}<span class="tag ok status">Ready</span>{:else if status}<span class="tag test status">Not ready</span>{/if}
        </div>
        <p class="active-line">
          {#if active}
            Playing <strong>{active.name}</strong>
          {:else}
            No seed active. Eden loads the normal game.
          {/if}
        </p>
        {#if status?.mod_disabled}
          <p class="mod-off">
            <Icon name="alert" size={14} />
            Eden has the randomizer switched off for {EDITION_NAMES[edition]}. In Eden, right-click the game, open Properties, and tick “SV Randomizer” under Add-ons.
          </p>
        {/if}
        <div class="game-actions">
          <button class="btn sm primary" disabled={!!blocked || library.busy === `play:${edition}`} onclick={() => library.play(edition)}>
            <Icon name="play" size={13} stroke={2} />Play in Eden
          </button>
          <button
            class="btn sm ghost"
            disabled={!active || library.busy === `off:${edition}`}
            onclick={() => library.deactivate(edition)}
          >
            <Icon name="power" size={14} />Turn off randomizer
          </button>
        </div>
        {#if blocked}<p class="why">{blocked}</p>{/if}
      </section>
    {/each}
  </div>

  <div class="list-head">
    <div class="filters" role="group" aria-label="Show">
      {#each FILTERS as option (option)}
        <button aria-pressed={filter === option} onclick={() => (filter = option)}>
          {option === 'all' ? 'All' : EDITION_NAMES[option]}
        </button>
      {/each}
    </div>
  </div>

  {#if library.seeds?.state === 'failed'}
    <p class="state error">{library.seeds.error}</p>
  {:else if !library.seeds || library.seeds.state === 'loading'}
    <p class="state">Reading your seeds…</p>
  {:else if !shown.length}
    <div class="empty">
      <h2>{seeds.length ? `No ${EDITION_NAMES[filter as Edition]} seeds yet` : 'No seeds yet'}</h2>
      <p>Pick your options, then press Randomize. New seeds land here, ready to switch on.</p>
    </div>
  {:else}
    <ul class="seeds">
      {#each shown as seed (seed.path)}
        {@const blocked = activateBlock(seed)}
        <li class="seed" class:active={seed.active} data-edition={seed.edition ?? undefined}>
          <div class="seed-main">
            <div class="seed-title">
              <h3>{seed.name}</h3>
              {#if seed.active}
                <span class="active-mark"><span class="pulse"></span>Active in Eden</span>
              {/if}
            </div>
            <p class="seed-meta">
              <span class="edition">{seed.edition ? EDITION_NAMES[seed.edition] : 'Unknown game'}</span>
              <span>Seed <span class="mono">{seed.seed}</span></span>
              <span>{when(seed.created_unix)}</span>
              <span class="num">{plural(seed.file_count, 'file')}</span>
              {#if seed.group}<span class="group"><Icon name="folder" size={12} />{seed.group}</span>{/if}
            </p>
          </div>
          <div class="seed-actions">
            {#if !seed.active}
              <button
                class="btn sm"
                disabled={!!blocked || library.busy !== null}
                title={blocked ?? `Install ${seed.name} as the seed Eden loads`}
                onclick={() => library.activate(seed)}
              >
                {#if library.busy === seed.path}<Icon name="spinner" size={14} class="spin" />{/if}Activate
              </button>
            {/if}
            <button class="btn sm ghost icon" title="Open folder" aria-label="Open the folder of {seed.name}" onclick={() => library.open(seed.path)}>
              <Icon name="folder" size={15} />
            </button>
            <button
              class="btn sm ghost icon"
              title={seed.has_spoiler ? 'Open spoiler log' : 'This seed has no spoiler log'}
              aria-label="Open the spoiler log of {seed.name}"
              disabled={!seed.has_spoiler}
              onclick={() => library.openSpoiler(seed)}
            >
              <Icon name="file" size={15} />
            </button>
            <button class="btn sm ghost icon remove" title="Delete" aria-label="Delete {seed.name}" onclick={() => (library.pendingDelete = seed)}>
              <Icon name="trash" size={15} />
            </button>
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</article>

<Dialog open={!!library.pendingDelete} title="Delete {library.pendingDelete?.name ?? 'seed'}?" onclose={() => (library.pendingDelete = null)} width={460}>
  {#if library.pendingDelete}
    {@const seed = library.pendingDelete}
    <p>This deletes the seed folder and its {plural(seed.file_count, 'file')} from your disk. It cannot be undone.</p>
    {#if seed.active}
      <p class="spaced">It is the active seed. Eden keeps its installed copy until you switch seeds or turn the randomizer off.</p>
    {/if}
    <p class="path mono">{seed.path}</p>
  {/if}
  {#snippet footer()}
    <button class="btn" onclick={() => (library.pendingDelete = null)}>Keep it</button>
    <button class="btn solid-danger" onclick={() => library.confirmDelete()}>
      <Icon name="trash" size={15} />Delete seed
    </button>
  {/snippet}
</Dialog>

<style>
  .library {
    max-width: 920px;
  }

  header {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 16px;
    margin-bottom: 24px;
  }

  h1 {
    font: 620 30px/1.1 var(--font-display);
    font-stretch: 92%;
    letter-spacing: -0.02em;
  }

  .meta {
    margin-top: 8px;
    color: var(--text-3);
    font-size: 13px;
  }
  .sep {
    margin: 0 7px;
  }

  .games {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 14px;
    margin-bottom: 30px;
  }

  .game {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 18px 18px 16px;
    border-radius: var(--r-xl);
    background:
      radial-gradient(120% 140% at 0% 0%, color-mix(in oklab, var(--accent) 13%, transparent), transparent 55%),
      var(--surface-2);
    box-shadow: 0 0 0 1px var(--line);
    overflow: hidden;
  }

  .game-head {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .swatch {
    width: 10px;
    height: 10px;
    border-radius: 3px;
    rotate: 45deg;
    background: var(--accent);
    box-shadow: 0 0 12px var(--accent);
  }
  .game h2 {
    flex: 1;
    font: 620 20px/1 var(--font-display);
    font-stretch: 90%;
    letter-spacing: -0.01em;
  }

  .active-line {
    color: var(--text-2);
    font-size: 13.5px;
  }
  .active-line strong {
    color: var(--text);
    font-weight: 600;
  }

  .mod-off {
    display: flex;
    gap: 8px;
    padding: 9px 11px;
    border-radius: var(--r-m);
    background: color-mix(in oklab, var(--warn) 10%, transparent);
    color: color-mix(in oklab, var(--warn) 55%, white);
    font-size: 12.5px;
    line-height: 1.45;
  }
  .mod-off :global(svg) {
    margin-top: 1px;
    color: var(--warn);
  }

  .game-actions {
    display: flex;
    gap: 8px;
    margin-top: auto;
    padding-top: 4px;
  }

  .why {
    color: var(--text-3);
    font-size: 12px;
  }

  .list-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 10px;
  }

  .filters {
    display: inline-flex;
    gap: 2px;
    padding: 3px;
    border-radius: var(--r-m);
    background: var(--ink);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .filters button {
    height: 26px;
    padding: 0 12px;
    border-radius: var(--r-s);
    color: var(--text-3);
    font-size: 12.5px;
    font-weight: 540;
    transition:
      background-color 140ms var(--ease),
      color 140ms var(--ease);
  }
  .filters button[aria-pressed='true'] {
    background: var(--surface-4);
    color: var(--text);
  }

  .seeds {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .seed {
    position: relative;
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 13px 12px 13px 18px;
    border-radius: var(--r-l);
    background: var(--surface-2);
    box-shadow: 0 0 0 1px var(--line);
    transition: background-color 140ms var(--ease);
  }
  .seed::before {
    content: '';
    position: absolute;
    left: 0;
    top: 12px;
    bottom: 12px;
    width: 3px;
    border-radius: 0 3px 3px 0;
    background: var(--accent);
    opacity: 0.55;
  }
  .seed:hover {
    background: var(--surface-3);
  }
  .seed.active {
    background: color-mix(in oklab, var(--accent) 9%, var(--surface-2));
    box-shadow: 0 0 0 1px var(--accent-line);
  }
  .seed.active::before {
    opacity: 1;
  }

  .seed-main {
    flex: 1;
    min-width: 0;
  }

  .seed-title {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .seed-title h3 {
    overflow: hidden;
    font-size: 14.5px;
    font-weight: 600;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .active-mark {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 22px;
    padding: 0 9px;
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--accent-text);
    font-size: 12px;
    font-weight: 620;
    white-space: nowrap;
  }
  .pulse {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in oklab, var(--accent) 25%, transparent);
  }

  .seed-meta {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 14px;
    margin-top: 4px;
    color: var(--text-3);
    font-size: 12.5px;
  }
  .seed-meta .mono {
    color: var(--text-2);
  }
  .edition {
    color: var(--accent-text);
    font-weight: 560;
  }
  .group {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }

  .seed-actions {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .seed-actions .btn:first-child:not(.icon) {
    margin-right: 6px;
  }
  .remove:hover:not(:disabled) {
    color: color-mix(in oklab, var(--danger) 80%, white);
  }

  .state {
    padding: 20px 0;
    color: var(--text-3);
  }
  .state.error {
    color: color-mix(in oklab, var(--danger) 75%, white);
  }

  .empty {
    padding: 32px;
    border-radius: var(--r-xl);
    border: 1px dashed var(--line-2);
  }
  .empty h2 {
    font: 600 18px/1.3 var(--font-display);
  }
  .empty p {
    margin-top: 6px;
    color: var(--text-2);
  }

  .spaced {
    margin-top: 10px;
  }
  .path {
    margin-top: 12px;
    padding: 9px 11px;
    border-radius: var(--r-m);
    background: var(--ink);
    color: var(--text-2);
    font-size: 12px;
    overflow-wrap: anywhere;
  }
</style>
