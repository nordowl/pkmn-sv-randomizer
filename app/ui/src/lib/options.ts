import type { ChoiceDef, Kind, OptionDef, OptValue, PageInfo, Registry, Req } from './api';

/** The one option id the UI knows by name: every other pool inherits from it. */
export const GLOBAL_POOL = 'pool.global';

export const REGIONS = ['Paldea', 'Kitakami', 'Blueberry'] as const;
export type Region = (typeof REGIONS)[number];

export const STAGE_LABELS = ['Single-stage', 'First stage', 'Middle stage', 'Final stage'] as const;
export const DEX_LABELS = ['Paldea', 'Kitakami', 'Blueberry'] as const;
export const GENERATION_COUNT = 9;

export type NumberKind = Extract<Kind, { type: 'number' }>;
export type ChoiceKind = Extract<Kind, { type: 'choice' }>;

/** The region a group or option label names, as in "Kitakami raids" or "Species Kitakami trainers may use". */
export function regionOf(text: string): Region | null {
  const words = text.split(' ');
  return REGIONS.find((region) => words.includes(region)) ?? null;
}

/** "Sprigatito's slot" names the slot "Sprigatito". */
function slotOf(group: string): string | null {
  return /^(.+?)['’]s slot$/.exec(group)?.[1] ?? null;
}

export interface Group {
  name: string;
  /** How the group is named on its tab when it shares a section. */
  tab: string;
  options: OptionDef[];
}

/** One heading on an options page. Consecutive groups that open with the same option, such as the
 * regions, the starter slots and the trainer groups, are parallel and share one section with a tab each. */
export interface Section {
  id: string;
  title: string;
  groups: [Group, ...Group[]];
}

/** How a group's options are laid out, in order. */
export type Item =
  | { kind: 'option'; def: OptionDef }
  /** The same option for each region, such as the trainer pools, shown as one row with a tab per region. */
  | { kind: 'regional'; defs: [OptionDef, ...OptionDef[]] }
  /** Three or more plain switches that share one description, such as the star tiers, shown as one row of cards. */
  | { kind: 'switches'; help: string; defs: OptionDef[] }
  /** Three or more switches in a row that each unlock the options after them, such as the scripted
   * battle groups, shown with a tab per switch. Each part is the switch and what it unlocks. */
  | { kind: 'switched'; parts: [OptionDef, ...OptionDef[]][] };

/** Lookups over the registry, built once per registry. */
export class Catalog {
  readonly byId = new Map<string, OptionDef>();
  readonly pools: OptionDef[];
  private readonly byPage = new Map<string, OptionDef[]>();
  private readonly titles = new Map<string, string>();

  constructor(readonly registry: Registry) {
    for (const page of registry.pages) {
      this.byPage.set(page.id, []);
      this.titles.set(page.id, page.title);
    }
    for (const def of registry.options) {
      this.byId.set(def.id, def);
      const list = this.byPage.get(def.page);
      if (list) list.push(def);
      else this.byPage.set(def.page, [def]);
    }
    this.pools = registry.options.filter((def) => def.kind.type === 'pool');
  }

  pageTitle(id: string): string {
    return this.titles.get(id) ?? id;
  }

  options(page: string): OptionDef[] {
    return this.byPage.get(page) ?? [];
  }

  hasPage(id: string): boolean {
    return this.titles.has(id);
  }

  defaultOf(id: string): OptValue | undefined {
    return this.registry.defaults.values[id];
  }
}

export function groupsOf(options: OptionDef[]): Group[] {
  const groups: Group[] = [];
  for (const def of options) {
    let group = groups.find((g) => g.name === def.group);
    if (!group) {
      const tab = slotOf(def.group) ?? regionOf(def.group) ?? def.group;
      group = { name: def.group, tab, options: [] };
      groups.push(group);
    }
    group.options.push(def);
  }
  return groups;
}

export function sectionsOf(groups: Group[]): Section[] {
  const runs: [Group, ...Group[]][] = [];
  for (const group of groups) {
    const run = runs.at(-1);
    if (run && run[0].options[0]?.label === group.options[0]?.label) run.push(group);
    else runs.push([group]);
  }
  return runs.map((groups) => ({ id: slug(groups[0].name), title: sectionTitle(groups), groups }));
}

function sectionTitle(groups: [Group, ...Group[]]): string {
  if (groups.length === 1) return groups[0].name;
  if (groups.every((group) => regionOf(group.name))) return 'Regions';
  if (groups.every((group) => slotOf(group.name))) return 'Slots';
  return 'Groups';
}

function slug(text: string): string {
  return fold(text).replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '');
}

