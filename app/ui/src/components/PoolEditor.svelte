<script lang="ts">
  import type { OptionDef, PoolFilter } from '../lib/api';
  import { app } from '../lib/app.svelte';
  import { count as formatCount, EDITION_NAMES } from '../lib/format';
  import { DEX_LABELS, GENERATION_COUNT, GLOBAL_POOL, STAGE_LABELS } from '../lib/options';
  import Icon from './Icon.svelte';
  import Toggle from './controls/Toggle.svelte';
  import SpeciesList from './species/SpeciesList.svelte';

  let { def, value, onchange }: { def: OptionDef; value: PoolFilter; onchange: (value: PoolFilter) => void } = $props();

  const isGlobal = $derived(def.id === GLOBAL_POOL);
  const own = $derived(isGlobal || !value.inherit);
  const gens = Array.from({ length: GENERATION_COUNT }, (_, i) => i);

  const counts = $derived(app.poolCounts);
  const known = $derived(counts?.value ?? null);
  const count = $derived(known?.counts[def.id]);
  const share = $derived(known && count !== undefined && known.total ? count / known.total : 0);

  const set = (patch: Partial<PoolFilter>) => onchange({ ...value, ...patch });
  const flip = (list: boolean[], index: number) => list.map((on, i) => (i === index ? !on : on));

  function useOwnPool(on: boolean) {
    if (on) {
      const global = app.settings.values[GLOBAL_POOL] as PoolFilter | undefined;
      onchange({ ...structuredClone(global ?? value), inherit: false });
    } else {
      const fallback = app.catalog?.defaultOf(def.id) as PoolFilter | undefined;
      onchange(fallback ?? { ...value, inherit: true });
    }
  }

  const blockedFor = (list: 'banned' | 'allowed') => {
    const blocked = new Map<number, string>();
    for (const n of value.banned) blocked.set(n, list === 'banned' ? 'Already banned' : 'In the ban list');
    for (const n of value.allowed) blocked.set(n, list === 'allowed' ? 'Already allowed' : 'In the allow list');
    return blocked;
  };
</script>

