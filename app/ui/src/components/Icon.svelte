<script lang="ts" module>
  const ICONS = {
    search: 'M10.5 4a6.5 6.5 0 1 0 0 13 6.5 6.5 0 0 0 0-13ZM20 20l-4.6-4.6',
    dice: 'M7.5 3.5h9a4 4 0 0 1 4 4v9a4 4 0 0 1-4 4h-9a4 4 0 0 1-4-4v-9a4 4 0 0 1 4-4Z',
    chevronDown: 'M6.5 9.5 12 15l5.5-5.5',
    chevronRight: 'M9.5 6.5 15 12l-5.5 5.5',
    check: 'M5 12.5 9.5 17 19 7.5',
    x: 'M6.5 6.5l11 11M17.5 6.5l-11 11',
    alert: 'M12 4 2.8 19.5h18.4L12 4ZM12 10v4.2M12 16.9h.01',
    info: 'M12 3.5a8.5 8.5 0 1 0 0 17 8.5 8.5 0 0 0 0-17ZM12 11v5M12 8h.01',
    folder: 'M3.5 7a2 2 0 0 1 2-2h3.8l2 2.4h7.2a2 2 0 0 1 2 2V17a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2Z',
    file: 'M13.5 3.5H7.5a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2h9a2 2 0 0 0 2-2V8.5ZM13.5 3.5v5h5M9 13h6M9 16.5h4',
    trash: 'M4.5 7h15M10 11v5.5M14 11v5.5M6.5 7l.8 11.3a2 2 0 0 0 2 1.7h5.4a2 2 0 0 0 2-1.7L17.5 7M9.5 7V4.5h5V7',
    play: 'M8 5.8v12.4a.8.8 0 0 0 1.2.7l9.6-6.2a.8.8 0 0 0 0-1.4L9.2 5.1a.8.8 0 0 0-1.2.7Z',
    power: 'M12 3.5v7.5M6.6 6.9a8 8 0 1 0 10.8 0',
    sliders: 'M4 7h9M17 7h3M4 17h3M11 17h9M15 5v4M9 15v4',
    upload: 'M12 15V4.5M7.5 9 12 4.5 16.5 9M4.5 15v2.5a2 2 0 0 0 2 2h11a2 2 0 0 0 2-2V15',
    download: 'M12 4.5V15M7.5 10.5 12 15l4.5-4.5M4.5 15v2.5a2 2 0 0 0 2 2h11a2 2 0 0 0 2-2V15',
    link: 'M10 14a4.2 4.2 0 0 0 6 0l3-3a4.2 4.2 0 0 0-6-6l-1 1M14 10a4.2 4.2 0 0 0-6 0l-3 3a4.2 4.2 0 0 0 6 6l1-1',
    copy: 'M9.5 9.5h8.5a2 2 0 0 1 2 2V18a2 2 0 0 1-2 2h-6.5a2 2 0 0 1-2-2ZM14.5 9.5V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v6.5a2 2 0 0 0 2 2h3.5',
    reset: 'M4.5 12a7.5 7.5 0 1 0 2.2-5.3L4.5 9M4.5 4.5V9H9',
    plus: 'M12 5.5v13M5.5 12h13',
    minus: 'M5.5 12h13',
    lock: 'M7 11h10a1.5 1.5 0 0 1 1.5 1.5v6A1.5 1.5 0 0 1 17 20H7a1.5 1.5 0 0 1-1.5-1.5v-6A1.5 1.5 0 0 1 7 11ZM8.5 11V8a3.5 3.5 0 0 1 7 0v3',
    stack: 'M4 8 12 4.5 20 8l-8 3.5L4 8ZM4 12.2l8 3.5 8-3.5M4 16.4l8 3.5 8-3.5',
    cog: 'M12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6ZM12 3v2.5M12 18.5V21M5.6 5.6l1.8 1.8M16.6 16.6l1.8 1.8M3 12h2.5M18.5 12H21M5.6 18.4l1.8-1.8M16.6 7.4l1.8-1.8',
    arrowRight: 'M5 12h14M13.5 6.5 19 12l-5.5 5.5',
    spinner: 'M12 3.5a8.5 8.5 0 1 0 8.5 8.5',
    external: 'M13.5 4.5h6v6M19.5 4.5 11 13M17.5 14v4a2 2 0 0 1-2 2h-9a2 2 0 0 1-2-2V9a2 2 0 0 1 2-2h4',
    spark: 'M12 3.5l1.9 5.2 5.2 1.9-5.2 1.9-1.9 5.2-1.9-5.2-5.2-1.9 5.2-1.9Z',
    paste: 'M9 3.5h6v3H9ZM15 5h2a2 2 0 0 1 2 2v11.5a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2h2',
    clock: 'M12 3.5a8.5 8.5 0 1 0 0 17 8.5 8.5 0 0 0 0-17ZM12 7.5V12l3 2',
    dash: 'M7 12h10',
    undo: 'M9 14 4.5 9.5 9 5M4.5 9.5h10a5 5 0 0 1 0 10H11',
    layers: 'M12 4.5 20 9l-8 4.5L4 9l8-4.5ZM4 13.5l8 4.5 8-4.5',
  } satisfies Record<string, string>;

  export type IconName = keyof typeof ICONS;
</script>

<script lang="ts">
  let { name, size = 16, stroke = 1.6, class: className = '' }: { name: IconName; size?: number; stroke?: number; class?: string } =
    $props();
</script>

<svg
  class={className}
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width={stroke}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  <path d={ICONS[name]} />
  {#if name === 'dice'}
    <g fill="currentColor" stroke="none">
      <circle cx="8.6" cy="8.6" r="1.25" />
      <circle cx="15.4" cy="8.6" r="1.25" />
      <circle cx="12" cy="12" r="1.25" />
      <circle cx="8.6" cy="15.4" r="1.25" />
      <circle cx="15.4" cy="15.4" r="1.25" />
    </g>
  {/if}
</svg>

<style>
  svg {
    flex: none;
    display: block;
  }
</style>
