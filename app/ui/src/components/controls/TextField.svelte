<script lang="ts">
  let {
    value,
    max,
    label,
    multiline,
    onchange,
  }: { value: string; max: number; label: string; multiline: boolean; onchange: (value: string) => void } = $props();

  const length = $derived([...value].length);
</script>

<div class="text-field" class:multiline>
  {#if multiline}
    <textarea class="field" rows="3" maxlength={max} aria-label={label} {value} oninput={(e) => onchange(e.currentTarget.value)}></textarea>
  {:else}
    <input class="field" maxlength={max} aria-label={label} {value} oninput={(e) => onchange(e.currentTarget.value)} />
  {/if}
  <span class="count num" class:full={length >= max}>{length} / {max}</span>
</div>

<style>
  .text-field {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 5px;
  }
  .text-field.multiline {
    width: 100%;
  }
  input {
    width: 240px;
  }
  textarea {
    width: 100%;
  }
  .count {
    color: var(--text-4);
    font-size: 11.5px;
  }
  .count.full {
    color: var(--warn);
  }
</style>
