<script lang="ts">
  import { EDITION_NAMES } from '../lib/format';
  import { library } from '../lib/library.svelte';
  import Dialog from './Dialog.svelte';
  import Icon from './Icon.svelte';

  const pending = $derived(library.pendingSwitch);
  const name = $derived(pending ? EDITION_NAMES[pending.check.edition] : '');
</script>

<Dialog open={!!pending} title="You have a {name} save in Eden" onclose={() => library.cancelSwitch()} width={540}>
  {#snippet lead()}
    <span class="lead" aria-hidden="true"><Icon name="alert" size={18} /></span>
  {/snippet}
  {#if pending}
    <p>
      Switching seeds in the middle of a playthrough can break that save. Pokémon, items and story progress from
      {pending.check.active_seed ? `“${pending.check.active_seed}”` : 'the game you played'} may not exist in
      <strong>{pending.seed.name}</strong>.
    </p>
    <p class="hint">Backing up copies the save folders below to the app’s backup folder first, so you can go back.</p>
    <ul class="saves">
      {#each pending.check.saves as save (save)}
        <li class="mono">{save}</li>
      {/each}
    </ul>
  {/if}
  {#snippet footer()}
    <button class="btn ghost" onclick={() => library.cancelSwitch()}>Cancel</button>
    <button class="btn" onclick={() => library.confirmSwitch(false)}>Switch without backup</button>
    <button class="btn primary" onclick={() => library.confirmSwitch(true)}>
      <Icon name="download" size={15} />Back up save and switch
    </button>
  {/snippet}
</Dialog>

<style>
  .lead {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: var(--r-m);
    background: color-mix(in oklab, var(--warn) 16%, transparent);
    color: var(--warn);
  }

  p {
    font-size: 13.5px;
    line-height: 1.55;
  }
  p strong {
    color: var(--text);
  }

  .hint {
    margin-top: 10px;
    color: var(--text-3);
    font-size: 13px;
  }

  .saves {
    margin: 12px 0 0;
    padding: 0;
    list-style: none;
  }
  .saves li {
    padding: 9px 11px;
    border-radius: var(--r-m);
    background: var(--ink);
    color: var(--text-2);
    font-size: 11.5px;
    overflow-wrap: anywhere;
  }
  .saves li + li {
    margin-top: 4px;
  }
</style>