export function itemsOf(group: Group): Item[] {
  const items: Item[] = [];
  const placed = new Set<OptionDef>();
  const toggles = group.options.filter((def) => def.kind.type === 'toggle' && !def.requires.length);
  const shared = toggles[0]?.help;
  const switches = toggles.length >= 3 && toggles.every((def) => def.help === shared) ? toggles : [];

  group.options.forEach((def, index) => {
    if (placed.has(def)) return;
    const parts: [OptionDef, ...OptionDef[]][] = [];
    for (let at = index, part; (part = switchedAt(group.options, at)); at += part.length) parts.push(part);
    if (parts.length >= 3) {
      items.push({ kind: 'switched', parts });
      for (const each of parts.flat()) placed.add(each);
      return;
    }
    if (switches.includes(def)) {
      items.push({ kind: 'switches', help: shared ?? '', defs: switches });
      for (const each of switches) placed.add(each);
      return;
    }
    const base = def.kind.type === 'pool' ? withoutRegion(def.label) : null;
    const defs = group.options.filter(
      (other) => other.kind.type === 'pool' && !placed.has(other) && withoutRegion(other.label) === base,
    );
    if (defs.length > 1) {
      items.push({ kind: 'regional', defs: defs as [OptionDef, ...OptionDef[]] });
      for (const each of defs) placed.add(each);
      return;
    }
    items.push({ kind: 'option', def });
    placed.add(def);
  });
  return items;
}

/** The switch at `index` and the options right after it that only apply while it is on. */
function switchedAt(options: OptionDef[], index: number): [OptionDef, ...OptionDef[]] | null {
  const head = options[index];
  if (head?.kind.type !== 'toggle') return null;
  let end = index + 1;
  while (options[end]?.requires.some((req) => req.when === 'on' && req.id === head.id)) end++;
  return end > index + 1 ? [head, ...options.slice(index + 1, end)] : null;
}

/** "Species Kitakami trainers may use" and "Species trainers may use" are the same option for two regions. */
function withoutRegion(label: string): string {
  const region = regionOf(label);
  return region ? label.replace(region, '').replace(/\s+/g, ' ').trim() : label;
}

/** Mirrors `Settings::applies` in options.rs. */
export function holds(req: Req, values: Record<string, OptValue>): boolean {
  const value = values[req.id];
  switch (req.when) {
    case 'on':
      return value === true;
    case 'off':
      return value === false;
    case 'is':
      return value === req.value;
    case 'not':
      return value !== req.value;
  }
}

export function applies(def: OptionDef, values: Record<string, OptValue>): boolean {
  return def.requires.every((req) => holds(req, values));
}

/** "Turn on <target> first", split so the target can be a link. */
export interface Unmet {
  prefix: string;
  target: OptionDef | null;
  targetText: string;
  suffix: string;
}

export function choiceLabel(def: OptionDef | null | undefined, value: string): string {
  if (def?.kind.type !== 'choice') return value;
  return def.kind.choices.find((choice) => choice.value === value)?.label ?? value;
}

export function firstUnmet(def: OptionDef, catalog: Catalog, values: Record<string, OptValue>): Unmet | null {
  const req = def.requires.find((r) => !holds(r, values));
  if (!req) return null;
  const target = catalog.byId.get(req.id) ?? null;
  const targetText = target?.label ?? req.id;
  const where = target && target.page !== def.page ? ` on the ${catalog.pageTitle(target.page)} page` : '';
  switch (req.when) {
    case 'on':
      return { prefix: 'Turn on ', target, targetText, suffix: `${where} first` };
    case 'off':
      return { prefix: 'Turn off ', target, targetText, suffix: `${where} first` };
    case 'is':
      return { prefix: 'Set ', target, targetText, suffix: ` to ${choiceLabel(target, req.value)}${where} first` };
    case 'not':
      return {
        prefix: 'Set ',
        target,
        targetText,
        suffix: ` to something other than ${choiceLabel(target, req.value)}${where} first`,
      };
  }
}