<div class="pool" class:inherits={!own}>
  <div class="head">
    <div class="tally" class:empty={count === 0} class:stale={counts?.state === 'loading'}>
      {#if count !== undefined && known}
        <span class="big num">{formatCount(count)}</span>
        <span class="of">of {formatCount(known.total)} forms</span>
        <span class="meter" aria-hidden="true"><span style:scale="{share} 1"></span></span>
      {:else if !app.gameReady}
        <span class="of">The species count shows once {EDITION_NAMES[app.edition]} is ready.</span>
      {:else if counts?.state === 'failed'}
        <span class="of">Could not count species: {counts.error}</span>
      {:else}
        <span class="of">Counting species…</span>
      {/if}
    </div>
    {#if !isGlobal}
      <label class="own">
        <span>Use its own pool</span>
        <Toggle checked={own} label="Use its own pool for {def.label}" onchange={useOwnPool} />
      </label>
    {/if}
  </div>

  {#if count === 0}
    <p class="warning">
      <Icon name="alert" size={15} />
      Nothing is left in this pool, so Randomize stays off. Loosen the filters below or shorten the ban list.
    </p>
  {/if}

  {#if !own}
    <p class="inherit-note">
      Uses the global pool.
      {#if app.view.kind !== 'page' || app.view.id !== app.catalog?.byId.get(GLOBAL_POOL)?.page}
        <button class="link" onclick={() => app.reveal(GLOBAL_POOL)}>Edit the global pool</button>
      {/if}
    </p>
  {:else}
    <section class="gens">
      <div class="section-head">
        <h4>Generations</h4>
        <p>Untick a generation to leave its Pokémon out. Legendaries covers legendary, mythical and Ultra Beast Pokémon.</p>
      </div>
      <div class="matrix" role="group" aria-label="Generations">
        <span></span>
        {#each gens as gen (gen)}<span class="col num">Gen {gen + 1}</span>{/each}
        <span class="row-label">Pokémon</span>
        {#each gens as gen (gen)}
          <button
            class="cell"
            aria-pressed={value.generations[gen]}
            aria-label="Gen {gen + 1} Pokémon"
            onclick={() => set({ generations: flip(value.generations, gen) })}
          >
            <Icon name="check" size={15} stroke={2.2} />
          </button>
        {/each}
        <span class="row-label">Legendaries</span>
        {#each gens as gen (gen)}
          <button
            class="cell"
            aria-pressed={value.legendaries[gen] && value.generations[gen]}
            aria-label="Gen {gen + 1} legendaries"
            disabled={!value.generations[gen]}
            title={value.generations[gen] ? undefined : `Gen ${gen + 1} is left out entirely`}
            onclick={() => set({ legendaries: flip(value.legendaries, gen) })}
          >
            <Icon name="check" size={15} stroke={2.2} />
          </button>
        {/each}
      </div>
    </section>

    <div class="trio">
      <section>
        <div class="section-head">
          <h4>Evolution stages</h4>
          <p>Single-stage Pokémon never evolve.</p>
        </div>
        <div class="chips">
          {#each STAGE_LABELS as stage, i (stage)}
            <button class="chip" aria-pressed={value.stages[i]} onclick={() => set({ stages: flip(value.stages, i) })}>
              <span class="box"><Icon name="check" size={11} stroke={2.6} /></span>{stage}
            </button>
          {/each}
        </div>
      </section>

      <section>
        <div class="section-head">
          <h4>Regional dex</h4>
          <p>Tick a dex to use only Pokémon listed in it. With none ticked, there is no limit.</p>
        </div>
        <div class="chips">
          {#each DEX_LABELS as dex, i (dex)}
            <button class="chip" aria-pressed={value.regional_dex[i]} onclick={() => set({ regional_dex: flip(value.regional_dex, i) })}>
              <span class="box"><Icon name="check" size={11} stroke={2.6} /></span>{dex}
            </button>
          {/each}
        </div>
      </section>

      <section>
        <div class="section-head">
          <h4>Kinds</h4>
        </div>
        <div class="switches">
          <label>
            <span>Paradox Pokémon</span>
            <Toggle checked={value.paradox} label="Paradox Pokémon" onchange={(on) => set({ paradox: on })} />
          </label>
          <label>
            <span>Alternate forms</span>
            <Toggle checked={value.alt_forms} label="Alternate forms" onchange={(on) => set({ alt_forms: on })} />
          </label>
          <label>
            <span>Irregular forms</span>
            <Toggle checked={value.irregular_forms} label="Irregular forms" onchange={(on) => set({ irregular_forms: on })} />
          </label>
        </div>
      </section>
    </div>

    <div class="lists">
      <SpeciesList
        title="Ban list"
        help="Never used, whatever the filters above say."
        list={value.banned}
        blocked={blockedFor('banned')}
        onchange={(banned) => set({ banned })}
      />
      <SpeciesList
        title="Allow list"
        help="Always allowed, even when a filter above leaves them out."
        list={value.allowed}
        blocked={blockedFor('allowed')}
        onchange={(allowed) => set({ allowed })}
      />
    </div>
  {/if}
</div>

<style>
  .pool {
    display: flex;
    flex-direction: column;
    gap: 22px;
    margin-top: 14px;
    padding: 20px 22px 22px;
    border-radius: var(--r-xl);
    background: var(--surface-2);
    box-shadow: 0 0 0 1px var(--line);
  }
  .pool.inherits {
    gap: 10px;
    padding: 16px 20px;
  }

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }

  .tally {
    display: grid;
    grid-template-columns: auto 1fr;
    align-items: baseline;
    column-gap: 10px;
    row-gap: 8px;
    min-width: 260px;
    transition: opacity 160ms var(--ease);
  }
  .tally.stale {
    opacity: 0.6;
  }

  .big {
    font: 640 34px/1 var(--font-display);
    font-stretch: 88%;
    letter-spacing: -0.02em;
    color: var(--text);
  }
  .tally.empty .big {
    color: var(--danger);
  }

  .of {
    color: var(--text-2);
    font-size: 13.5px;
  }

  .meter {
    grid-column: 1 / -1;
    height: 4px;
    border-radius: 999px;
    background: var(--surface-4);
    overflow: hidden;
  }
  .meter span {
    display: block;
    height: 100%;
    background: linear-gradient(90deg, var(--accent), var(--accent-2));
    transform-origin: left center;
    transition: scale 360ms var(--ease-out);
  }

  .own {
    display: flex;
    align-items: center;
    gap: 12px;
    color: var(--text-2);
    font-size: 13px;
    cursor: pointer;
  }

  .warning {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px;
    border-radius: var(--r-m);
    background: color-mix(in oklab, var(--danger) 12%, transparent);
    color: color-mix(in oklab, var(--danger) 70%, white);
    font-size: 13px;
  }

  .inherit-note {
    color: var(--text-2);
    font-size: 13px;
  }

  .section-head {
    margin-bottom: 12px;
  }
  .section-head h4 {
    margin: 0 0 3px;
    font-size: 13px;
    font-weight: 600;
  }
  .section-head p {
    color: var(--text-3);
    font-size: 12.5px;
    max-width: 60ch;
  }

  .matrix {
    display: grid;
    grid-template-columns: 96px repeat(9, minmax(34px, 1fr));
    align-items: center;
    gap: 6px;
  }
  .col {
    color: var(--text-3);
    font-size: 11.5px;
    text-align: center;
  }
  .row-label {
    color: var(--text-2);
    font-size: 13px;
  }

  .cell {
    display: grid;
    place-items: center;
    height: 32px;
    border-radius: var(--r-s);
    background: transparent;
    box-shadow: inset 0 0 0 1px var(--line-2);
    color: transparent;
    transition:
      background-color 140ms var(--ease),
      color 140ms var(--ease),
      box-shadow 140ms var(--ease),
      scale 140ms var(--ease);
  }
  .cell:hover:not(:disabled) {
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.22);
  }
  .cell:active:not(:disabled) {
    scale: 0.94;
  }
  .cell[aria-pressed='true'] {
    background: var(--accent-softer);
    box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--accent) 34%, transparent);
    color: var(--accent-text);
  }
  .cell:disabled {
    opacity: 0.3;
    box-shadow: inset 0 0 0 1px var(--line);
    background-image: repeating-linear-gradient(135deg, transparent 0 5px, rgb(255 255 255 / 0.04) 5px 6px);
  }

  .trio {
    display: grid;
    grid-template-columns: 1.15fr 1fr 0.9fr;
    gap: 28px;
    padding-top: 20px;
    border-top: 1px solid var(--line);
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 30px;
    padding: 0 12px 0 9px;
    border-radius: 999px;
    background: transparent;
    box-shadow: inset 0 0 0 1px var(--line-2);
    color: var(--text-2);
    font-size: 13px;
    font-weight: 520;
    transition:
      background-color 140ms var(--ease),
      color 140ms var(--ease),
      box-shadow 140ms var(--ease),
      scale 140ms var(--ease);
  }
  .chip:hover {
    color: var(--text);
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.2);
  }
  .chip:active {
    scale: 0.96;
  }
  .chip[aria-pressed='true'] {
    background: var(--accent-softer);
    box-shadow: inset 0 0 0 1px var(--accent-line);
    color: var(--text);
  }

  .box {
    display: grid;
    place-items: center;
    width: 15px;
    height: 15px;
    border-radius: 5px;
    box-shadow: inset 0 0 0 1.5px var(--text-4);
    color: transparent;
    transition:
      background-color 140ms var(--ease),
      box-shadow 140ms var(--ease),
      color 140ms var(--ease);
  }
  .chip[aria-pressed='true'] .box {
    background: var(--accent);
    box-shadow: none;
    color: var(--on-accent);
  }

  .switches {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .switches label {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    color: var(--text-2);
    font-size: 13px;
    cursor: pointer;
  }

  .lists {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 28px;
    padding-top: 20px;
    border-top: 1px solid var(--line);
  }
</style>
