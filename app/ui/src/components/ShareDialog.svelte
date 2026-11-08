<script lang="ts">
  import { api } from '../lib/api';
  import { app } from '../lib/app.svelte';
  import { copyText } from '../lib/clipboard';
  import { errorText } from '../lib/format';
  import Dialog from './Dialog.svelte';
  import Icon from './Icon.svelte';

  let { open, onclose }: { open: boolean; onclose: () => void } = $props();

  let withSeed = $state(true);
  let code = $state<string | null>(null);
  let codeError = $state<string | null>(null);
  let copied = $state(false);
  let pasted = $state('');
  let applying = $state(false);
  let applyError = $state<string | null>(null);
  let warnings = $state<string[] | null>(null);

  const seed = $derived(app.seed.trim());

  $effect(() => {
    if (!open) return;
    const settings = app.settings;
    const includeSeed = withSeed && seed ? seed : null;
    let current = true;
    code = null;
    codeError = null;
    copied = false;
    api.encodeShareCode(settings, includeSeed).then(
      (made) => current && (code = made),
      (error) => current && (codeError = errorText(error)),
    );
    return () => (current = false);
  });

  $effect(() => {
    if (open) return;
    pasted = '';
    applyError = null;
    warnings = null;
  });

  async function copy() {
    if (!code) return;
    copied = await copyText(code);
    if (!copied) app.toast({ tone: 'error', title: 'Could not copy', body: 'Select the code and copy it with Ctrl+C instead.' });
  }

  async function apply() {
    applying = true;
    applyError = null;
    try {
      const decoded = await api.decodeShareCode(pasted.trim());
      const previousSeed = app.seed;
      app.replaceSettings(
        decoded.settings,
        'Applied the share code',
        { body: decoded.seed ? `Seed set to ${decoded.seed}.` : undefined },
        () => (app.seed = previousSeed),
      );
      if (decoded.seed) app.seed = decoded.seed;
      if (decoded.warnings.length) warnings = decoded.warnings;
      else onclose();
    } catch (error) {
      applyError = errorText(error);
    } finally {
      applying = false;
    }
  }

</script>

<Dialog {open} {onclose} title="Share code" width={560}>
  <section>
    <h3>Copy a code for these settings</h3>
    <p>Anyone with the code can load the same settings. With the seed included, they get the same game too.</p>
    <label class="check" class:disabled={!seed}>
      <input
        type="checkbox"
        checked={withSeed && !!seed}
        disabled={!seed}
        onchange={(event) => (withSeed = event.currentTarget.checked)}
      />
      {#if seed}
        Include the seed <span class="mono">{seed}</span>
      {:else}
        Include the seed (type or roll one first)
      {/if}
    </label>
    <div class="code-row">
      <textarea class="field mono code" readonly rows="3" value={code ?? codeError ?? 'Making the code…'} aria-label="Share code"
        onfocus={(e) => e.currentTarget.select()}></textarea>
      <button class="btn primary" disabled={!code} onclick={copy}>
        <Icon name={copied ? 'check' : 'copy'} size={15} />{copied ? 'Copied' : 'Copy code'}
      </button>
    </div>
  </section>

  <section>
    <h3>Use a code</h3>
    <p>Paste a code someone shared with you. It replaces your current settings; you can undo that right after.</p>
    <textarea class="field mono code" rows="3" placeholder="SVR1-…" bind:value={pasted} aria-label="Code to apply"></textarea>
    {#if applyError}<p class="error" role="alert">{applyError}</p>{/if}
    {#if warnings}
      <div class="warnings" role="status">
        <p><strong>Applied.</strong> A few values did not fit and were reset:</p>
        <ul>{#each warnings as warning, i (i)}<li>{warning}</li>{/each}</ul>
      </div>
    {/if}
    <div class="apply">
      {#if warnings}
        <button class="btn" onclick={onclose}>Done</button>
      {:else}
        <button class="btn" disabled={!pasted.trim() || applying} onclick={apply}>
          <Icon name="paste" size={15} />{applying ? 'Applying…' : 'Apply code'}
        </button>
      {/if}
    </div>
  </section>
</Dialog>

<style>
  section + section {
    margin-top: 22px;
    padding-top: 20px;
    border-top: 1px solid var(--line);
  }

  h3 {
    color: var(--text);
    font-size: 14px;
    font-weight: 600;
  }

  p {
    margin-top: 4px;
    font-size: 13px;
  }

  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 14px 0 10px;
    color: var(--text);
    font-size: 13px;
    cursor: pointer;
  }
  .check.disabled {
    color: var(--text-3);
    cursor: default;
  }
  .check input {
    accent-color: var(--accent);
    width: 15px;
    height: 15px;
    margin: 0;
  }

  .code-row {
    display: flex;
    gap: 10px;
    align-items: flex-start;
  }

  .code {
    width: 100%;
    margin-top: 10px;
    font-size: 12px;
    line-height: 1.5;
    word-break: break-all;
    resize: none;
  }
  .code-row .code {
    margin-top: 0;
  }

  .error {
    margin-top: 8px;
    color: color-mix(in oklab, var(--danger) 75%, white);
  }

  .warnings {
    margin-top: 10px;
    padding: 10px 12px;
    border-radius: var(--r-m);
    background: color-mix(in oklab, var(--warn) 10%, transparent);
    font-size: 13px;
  }
  .warnings p {
    margin: 0;
    color: var(--text);
  }
  .warnings ul {
    margin: 6px 0 0;
    padding-left: 18px;
    color: var(--text-2);
  }

  .apply {
    display: flex;
    justify-content: flex-end;
    margin-top: 10px;
  }
</style>