/** How a pool is named in sentences such as "… has no species left". */
export function poolName(def: OptionDef, catalog: Catalog | null): string {
  if (def.id === GLOBAL_POOL) return 'The global pool';
  const page = catalog?.pageTitle(def.page);
  return page ? `${def.label} (${page})` : def.label;
}

export function sameValue(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (typeof a !== 'object' || typeof b !== 'object' || a === null || b === null) return false;
  if (Array.isArray(a) !== Array.isArray(b)) return false;
  const left = a as Record<string, unknown>;
  const right = b as Record<string, unknown>;
  const keys = Object.keys(left);
  return keys.length === Object.keys(right).length && keys.every((key) => sameValue(left[key], right[key]));
}

export function changedOn(options: OptionDef[], values: Record<string, OptValue>, catalog: Catalog): number {
  return options.filter((def) => !sameValue(values[def.id], catalog.defaultOf(def.id))).length;
}

/** Snaps to the step grid and range; the backend only accepts integers. */
export function fitNumber(raw: number, kind: Pick<NumberKind, 'default' | 'min' | 'max' | 'step'>): number {
  if (!Number.isFinite(raw)) return kind.default;
  const step = kind.step > 0 ? kind.step : 1;
  const stepped = kind.min + Math.round((raw - kind.min) / step) * step;
  return Math.min(kind.max, Math.max(kind.min, Math.round(stepped)));
}

/** Mirrors `with_unit` in describe.rs: "30%" but "3 levels". */
export function withUnit(value: number, unit: string): string {
  if (!unit) return String(value);
  return /[\p{L}\p{N}]/u.test(unit) ? `${value} ${unit}` : `${value}${unit}`;
}

export function chosen(kind: ChoiceKind, value: OptValue | undefined): ChoiceDef | undefined {
  return kind.choices.find((choice) => choice.value === value);
}

export function fold(text: string): string {
  return text.normalize('NFD').replace(/\p{Diacritic}/gu, '').toLowerCase();
}

export type SearchHit =
  | { kind: 'page'; page: PageInfo; optionCount: number }
  | { kind: 'option'; def: OptionDef; pageTitle: string; matched: 'label' | 'group' | 'choice' | 'help'; choice?: string };

interface Indexed {
  def: OptionDef;
  label: string;
  group: string;
  help: string;
  choices: { label: string; folded: string }[];
}

export class SearchIndex {
  private readonly items: Indexed[];

  constructor(private readonly catalog: Catalog) {
    this.items = catalog.registry.options.map((def) => ({
      def,
      label: fold(def.label),
      group: fold(def.group),
      help: fold(def.help),
      choices: def.kind.type === 'choice' ? def.kind.choices.map((c) => ({ label: c.label, folded: fold(c.label) })) : [],
    }));
  }

  search(query: string, limit = 40): SearchHit[] {
    const terms = fold(query).split(/\s+/).filter(Boolean);
    if (!terms.length) return [];
    const all = (text: string) => terms.every((term) => text.includes(term));

    const pages: SearchHit[] = this.catalog.registry.pages
      .filter((page) => all(fold(page.title)))
      .map((page) => ({ kind: 'page', page, optionCount: this.catalog.options(page.id).length }));

    const scored: { hit: SearchHit; score: number; order: number }[] = [];
    this.items.forEach((item, order) => {
      const choiceText = item.choices.map((c) => c.folded).join(' ');
      if (!all(`${item.label} ${item.group} ${choiceText} ${item.help}`)) return;
      const pageTitle = this.catalog.pageTitle(item.def.page);
      const base = { kind: 'option' as const, def: item.def, pageTitle };
      let hit: SearchHit;
      let score: number;
      if (all(item.label)) {
        hit = { ...base, matched: 'label' };
        score = item.label.startsWith(terms[0] ?? '') ? 0 : 1;
      } else if (all(item.group)) {
        hit = { ...base, matched: 'group' };
        score = 2;
      } else {
        const choice = item.choices.find((c) => terms.some((term) => c.folded.includes(term)));
        hit = choice ? { ...base, matched: 'choice', choice: choice.label } : { ...base, matched: 'help' };
        score = choice ? 3 : 4;
      }
      scored.push({ hit, score, order });
    });
    scored.sort((a, b) => a.score - b.score || a.order - b.order);
    return [...pages, ...scored.slice(0, limit).map((s) => s.hit)];
  }
}
