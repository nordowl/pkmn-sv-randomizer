<script lang="ts">
  import { api, EDITIONS, type AppConfig, type Edition, type Environment, type GameFileOverride, type PathSource, type ResolvedPath } from '../lib/api';
  import { app } from '../lib/app.svelte';
  import { EDITION_NAMES } from '../lib/format';
  import { sameValue } from '../lib/options';
  import Icon from './Icon.svelte';
  import PathRow from './PathRow.svelte';

  type PathKey = 'eden_exe' | 'eden_data_dir' | 'keys_file' | 'load_dir' | 'nand_dir' | 'library_dir';
  type Edits = Partial<Pick<AppConfig, PathKey | 'game_dirs' | 'scarlet' | 'violet'>>;

  interface PathField {
    key: PathKey;
    label: string;
    help: string;
    pick: () => Promise<string | null>;
    resolved: (env: Environment) => { path: string | null; source: PathSource; exists: boolean | null };
  }

  const folder = (title: string) => () => api.pickFolder(title);

  const EDEN: PathField[] = [
    {
      key: 'eden_exe',
      label: 'Eden program',
      help: 'eden.exe, which Play in Eden starts.',
      pick: () => api.pickFile('Choose eden.exe', [{ name: 'Eden', extensions: ['exe'] }]),
      resolved: (env) => env.eden.exe,
    },
    {
      key: 'eden_data_dir',
      label: 'Eden data folder',
      help: 'Where Eden keeps its keys, mods and saves.',
      pick: folder('Choose Eden’s data folder'),
      resolved: (env) => env.eden.data_dir,
    },
    {
      key: 'load_dir',
      label: 'Mod folder',
      help: 'Eden’s load folder. The seed you switch on is installed here.',
      pick: folder('Choose Eden’s load folder'),
      resolved: (env) => env.eden.load_dir,
    },
    {
      key: 'nand_dir',
      label: 'NAND folder',
      help: 'Holds your saves, so the app can warn you and back them up before switching seeds.',
      pick: folder('Choose Eden’s NAND folder'),
      resolved: (env) => env.eden.nand_dir,
    },
  ];

  const KEYS: PathField = {
    key: 'keys_file',
    label: 'Keys file',
    help: 'Your prod.keys. The randomizer needs it to read the game files.',
    pick: () => api.pickFile('Choose your prod.keys file', [{ name: 'Switch keys', extensions: ['keys'] }]),
    resolved: (env) => env.eden.keys,
  };

  const LIBRARY: PathField = {
    key: 'library_dir',
    label: 'Seed library',
    help: 'Where new seeds are saved, unless a batch goes somewhere else.',
    pick: folder('Choose the seed library folder'),
    resolved: (env) => ({ path: env.library_dir, source: env.config.library_dir ? 'override' : 'default', exists: null }),
  };

  const FILES = ['base', 'update'] as const;

  let edits = $state<Edits>({});
  let saving = $state(false);

  const env = $derived(app.env);
  const config = $derived(env?.config ?? null);
  const dirty = $derived(
    !!config && (Object.keys(edits) as (keyof Edits)[]).some((key) => !sameValue(edits[key], config[key])),
  );

  function shown(field: PathField, env: Environment) {
    const saved = env.config[field.key];
    const pending = field.key in edits && edits[field.key] !== saved;
    const draft = field.key in edits ? (edits[field.key] ?? null) : saved;
    const resolved = field.resolved(env);
    return {
      path: pending ? (draft ?? null) : resolved.path,
      source: pending ? ('unsaved' as const) : resolved.source,
      exists: pending ? null : resolved.exists,
      pending,
      overridden: draft !== null,
    };
  }

  async function choose(field: PathField) {
    try {
      const path = await field.pick();
      if (path) edits[field.key] = path;
    } catch (error) {
      app.fail('Nothing was chosen', error);
    }
  }

  const games = (edition: Edition): GameFileOverride => edits[edition] ?? config?.[edition] ?? { base: null, update: null };

  function gameFile(edition: Edition, which: 'base' | 'update', env: Environment) {
    const draft = games(edition)[which];
    const saved = env.config[edition][which];
    const status = env.games.find((game) => game.edition === edition);
    const pending = draft !== saved;
    const found = status?.[which] ?? null;
    const resolved: ResolvedPath = saved
      ? { path: saved, source: 'override', exists: true }
      : { path: found, source: found ? 'detected' : 'default', exists: !!found };
    return {
      path: pending ? draft : resolved.path,
      source: pending ? ('unsaved' as const) : resolved.source,
      exists: pending || !resolved.path ? null : status?.state === 'ready' ? true : null,
      pending,
      overridden: draft !== null,
    };
  }

  async function chooseGameFile(edition: Edition, which: 'base' | 'update') {
    const name = EDITION_NAMES[edition];
    const filter = which === 'base' ? { name: `${name} cartridge dump`, extensions: ['xci'] } : { name: `${name} 4.0.0 update`, extensions: ['nsp'] };
    try {
      const path = await api.pickFile(which === 'base' ? `Choose the ${name} .xci` : `Choose the ${name} 4.0.0 update .nsp`, [filter]);
      if (path) edits[edition] = { ...games(edition), [which]: path };
    } catch (error) {
      app.fail('Nothing was chosen', error);
    }
  }

  const dirs = $derived(edits.game_dirs !== undefined ? (edits.game_dirs ?? env?.eden.game_dirs ?? []) : (config?.game_dirs ?? env?.eden.game_dirs ?? []));
  const ownDirs = $derived(edits.game_dirs !== undefined ? edits.game_dirs !== null : !!config?.game_dirs);

  async function addDir() {
    try {
      const path = await api.pickFolder('Add a folder with your game dumps');
      if (path && !dirs.includes(path)) edits.game_dirs = [...dirs, path];
    } catch (error) {
      app.fail('No folder was added', error);
    }
  }

  async function save() {
    if (!env) return;
    saving = true;
    // Spread over the latest config, so batch numbering and batch folders the backend recorded survive.
    const ok = await app.saveConfig({ ...env.config, ...edits });
    saving = false;
    if (ok) edits = {};
  }
