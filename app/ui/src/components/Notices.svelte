<script lang="ts">
  import { app } from '../lib/app.svelte';
  import { EDITION_NAMES } from '../lib/format';
  import { poolName } from '../lib/options';
  import Icon from './Icon.svelte';

  type Action = { label: string; run: () => void; primary?: boolean };
  type Notice = { id: string; tone: 'danger' | 'warn' | 'info'; title: string; body: string; actions: Action[] };

  const settingsAction: Action = { label: 'Open App settings', run: () => app.goTo({ kind: 'settings' }) };

  /** At most one environment problem at a time, most fundamental first, then the pool block. */
  const notices = $derived.by((): Notice[] => {
    const list: Notice[] = [];
    const env = app.env;
    const name = EDITION_NAMES[app.edition];
    const status = app.gameStatus;
    const onSettings = app.view.kind === 'settings';

    if (app.envError && !env) {
      list.push({
        id: 'env',
        tone: 'danger',
        title: 'Eden and your games could not be checked',
        body: app.envError,
        actions: [{ label: 'Try again', run: () => app.refreshEnv(), primary: true }],
      });
    } else if (env?.keys_error) {
      list.push({
        id: 'keys',
        tone: 'danger',
        title: 'Your keys file did not load',
        body: `${env.keys_error} The randomizer needs it to read your game files.`,
        actions: [{ label: 'Choose keys file', run: () => app.chooseKeysFile(), primary: true }, ...(onSettings ? [] : [settingsAction])],
      });
    } else if (env && env.games.every((game) => game.state === 'missing')) {
      list.push({
        id: 'no-games',
        tone: 'danger',
        title: 'No games found',
        body: status?.message ?? 'No game folders are set up yet.',
        actions: [
          { label: 'Add game folder', run: () => app.addGameFolder(), primary: true },
          { label: `Choose the ${name} .xci`, run: () => app.chooseGameFile(app.edition, 'base') },
        ],
      });
    } else if (env && status && status.state !== 'ready') {
      const fix: Action =
        status.state === 'missing' || !status.base
          ? { label: `Choose the ${name} .xci`, run: () => app.chooseGameFile(app.edition, 'base'), primary: true }
          : { label: 'Choose the update file', run: () => app.chooseGameFile(app.edition, 'update'), primary: true };
      list.push({
        id: 'game',
        tone: 'warn',
        title: `${name} is not ready`,
        body: status.message ?? `${name} could not be opened.`,
        actions: [fix, ...(onSettings ? [] : [settingsAction])],
      });
    }

    if (app.view.kind === 'library' && env && !env.eden.exe.exists) {
      list.push({
        id: 'eden',
        tone: 'info',
        title: 'Eden was not found',
        body: env.eden.exe.path
          ? `Nothing is at “${env.eden.exe.path}”. You can still make seeds; to switch and play them from here, choose eden.exe.`
          : 'You can still make seeds; to switch and play them from here, choose eden.exe.',
        actions: [{ label: 'Choose eden.exe', run: () => app.chooseEdenExe(), primary: true }],
      });
    }

    const block = app.block;
    if (block?.kind === 'pool') {
      list.push({
        id: 'pool',
        tone: 'warn',
        title: `${poolName(block.def, app.catalog)} has no species left`,
        body: 'Randomize stays off until at least one species fits. Loosen its filters or shorten its ban list.',
        actions: [{ label: 'Show me the pool', run: () => app.reveal(block.def.id), primary: true }],
      });
    }
    return list;
  });

  const ICON = { danger: 'alert', warn: 'alert', info: 'info' } as const;
</script>

{#if notices.length}
  <div class="notices">
    {#each notices as notice (notice.id)}
      <div class="notice {notice.tone}" role={notice.tone === 'danger' ? 'alert' : 'status'}>
        <span class="icon"><Icon name={ICON[notice.tone]} size={17} /></span>
        <div class="text">
          <p class="title">{notice.title}</p>
          <p class="body">{notice.body}</p>
        </div>
        <div class="actions">
          {#each notice.actions as action (action.label)}
            <button class="btn sm" class:primary={action.primary} onclick={action.run}>{action.label}</button>
          {/each}
        </div>
      </div>
    {/each}
  </div>
{/if}

<style>
  .notices {
    display: flex;
    flex-direction: column;
    gap: 10px;
    max-width: 880px;
    margin-bottom: 28px;
  }

  .notice {
    --tone: var(--warn);
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: start;
    gap: 12px;
    padding: 14px 14px 14px 16px;
    border-radius: var(--r-l);
    background: color-mix(in oklab, var(--tone) 9%, var(--surface-2));
    box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--tone) 28%, transparent);
    animation: notice-in 260ms var(--ease-out);
  }
  .notice.danger {
    --tone: var(--danger);
  }
  .notice.info {
    --tone: var(--info);
  }

  .icon {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: var(--r-m);
    background: color-mix(in oklab, var(--tone) 18%, transparent);
    color: color-mix(in oklab, var(--tone) 80%, white);
  }

  .title {
    font-size: 14px;
    font-weight: 620;
  }

  .body {
    margin-top: 3px;
    color: var(--text-2);
    font-size: 13px;
    line-height: 1.5;
    overflow-wrap: anywhere;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 8px;
    max-width: 300px;
  }

  @keyframes notice-in {
    from {
      opacity: 0;
      translate: 0 -4px;
    }
  }
</style>
