<script lang="ts">
  import type { PathSource } from '../lib/api';
  import Icon from './Icon.svelte';

  let {
    label,
    help,
    path,
    source,
    exists,
    pending,
    pickLabel = 'Choose…',
    clearLabel = 'Use detected',
    onpick,
    onclear,
  }: {
    label: string;
    help: string;
    path: string | null;
    source: PathSource | 'unsaved';
    /** null when the app cannot know, such as before saving. */
    exists: boolean | null;
    pending: boolean;
    pickLabel?: string;
    clearLabel?: string;
    onpick: () => void;
    /** Present only when there is an override to drop. */
    onclear?: () => void;
  } = $props();

  const SOURCE_TEXT: Record<PathSource | 'unsaved', { text: string; title: string }> = {
    detected: { text: 'Detected', title: 'Found by looking at Eden and your folders' },
    override: { text: 'Your choice', title: 'You picked this; it replaces what was detected' },
    default: { text: 'Default', title: 'The usual place; nothing was found or chosen' },
    unsaved: { text: 'Not saved yet', title: 'Save changes to use this' },
  };
</script>

<div class="path-row" class:pending>
  <div class="text">
    <h3>{label}</h3>
    <p class="help">{help}</p>
    <div class="value">
      {#if path}
        <span class="mono path" title={path}>{path}</span>
      {:else}
        <span class="none">Not found</span>
      {/if}
      <span class="tag" class:accent={pending} title={SOURCE_TEXT[source].title}>{SOURCE_TEXT[source].text}</span>
      {#if path && exists === true}
        <span class="tag ok status">Found</span>
      {:else if path && exists === false}
        <span class="tag bad status">Missing</span>
      {/if}
    </div>
  </div>
  <div class="actions">
    <button class="btn sm" onclick={onpick}><Icon name="folder" size={14} />{pickLabel}</button>
    {#if onclear}
      <button class="btn sm ghost" onclick={onclear}><Icon name="undo" size={14} />{clearLabel}</button>
    {/if}
  </div>
</div>

<style>
  .path-row {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 24px;
    padding: 16px 0;
  }
  .path-row + :global(.path-row) {
    border-top: 1px solid var(--line);
  }

  .text {
    flex: 1;
    min-width: 0;
  }

  h3 {
    font-size: 14px;
    font-weight: 600;
  }

  .help {
    margin-top: 3px;
    color: var(--text-3);
    font-size: 12.5px;
  }

  .value {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 9px;
    min-width: 0;
  }

  .path {
    min-width: 0;
    overflow: hidden;
    color: var(--text);
    font-size: 12.5px;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .pending .path {
    color: var(--accent-text);
  }

  .none {
    color: var(--text-3);
    font-size: 13px;
  }

  .actions {
    display: flex;
    flex: none;
    gap: 6px;
    padding-top: 2px;
  }
</style>
