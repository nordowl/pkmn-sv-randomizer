<script lang="ts">
  import type { OptionDef, OptValue, PoolFilter, SpeciesPick } from '../lib/api';
  import { app } from '../lib/app.svelte';
  import { chosen, firstUnmet, sameValue } from '../lib/options';
  import Icon from './Icon.svelte';
  import PoolEditor from './PoolEditor.svelte';
  import ChoiceCards from './controls/ChoiceCards.svelte';
  import NumberField from './controls/NumberField.svelte';
  import Segmented from './controls/Segmented.svelte';
  import Select from './controls/Select.svelte';
  import TextField from './controls/TextField.svelte';
  import Toggle from './controls/Toggle.svelte';
  import SpeciesPicker from './species/SpeciesPicker.svelte';

  let { def }: { def: OptionDef } = $props();

  /** Segmented controls this long move under the text instead of squeezing it. */
  const LONG_SEGMENTS = 34;

  const kind = $derived(def.kind);
  const value = $derived(app.settings.values[def.id]);
  const changed = $derived(app.catalog ? !sameValue(value, app.catalog.defaultOf(def.id)) : false);
  const unmet = $derived(app.catalog ? firstUnmet(def, app.catalog, app.settings.values) : null);
  /** Short choice lists that explain themselves show as cards, so every description is visible at
   * once. Longer lists, like the Poké Balls, stay a dropdown. */
  const MAX_CARDS = 8;
  const cards = $derived(kind.type === 'choice' && kind.choices.length <= MAX_CARDS && kind.choices.some((c) => c.help));
  const picked = $derived(kind.type === 'choice' && !cards ? chosen(kind, value) : undefined);

  const layout = $derived.by(() => {
    switch (kind.type) {
      case 'pool':
        return 'block';
      case 'text':
        return kind.max_len > 60 ? 'stacked' : 'inline';
      case 'choice':
        if (cards) return 'block';
        return kind.choices.length <= 4 && kind.choices.reduce((n, c) => n + c.label.length, 0) > LONG_SEGMENTS
          ? 'stacked'
          : 'inline';
      default:
        return 'inline';
    }
  });

  const set = (next: OptValue) => app.setValue(def.id, next);

  let element: HTMLElement | undefined = $state();
  let flashing = $state(false);

  $effect(() => {
    const highlight = app.highlight;
    if (highlight?.id !== def.id || Date.now() - highlight.at > 1500 || !element) return;
    element.scrollIntoView({ block: 'center', behavior: matchMedia('(prefers-reduced-motion: reduce)').matches ? 'auto' : 'smooth' });
    flashing = true;
    const timer = setTimeout(() => (flashing = false), 1600);
    return () => clearTimeout(timer);
  });
</script>

<div class="row {layout}" class:off={!!unmet} class:flash={flashing} bind:this={element} id="option-{def.id}">
  <div class="text">
    <div class="head">
      {#if changed}<span class="changed" title="Changed from the default"></span>{/if}
      <h3 class="label">{def.label}</h3>
    </div>
    {#if def.help}<p class="help">{def.help}</p>{/if}
    {#if picked?.help}
      <p class="choice-help"><span>{picked.label}</span> {picked.help}</p>
    {/if}
    {#if unmet}
      <p class="reason">
        <Icon name="lock" size={13} />
        <span>
          {unmet.prefix}{#if unmet.target}<button class="link" onclick={() => app.reveal(unmet.target!.id)}>{unmet.targetText}</button>{:else}{unmet.targetText}{/if}{unmet.suffix}.
        </span>
      </p>
    {/if}
  </div>

  <div class="control" inert={!!unmet}>
    {#if kind.type === 'toggle'}
      <Toggle checked={value === true} label={def.label} onchange={set} />
    {:else if kind.type === 'choice'}
      {#if cards}
        <ChoiceCards choices={kind.choices} value={String(value)} name={def.id} label={def.label} onchange={set} />
      {:else if kind.choices.length <= 4}
        <Segmented choices={kind.choices} value={String(value)} label={def.label} onchange={set} />
      {:else}
        <Select choices={kind.choices} value={String(value)} label={def.label} onchange={set} />
      {/if}
    {:else if kind.type === 'number'}
      <NumberField {kind} value={typeof value === 'number' ? value : kind.default} label={def.label} onchange={set} />
    {:else if kind.type === 'text'}
      <TextField
        value={typeof value === 'string' ? value : kind.default}
        max={kind.max_len}
        multiline={kind.max_len > 60}
        label={def.label}
        onchange={set}
      />
    {:else if kind.type === 'species'}
      <SpeciesPicker value={value as SpeciesPick} label={def.label} onchange={set} />
    {:else if kind.type === 'pool'}
      <PoolEditor {def} value={value as PoolFilter} onchange={set} />
    {/if}
  </div>
</div>

<style>
  .row {
    position: relative;
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    justify-content: space-between;
    column-gap: 32px;
    row-gap: 12px;
    padding: 18px 16px;
    margin: 0 -16px;
    border-radius: var(--r-l);
    transition: background-color 600ms var(--ease);
  }
  .row + :global(.row)::before {
    content: '';
    position: absolute;
    top: 0;
    left: 16px;
    right: 16px;
    height: 1px;
    background: var(--line);
  }
  .row.flash {
    background: var(--accent-soft);
    transition-duration: 120ms;
  }

  .text {
    flex: 1 1 320px;
    min-width: 0;
    max-width: 620px;
  }
  .stacked .text,
  .block .text {
    flex-basis: 100%;
  }

  .control {
    flex: none;
    display: flex;
    align-items: center;
    min-height: 34px;
  }
  .inline .control {
    margin-top: -4px;
  }
  .stacked .control,
  .block .control {
    flex-basis: 100%;
    margin-top: -2px;
  }
  .block .control {
    display: block;
  }

  .head {
    position: relative;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 8px;
  }

  .label {
    margin-right: 2px;
    font-size: 14.5px;
    font-weight: 600;
    letter-spacing: -0.005em;
  }

  .changed {
    position: absolute;
    left: -13px;
    top: 50%;
    width: 6px;
    height: 6px;
    margin-top: -3px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-softer);
  }

  .help {
    margin-top: 5px;
    color: var(--text-2);
    font-size: 13px;
    line-height: 1.5;
  }

  .choice-help {
    margin-top: 6px;
    color: var(--text-3);
    font-size: 12.5px;
  }
  .choice-help span {
    color: var(--text-2);
    font-weight: 560;
  }

  .reason {
    display: flex;
    align-items: center;
    gap: 7px;
    margin-top: 8px;
    color: var(--text-2);
    font-size: 12.5px;
  }
  .reason :global(svg) {
    color: var(--text-3);
  }

  .off .label,
  .off .help,
  .off .choice-help,
  .off .control {
    opacity: 0.42;
  }
  .off .control {
    filter: saturate(0.4);
  }
</style>
