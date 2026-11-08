import '@fontsource-variable/bricolage-grotesque/standard.css';
import '@fontsource-variable/instrument-sans/wdth.css';
import '@fontsource-variable/geist-mono/index.css';
import './app.css';

import { mount } from 'svelte';
import App from './App.svelte';
import { useApi } from './lib/api';

if (import.meta.env.DEV && !('__TAURI_INTERNALS__' in window)) {
  const { mockApi } = await import('./lib/mock');
  useApi(mockApi);
  document.documentElement.dataset.mock = '';
}

const target = document.getElementById('app');
if (!target) throw new Error('missing #app');

export default mount(App, { target });