</script>

{#snippet pathField(field: PathField, env: Environment, clearLabel = 'Use detected')}
  {@const row = shown(field, env)}
  <PathRow
    label={field.label}
    help={field.help}
    path={row.path}
    source={row.source}
    exists={row.exists}
    pending={row.pending}
    {clearLabel}
    onpick={() => choose(field)}
    onclear={row.overridden ? () => (edits[field.key] = null) : undefined}
  />
{/snippet}

<article class="settings">
  <header>
    <div>
      <h1>App settings</h1>
      <p class="meta">Where the randomizer finds Eden, your keys and your games. Everything is detected unless you choose otherwise.</p>
    </div>
    <button class="btn ghost sm" onclick={() => app.refreshEnv()}><Icon name="reset" size={14} />Check again</button>
  </header>

  {#if !env}
    <p class="loading">{app.envError ?? 'Looking for Eden and your games…'}</p>
  {:else}
    <section class="block">
      <h2>Games</h2>
      <div class="games">
        {#each EDITIONS as edition (edition)}
          {@const status = env.games.find((game) => game.edition === edition)}
          <div class="game" data-edition={edition}>
            <div class="game-head">
              <span class="swatch"></span>
              <h3>{EDITION_NAMES[edition]}</h3>
              <span class="mono title-id">{status?.title_id}</span>
              {#if status?.state === 'ready'}
                <span class="tag ok status">Ready</span>
              {:else if status?.state === 'error'}
                <span class="tag test status">Problem</span>
              {:else}
                <span class="tag status">Not found</span>
              {/if}
            </div>
            {#if status?.message}<p class="message">{status.message}</p>{/if}
            {#if status?.mod_disabled}
              <p class="message warn">Eden has the randomizer switched off for this game in its add-on list.</p>
            {/if}
            {#each FILES as which (which)}
              {@const file = gameFile(edition, which, env)}
              <PathRow
                label={which === 'base' ? 'Game (.xci)' : 'Update 4.0.0 (.nsp)'}
                help={which === 'base' ? 'Your cartridge dump of the base game.' : 'The 4.0.0 update. Other versions do not work.'}
                path={file.path}
                source={file.source}
                exists={file.exists}
                pending={file.pending}
                onpick={() => chooseGameFile(edition, which)}
                onclear={file.overridden ? () => (edits[edition] = { ...games(edition), [which]: null }) : undefined}
              />
            {/each}
          </div>
        {/each}
      </div>
    </section>

    <section class="block">
      <div class="block-head">
        <div>
          <h2>Game folders</h2>
          <p class="sub">Searched for .xci and .nsp files. {ownDirs ? 'You set this list.' : 'Taken from Eden’s own game folders.'}</p>
        </div>
        <div class="actions">
          <button class="btn sm" onclick={addDir}><Icon name="plus" size={14} />Add folder</button>
          {#if ownDirs}
            <button class="btn sm ghost" onclick={() => (edits.game_dirs = null)}><Icon name="undo" size={14} />Use Eden’s folders</button>
          {/if}
        </div>
      </div>
      {#if dirs.length}
        <ul class="dirs">
          {#each dirs as dir (dir)}
            <li>
              <Icon name="folder" size={15} />
              <span class="mono">{dir}</span>
              <button class="btn sm ghost icon" aria-label="Remove {dir}" title="Remove" onclick={() => (edits.game_dirs = dirs.filter((d) => d !== dir))}>
                <Icon name="x" size={14} />
              </button>
            </li>
          {/each}
        </ul>
      {:else}
        <p class="empty">No game folders yet. Add the folder that holds your .xci and .nsp files.</p>
      {/if}
    </section>

    <section class="block">
      <h2>Keys</h2>
      {@render pathField(KEYS, env)}
      {#if env.keys_error}<p class="message bad">{env.keys_error}</p>{/if}
    </section>

    <section class="block">
      <h2>Eden</h2>
      {#if env.eden.portable}<p class="sub">Eden runs in portable mode: its data lives in the user folder next to eden.exe.</p>{/if}
      {#each EDEN as field (field.key)}
        {@render pathField(field, env)}
      {/each}
    </section>

    <section class="block">
      <h2>Seeds</h2>
      {@render pathField(LIBRARY, env, 'Use the default')}
    </section>
  {/if}

  {#if dirty}
    <div class="savebar">
      <p>You have unsaved changes.</p>
      <button class="btn ghost" onclick={() => (edits = {})}>Discard</button>
      <button class="btn primary" disabled={saving} onclick={save}>{saving ? 'Saving…' : 'Save changes'}</button>
    </div>
  {/if}
</article>

<style>
  .settings {
    max-width: 880px;
    padding-bottom: 40px;
  }

  header {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 16px;
    margin-bottom: 26px;
  }
  h1 {
    font: 620 30px/1.1 var(--font-display);
    font-stretch: 92%;
    letter-spacing: -0.02em;
  }
  .meta {
    margin-top: 8px;
    max-width: 560px;
    color: var(--text-3);
    font-size: 13px;
  }

  .loading {
    color: var(--text-3);
  }

  .block {
    margin-bottom: 30px;
    padding: 18px 20px 6px;
    border-radius: var(--r-xl);
    background: var(--surface-2);
    box-shadow: 0 0 0 1px var(--line);
  }
  .block h2 {
    margin-bottom: 2px;
    color: var(--text-3);
    font: 600 11.5px/1 var(--font-ui);
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }
  .block-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    margin-bottom: 12px;
  }
  .sub {
    margin-top: 8px;
    color: var(--text-3);
    font-size: 12.5px;
  }
  .actions {
    display: flex;
    gap: 6px;
  }

  .games {
    display: flex;
    flex-direction: column;
    gap: 12px;
    margin: 14px 0 14px;
  }
  .game {
    padding: 14px 16px 0;
    border-radius: var(--r-l);
    background: var(--ink);
    box-shadow: inset 3px 0 0 color-mix(in oklab, var(--accent) 70%, transparent);
  }
  .game-head {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .swatch {
    width: 9px;
    height: 9px;
    border-radius: 3px;
    rotate: 45deg;
    background: var(--accent);
  }
  .game h3 {
    font: 620 17px/1 var(--font-display);
    font-stretch: 90%;
  }
  .title-id {
    flex: 1;
    color: var(--text-4);
    font-size: 11.5px;
  }

  .message {
    margin-top: 10px;
    padding: 9px 11px;
    border-radius: var(--r-m);
    background: color-mix(in oklab, var(--warn) 9%, transparent);
    color: var(--text-2);
    font-size: 12.5px;
    line-height: 1.5;
    overflow-wrap: anywhere;
  }
  .message.bad {
    margin-bottom: 14px;
    background: color-mix(in oklab, var(--danger) 10%, transparent);
  }

  .dirs {
    margin: 0 0 14px;
    padding: 0;
    list-style: none;
  }
  .dirs li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 6px 6px 12px;
    border-radius: var(--r-m);
    background: var(--ink);
    color: var(--text-3);
  }
  .dirs li + li {
    margin-top: 4px;
  }
  .dirs .mono {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    color: var(--text);
    font-size: 12.5px;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .empty {
    margin-bottom: 14px;
    color: var(--text-3);
    font-size: 13px;
  }

  .savebar {
    position: sticky;
    bottom: 16px;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 10px 10px 18px;
    border-radius: var(--r-l);
    background: var(--surface-3);
    box-shadow: var(--shadow-pop);
    animation: rise 220ms var(--ease-out);
  }
  .savebar p {
    flex: 1;
    color: var(--text);
    font-size: 13.5px;
  }
  @keyframes rise {
    from {
      opacity: 0;
      translate: 0 8px;
    }
  }
</style>
