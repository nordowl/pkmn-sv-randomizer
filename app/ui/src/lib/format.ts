import type { Edition } from './api';

export const EDITION_NAMES: Record<Edition, string> = { scarlet: 'Scarlet', violet: 'Violet' };

/** Commands reject with a plain string; anything else is an unexpected failure. */
export function errorText(error: unknown): string {
  if (typeof error === 'string') return error;
  if (error instanceof Error) return error.message;
  return 'Something went wrong. Try again.';
}

const countFormat = new Intl.NumberFormat('en-US');
export const count = (n: number) => countFormat.format(n);

export function plural(n: number, one: string, many = `${one}s`): string {
  return `${count(n)} ${n === 1 ? one : many}`;
}

// English like the rest of the interface, so "Today" never sits beside a date in another language.
const dateFormat = new Intl.DateTimeFormat('en-GB', { day: 'numeric', month: 'short', year: 'numeric' });
const timeFormat = new Intl.DateTimeFormat('en-GB', { hour: '2-digit', minute: '2-digit' });

export function when(unix: number): string {
  const date = new Date(unix * 1000);
  const today = new Date();
  const yesterday = new Date(today);
  yesterday.setDate(today.getDate() - 1);
  const time = timeFormat.format(date);
  if (date.toDateString() === today.toDateString()) return `Today, ${time}`;
  if (date.toDateString() === yesterday.toDateString()) return `Yesterday, ${time}`;
  return `${dateFormat.format(date)}, ${time}`;
}

export function duration(ms: number): string {
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)} s`;
  const minutes = Math.floor(ms / 60_000);
  return `${minutes} min ${Math.round((ms % 60_000) / 1000)} s`;
}

export function folderName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
}

export function joinPath(dir: string, name: string): string {
  return `${dir.replace(/[\\/]+$/, '')}\\${name}`;
}

/** Matches the backend's batch folder names: prefix plus a three-digit index. */
export function batchName(prefix: string, index: number): string {
  return `${prefix.trimStart()}${String(index).padStart(3, '0')}`;
}

/** Same shape as the backend's random seeds: up to ten decimal digits. */
export function rollSeed(): string {
  const digits = crypto.getRandomValues(new Uint32Array(2));
  const value = (Number(digits[0]) * 4_294_967_296 + Number(digits[1])) % 9_000_000_000;
  return String(1_000_000_000 + value);
}
