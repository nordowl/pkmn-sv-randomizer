// Browser stand-in for the Tauri backend, loaded only by `npm run dev` outside Tauri.
// Options with ids starting `demo.` are fake: they exist so every control kind, requirement and
// tag can be seen before the real sections land. Pick an environment with ?scenario=
// ready (default), nokeys, nogames, wrongupdate or noeden.

import type {
  ActivateResult,
  Api,
  AppConfig,
  ChangeLine,
  DecodedShare,
  Edition,
  Environment,
  FileFilter,
  GameStatus,
  JobSummary,
  Kind,
  Normalized,
  OptionDef,
  OptValue,
  PoolCounts,
  PoolFilter,
  Preset,
  RandoEvent,
  RandomizeRequest,
  Registry,
  ResolvedPath,
  SeedEntry,
  Settings,
  SpeciesEntry,
  SpeciesPick,
  SwitchCheck,
} from './api';

const scenario = new URLSearchParams(location.search).get('scenario') ?? 'ready';

const wait = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

type Extra = Partial<Pick<OptionDef, 'help' | 'requires' | 'sources' | 'status'>>;

const BALLS: [string, string, string][] = [
  ['unchanged', 'Unchanged', 'A Poké Ball, as usual.'],
  ['random', 'Random', 'Any Poké Ball type.'],
  ['poke_ball', 'Poké Ball', ''],
  ['great_ball', 'Great Ball', ''],
  ['ultra_ball', 'Ultra Ball', ''],
  ['master_ball', 'Master Ball', ''],
  ['premier_ball', 'Premier Ball', ''],
  ['luxury_ball', 'Luxury Ball', ''],
  ['cherish_ball', 'Cherish Ball', ''],
  ['heal_ball', 'Heal Ball', ''],
  ['net_ball', 'Net Ball', ''],
  ['dive_ball', 'Dive Ball', ''],
  ['nest_ball', 'Nest Ball', ''],
  ['repeat_ball', 'Repeat Ball', ''],
  ['timer_ball', 'Timer Ball', ''],
  ['dusk_ball', 'Dusk Ball', ''],
  ['quick_ball', 'Quick Ball', ''],
  ['safari_ball', 'Safari Ball', ''],
  ['fast_ball', 'Fast Ball', ''],
  ['level_ball', 'Level Ball', ''],
  ['lure_ball', 'Lure Ball', ''],
  ['heavy_ball', 'Heavy Ball', ''],
  ['love_ball', 'Love Ball', ''],
  ['friend_ball', 'Friend Ball', ''],
  ['moon_ball', 'Moon Ball', ''],
  ['sport_ball', 'Sport Ball', ''],
  ['dream_ball', 'Dream Ball', ''],
  ['beast_ball', 'Beast Ball', ''],
];

function opt(id: string, page: string, group: string, label: string, kind: Kind, extra: Extra = {}): OptionDef {
  return { id, page, group, label, help: '', kind, requires: [], sources: [], status: 'data', ...extra };
}

const toggle = (value: boolean): Kind => ({ type: 'toggle', default: value });
const choice = (value: string, choices: [string, string, string?][]): Kind => ({
  type: 'choice',
  default: value,
  choices: choices.map(([v, label, help]) => ({ value: v, label, help: help ?? '' })),
});
const number = (value: number, min: number, max: number, step: number, unit: string): Kind => ({
  type: 'number',
  default: value,
  min,
  max,
  step,
  unit,
});
const POOL: Kind = { type: 'pool' };
const SPECIES: Kind = { type: 'species' };
const text = (value: string, max_len: number): Kind => ({ type: 'text', default: value, max_len });

const WILD_MODES: [string, string, string][] = [
  ['swap', 'Row swap', 'Replaces each species but keeps its areas, biomes and levels.'],
  ['global', 'Global 1-to-1', 'Each species always becomes the same replacement, everywhere.'],
  ['area', 'Area 1-to-1', 'Each species becomes one replacement per area.'],
  ['shuffle', 'Full shuffle', 'Any species can appear anywhere.'],
  ['themed', 'Type-themed areas', 'Every area gets one random type, and its Pokémon share it.'],
];

const OPTIONS: OptionDef[] = [
  opt('pool.global', 'pool', 'Global pool', 'Species every section may use', POOL, {
    help: 'Generations, legendaries, Paradox Pokémon, evolution stages, forms and regional dexes. Each section can override this.',
    sources: ['upr', 'sv'],
  }),
  opt('pool.allow_tera_locked', 'pool', 'Global pool', 'Allow Ogerpon and Terapagos', toggle(false), {
    help: 'The SV reference warns not to Terastallize these two after catching them. Off keeps them out of every random pool.',
    sources: ['sv'],
    status: 'test',
  }),

  opt('demo.stats.mode', 'pokemon', 'Base stats', 'Base stats', choice('unchanged', [
    ['unchanged', 'Unchanged', 'Every Pokémon keeps its own base stats.'],
    ['shuffle', 'Shuffled', 'Each Pokémon’s six stats trade places, so its total stays the same.'],
    ['random', 'Random', 'New stats are rolled for every Pokémon.'],
  ]), { help: 'How each Pokémon’s HP, Attack, Defense, Sp. Atk, Sp. Def and Speed are set.', sources: ['upr', 'sv'] }),
  opt('demo.stats.keep_total', 'pokemon', 'Base stats', 'Keep each Pokémon’s base stat total', toggle(true), {
    help: 'New stats add up to the same total as before, so strong Pokémon stay strong.',
    requires: [{ when: 'is', id: 'demo.stats.mode', value: 'random' }],
    sources: ['sv'],
  }),
  opt('demo.stats.follow', 'pokemon', 'Base stats', 'Stats follow evolutions', toggle(true), {
    help: 'An evolved Pokémon keeps the stat spread of its earlier stage, scaled up.',
    requires: [{ when: 'not', id: 'demo.stats.mode', value: 'unchanged' }],
    sources: ['upr'],
  }),
  opt('demo.types.mode', 'pokemon', 'Types', 'Types', choice('unchanged', [
    ['unchanged', 'Unchanged', 'Every Pokémon keeps its types.'],
    ['follow', 'Random, following evolutions', 'Evolution lines share their new types.'],
    ['random', 'Fully random', 'Every Pokémon rolls its own types.'],
  ]), { sources: ['upr', 'sv'] }),
  opt('demo.types.dual', 'pokemon', 'Types', 'Force dual typing', toggle(false), {
    help: 'Every Pokémon gets two types.',
    requires: [{ when: 'not', id: 'demo.types.mode', value: 'unchanged' }],
    sources: ['upr', 'sv'],
  }),
  opt('demo.abilities.random', 'pokemon', 'Abilities', 'Randomize abilities', toggle(false), { sources: ['upr', 'sv'] }),
  opt('demo.abilities.no_wonder_guard', 'pokemon', 'Abilities', 'Ban Wonder Guard', toggle(true), {
    help: 'Wonder Guard can make a Pokémon impossible to hit. Keeping it out avoids fights you cannot win.',
    requires: [{ when: 'on', id: 'demo.abilities.random' }],
    sources: ['upr', 'sv'],
  }),
  opt('demo.catch.min', 'pokemon', 'Catching', 'Minimum catch rate', number(0, 0, 255, 5, ''), {
    help: 'Raises every Pokémon’s catch rate to at least this value. 0 keeps the game’s own rates.',
    sources: ['upr'],
  }),

  opt('demo.starters.mode', 'starters', 'Starters', 'Starter Pokémon', choice('unchanged', [
    ['unchanged', 'Unchanged', 'Sprigatito, Fuecoco and Quaxly, as usual.'],
    ['custom', 'Custom', 'You choose all three below.'],
    ['random', 'Random', 'Any species from the starter pool.'],
    ['basics', 'Random basics', 'Random first-stage Pokémon that evolve twice.'],
  ]), { sources: ['upr', 'sv'], status: 'test' }),
  opt('demo.starters.grass', 'starters', 'Sprigatito’s slot', 'Species and form', SPECIES, {
    help: 'The Pokémon in this slot. Leave it on random to roll one from the starter pool.',
    requires: [{ when: 'is', id: 'demo.starters.mode', value: 'custom' }],
  }),
  opt('demo.starters.grass.gender', 'starters', 'Sprigatito’s slot', 'Gender', choice('random', [
    ['random', 'Random', 'The game rolls the gender as usual.'],
    ['male', 'Male', 'Ignored for female-only and genderless species.'],
    ['female', 'Female', 'Ignored for male-only and genderless species.'],
  ]), {
    help: 'The gender of this starter.',
    requires: [{ when: 'is', id: 'demo.starters.mode', value: 'custom' }],
  }),
  opt('demo.starters.grass.ball', 'starters', 'Sprigatito’s slot', 'Poké Ball', choice('unchanged', BALLS), {
    help: 'The ball this starter comes in.',
  }),
  opt('demo.starters.fire', 'starters', 'Fuecoco’s slot', 'Species and form', SPECIES, {
    help: 'The Pokémon in this slot. Leave it on random to roll one from the starter pool.',
    requires: [{ when: 'is', id: 'demo.starters.mode', value: 'custom' }],
  }),
  opt('demo.starters.fire.gender', 'starters', 'Fuecoco’s slot', 'Gender', choice('random', [
    ['random', 'Random', 'The game rolls the gender as usual.'],
    ['male', 'Male', 'Ignored for female-only and genderless species.'],
    ['female', 'Female', 'Ignored for male-only and genderless species.'],
  ]), {
    help: 'The gender of this starter.',
    requires: [{ when: 'is', id: 'demo.starters.mode', value: 'custom' }],
  }),
  opt('demo.starters.fire.ball', 'starters', 'Fuecoco’s slot', 'Poké Ball', choice('unchanged', BALLS), {
    help: 'The ball this starter comes in.',
  }),
  opt('demo.starters.water', 'starters', 'Quaxly’s slot', 'Species and form', SPECIES, {
    help: 'The Pokémon in this slot. Leave it on random to roll one from the starter pool.',
    requires: [{ when: 'is', id: 'demo.starters.mode', value: 'custom' }],
  }),
  opt('demo.starters.water.gender', 'starters', 'Quaxly’s slot', 'Gender', choice('random', [
    ['random', 'Random', 'The game rolls the gender as usual.'],
    ['male', 'Male', 'Ignored for female-only and genderless species.'],
    ['female', 'Female', 'Ignored for male-only and genderless species.'],
  ]), {
    help: 'The gender of this starter.',
    requires: [{ when: 'is', id: 'demo.starters.mode', value: 'custom' }],
  }),
  opt('demo.starters.water.ball', 'starters', 'Quaxly’s slot', 'Poké Ball', choice('unchanged', BALLS), {
    help: 'The ball this starter comes in.',
  }),
  opt('demo.starters.pool', 'starters', 'Starters', 'Starter pool', POOL, {
    help: 'Which species can be rolled as a starter.',
    requires: [{ when: 'not', id: 'demo.starters.mode', value: 'unchanged' }],
    sources: ['sv'],
  }),
  opt('demo.starters.shiny', 'starters', 'Extras', 'Shiny chance', number(0, 0, 100, 5, '%'), {
    help: 'Chance for each starter to be shiny.',
    sources: ['sv'],
    status: 'test',
  }),
  opt('demo.starters.reveal', 'starters', 'Extras', 'Show the new starters', choice('nemona', [
    ['clavell', 'At Clavell’s visit', 'The cutscene at your house already shows them.'],
    ['nemona', 'At Nemona’s house', 'Before that, cutscenes show the original three.'],
    ['after', 'After choosing', 'You find out only once your starter is yours.'],
  ]), { sources: ['sv'], status: 'test' }),

  opt('demo.wild.same', 'wild', 'All regions', 'Use Paldea settings for all regions', toggle(false), {
    help: 'Kitakami and Blueberry follow the Paldea settings below.',
    sources: ['sv'],
  }),
  opt('demo.wild.paldea.on', 'wild', 'Paldea', 'Randomize wild Pokémon', toggle(false), {
    help: 'Replaces the wild Pokémon that roam Paldea.',
    sources: ['upr', 'sv'],
  }),
  opt('demo.wild.paldea.mode', 'wild', 'Paldea', 'How species are replaced', choice('swap', WILD_MODES), {
    requires: [{ when: 'on', id: 'demo.wild.paldea.on' }],
    sources: ['upr', 'sv'],
  }),
  opt('demo.wild.paldea.level', 'wild', 'Paldea', 'Level change', number(0, -50, 100, 5, '%'), {
    help: 'Raises or lowers every wild level by this percentage.',
    requires: [{ when: 'on', id: 'demo.wild.paldea.on' }],
    sources: ['upr'],
  }),
  opt('demo.wild.paldea.pool', 'wild', 'Paldea', 'Paldea wild pool', POOL, {
    help: 'Which species can appear in the wild in Paldea.',
    requires: [{ when: 'on', id: 'demo.wild.paldea.on' }],
    sources: ['sv'],
  }),
  opt('demo.wild.kitakami.on', 'wild', 'Kitakami', 'Randomize wild Pokémon in Kitakami', toggle(false), {
    requires: [{ when: 'off', id: 'demo.wild.same' }],
    sources: ['upr', 'sv'],
  }),
  opt('demo.wild.kitakami.mode', 'wild', 'Kitakami', 'How species are replaced in Kitakami', choice('swap', WILD_MODES), {
    requires: [{ when: 'off', id: 'demo.wild.same' }, { when: 'on', id: 'demo.wild.kitakami.on' }],
    sources: ['upr', 'sv'],
  }),
  opt('demo.wild.blueberry.on', 'wild', 'Blueberry Academy', 'Randomize wild Pokémon in the Terarium', toggle(false), {
    requires: [{ when: 'off', id: 'demo.wild.same' }],
    sources: ['upr', 'sv'],
  }),
  opt('demo.wild.blueberry.movers', 'wild', 'Blueberry Academy', 'Fliers and swimmers spawn only where they can move', toggle(true), {
    help: 'Keeps swimming Pokémon out of dry biomes and fliers out of caves.',
    requires: [{ when: 'on', id: 'demo.wild.blueberry.on' }],
    sources: ['sv'],
    status: 'test',
  }),

  opt('demo.trainers.mode', 'trainers', 'Teams', 'Trainer Pokémon', choice('unchanged', [
    ['unchanged', 'Unchanged', 'Trainers keep their usual teams.'],
    ['random', 'Random', 'Every trainer Pokémon is rolled from the pool.'],
    ['even', 'Even spread', 'Random, but every allowed species is used about as often.'],
  ]), { sources: ['upr', 'sv'] }),
  opt('demo.trainers.level', 'trainers', 'Teams', 'Level change', number(0, -50, 100, 5, '%'), {
    help: 'Raises or lowers every trainer Pokémon’s level by this percentage.',
    sources: ['upr', 'pknx'],
  }),
  opt('demo.trainers.extra', 'trainers', 'Teams', 'Extra Pokémon per team', number(0, 0, 5, 1, 'Pokémon'), {
    help: 'Adds Pokémon to each team, up to six.',
    sources: ['upr', 'sv', 'pknx'],
    status: 'test',
  }),
  opt('demo.trainers.names', 'trainers', 'Names', 'Random trainer names', toggle(false), { sources: ['upr'], status: 'test' }),
  opt('demo.trainers.name_list', 'trainers', 'Names', 'Your own name list', text('', 400), {
    help: 'Names separated by commas. Leave it empty to use the built-in list.',
    requires: [{ when: 'on', id: 'demo.trainers.names' }],
    sources: ['upr'],
    status: 'test',
  }),
  opt('demo.trainers.rival', 'trainers', 'Names', 'Rival’s title', text('Rival', 24), { sources: ['new'] }),

  opt('demo.raids.same', 'raids', 'All regions', 'Use Paldea settings for all regions', toggle(false), { sources: ['sv'] }),
  opt('demo.raids.paldea', 'raids', 'Paldea raids', 'Randomize raid bosses', toggle(false), { sources: ['sv'] }),
  opt('demo.raids.paldea.scale', 'raids', 'Paldea raids', 'Bosses grow stronger with star tier', toggle(true), {
    requires: [{ when: 'on', id: 'demo.raids.paldea' }],
    sources: ['new'],
  }),
  opt('demo.raids.kitakami', 'raids', 'Kitakami raids', 'Randomize raid bosses in Kitakami', toggle(false), {
    requires: [{ when: 'off', id: 'demo.raids.same' }],
    sources: ['sv'],
  }),
  opt('demo.raids.blueberry', 'raids', 'Blueberry raids', 'Randomize raid bosses in the Terarium', toggle(false), {
    requires: [{ when: 'off', id: 'demo.raids.same' }],
    sources: ['sv'],
  }),

  opt('demo.exp.models', 'experimental', 'Missing species', 'Borrow Legends Arceus models', toggle(false), {
    help: 'Adds up to 55 species that are not in the game, using models from Legends Arceus. Needs your own dump of that game.',
    sources: ['new'],
    status: 'research',
  }),
];

const PAGES = [
  ['pool', 'Species pool'],
  ['pokemon', 'Stats, types & abilities'],
  ['evolutions', 'Evolutions'],
  ['moves', 'Moves & TMs'],
  ['starters', 'Starters'],
  ['gifts_trades', 'Gifts & trades'],
  ['wild', 'Wild Pokémon'],
  ['static', 'Static encounters'],
  ['bosses', 'Titans & legendaries'],
  ['trainers', 'Trainers'],
  ['raids', 'Tera Raids'],
  ['items', 'Items'],
  ['shops', 'Shops'],
  ['extras', 'Other content'],
  ['experimental', 'Experimental'],
].map(([id, title]) => ({ id: id!, title: title! }));

const everything = (): PoolFilter => ({
  inherit: false,
  generations: Array(9).fill(true),
  legendaries: Array(9).fill(true),
  paradox: true,
  stages: [true, true, true, true],
  alt_forms: true,
  irregular_forms: false,
  regional_dex: [false, false, false],
  banned: [],
  allowed: [],
});

function defaultValue(def: OptionDef): OptValue {
  switch (def.kind.type) {
    case 'toggle':
    case 'choice':
    case 'number':
    case 'text':
      return def.kind.default;
    case 'pool':
      return { ...everything(), inherit: true };
    case 'species':
      return { national: null, form: 0 };
  }
}

const defaults = (): Settings => ({ values: Object.fromEntries(OPTIONS.map((def) => [def.id, defaultValue(def)])) });

const PRESETS: [string, string, string, Partial<PoolFilter>, boolean][] = [
  ['balanced', 'Balanced', 'A fair, varied run. Legendary and mythical Pokémon stay out of the random pools, so they never turn up early. Everything else is in play.', { legendaries: Array(9).fill(false) }, false],
  ['chaos', 'Chaos', 'Anything goes. Every species and form can turn up anywhere, legendaries, Paradox Pokémon, Ogerpon and Terapagos included.', { irregular_forms: true }, true],
  ['hard', 'Hard', 'A tougher run for experienced players. Strong species, legendaries and Paradox Pokémon included, stay in the pools, so the opposition can field them too.', {}, false],
  ['light', 'Light', 'A gentle shake-up that keeps the game familiar. Only Pokémon from the Paldea, Kitakami and Blueberry dexes are used, without legendaries or Paradox Pokémon.', { legendaries: Array(9).fill(false), paradox: false, regional_dex: [true, true, true] }, false],
];

function presets(): Preset[] {
  return PRESETS.map(([id, name, description, pool, teraLocked]) => {
    const settings = defaults();
    settings.values['pool.global'] = { ...everything(), inherit: true, ...pool };
    settings.values['pool.allow_tera_locked'] = teraLocked;
    return { id, name, description, settings };
  });
}

function isPool(value: unknown): value is PoolFilter {
  const v = value as PoolFilter;
  return typeof v === 'object' && v !== null && typeof v.inherit === 'boolean' && Array.isArray(v.generations) && Array.isArray(v.banned);
}

function isSpecies(value: unknown): value is SpeciesPick {
  const v = value as SpeciesPick;
  return typeof v === 'object' && v !== null && 'national' in v && typeof v.form === 'number';
}

function accept(def: OptionDef, value: OptValue): OptValue | null {
  const kind = def.kind;
  switch (kind.type) {
    case 'toggle':
      return typeof value === 'boolean' ? value : null;
    case 'choice':
      return typeof value === 'string' && kind.choices.some((c) => c.value === value) ? value : null;
    case 'number':
      return typeof value === 'number' && Number.isInteger(value) ? Math.min(kind.max, Math.max(kind.min, value)) : null;
    case 'text':
      return typeof value === 'string' ? [...value].slice(0, kind.max_len).join('') : null;
    case 'pool':
      return isPool(value) ? value : null;
    case 'species':
      return isSpecies(value) ? value : null;
  }
}

function normalized(settings: Settings): Normalized {
  const warnings: string[] = [];
  const values: Record<string, OptValue> = {};
  for (const def of OPTIONS) {
    const stored = settings.values[def.id];
    if (stored === undefined) {
      values[def.id] = defaultValue(def);
      continue;
    }
    const value = accept(def, stored);
    if (value === null) warnings.push(`option \`${def.id}\` had an invalid value and was reset`);
    values[def.id] = value ?? defaultValue(def);
  }
  for (const id of Object.keys(settings.values)) {
    if (!(id in values)) warnings.push(`unknown option \`${id}\` was ignored`);
  }
  return { settings: { values }, warnings };
}

type Row = [national: number, form: number, name: string, gen: number, legendary: 0 | 1, paradox: 0 | 1, stage: number, dex: string];

// dex letters: P Paldea, K Kitakami, B Blueberry
const ROWS: Row[] = [
  [1, 0, 'Bulbasaur', 1, 0, 0, 1, 'B'], [3, 0, 'Venusaur', 1, 0, 0, 3, 'B'],
  [25, 0, 'Pikachu', 1, 0, 0, 2, 'P'], [26, 0, 'Raichu', 1, 0, 0, 3, 'P'], [26, 1, 'Raichu', 1, 0, 0, 3, ''],
  [37, 0, 'Vulpix', 1, 0, 0, 1, 'K'], [37, 1, 'Vulpix', 1, 0, 0, 1, 'K'],
  [128, 1, 'Tauros', 1, 0, 0, 0, 'P'], [128, 2, 'Tauros', 1, 0, 0, 0, 'P'], [128, 3, 'Tauros', 1, 0, 0, 0, 'P'],
  [129, 0, 'Magikarp', 1, 0, 0, 1, 'P'], [130, 0, 'Gyarados', 1, 0, 0, 3, 'P'], [133, 0, 'Eevee', 1, 0, 0, 1, 'P'],
  [144, 0, 'Articuno', 1, 1, 0, 0, 'B'], [150, 0, 'Mewtwo', 1, 1, 0, 0, ''], [151, 0, 'Mew', 1, 1, 0, 0, ''],
  [194, 0, 'Wooper', 2, 0, 0, 1, 'K'], [194, 1, 'Wooper', 2, 0, 0, 1, 'P'], [212, 0, 'Scizor', 2, 0, 0, 3, 'P'],
  [248, 0, 'Tyranitar', 2, 0, 0, 3, 'PB'], [249, 0, 'Lugia', 2, 1, 0, 0, ''], [250, 0, 'Ho-Oh', 2, 1, 0, 0, ''],
  [280, 0, 'Ralts', 3, 0, 0, 1, 'P'], [282, 0, 'Gardevoir', 3, 0, 0, 3, 'P'], [334, 0, 'Altaria', 3, 0, 0, 3, 'P'],
  [384, 0, 'Rayquaza', 3, 1, 0, 0, ''],
  [443, 0, 'Gible', 4, 0, 0, 1, 'P'], [445, 0, 'Garchomp', 4, 0, 0, 3, 'P'], [448, 0, 'Lucario', 4, 0, 0, 3, 'P'],
  [479, 0, 'Rotom', 4, 0, 0, 0, 'P'], [479, 1, 'Rotom', 4, 0, 0, 0, 'P'], [479, 2, 'Rotom', 4, 0, 0, 0, 'P'],
  [487, 0, 'Giratina', 4, 1, 0, 0, ''], [487, 1, 'Giratina', 4, 1, 0, 0, ''], [493, 0, 'Arceus', 4, 1, 0, 0, ''],
  [570, 1, 'Zorua', 5, 0, 0, 1, ''], [571, 0, 'Zoroark', 5, 0, 0, 3, 'P'], [635, 0, 'Hydreigon', 5, 0, 0, 3, 'B'],
  [643, 0, 'Reshiram', 5, 1, 0, 0, 'B'],
  [658, 0, 'Greninja', 6, 0, 0, 3, 'P'], [700, 0, 'Sylveon', 6, 0, 0, 3, 'P'], [718, 0, 'Zygarde', 6, 1, 0, 0, ''],
  [778, 0, 'Mimikyu', 7, 0, 0, 0, 'P'], [784, 0, 'Kommo-o', 7, 0, 0, 3, 'B'], [791, 0, 'Solgaleo', 7, 1, 0, 0, 'B'],
  [887, 0, 'Dragapult', 8, 0, 0, 3, 'B'],
  [906, 0, 'Sprigatito', 9, 0, 0, 1, 'P'], [908, 0, 'Meowscarada', 9, 0, 0, 3, 'P'], [909, 0, 'Fuecoco', 9, 0, 0, 1, 'P'],
  [912, 0, 'Quaxly', 9, 0, 0, 1, 'P'], [925, 0, 'Maushold', 9, 0, 0, 3, 'P'], [925, 1, 'Maushold', 9, 0, 0, 3, 'P'],
  [937, 0, 'Ceruledge', 9, 0, 0, 3, 'P'], [964, 0, 'Palafin', 9, 0, 0, 3, 'P'], [978, 0, 'Tatsugiri', 9, 0, 0, 0, 'P'],
  [978, 1, 'Tatsugiri', 9, 0, 0, 0, 'P'], [978, 2, 'Tatsugiri', 9, 0, 0, 0, 'P'],
  [984, 0, 'Great Tusk', 9, 0, 1, 0, 'P'], [986, 0, 'Brute Bonnet', 9, 0, 1, 0, 'P'], [990, 0, 'Iron Treads', 9, 0, 1, 0, 'P'],
  [1001, 0, 'Wo-Chien', 9, 1, 0, 0, 'P'], [1007, 0, 'Koraidon', 9, 1, 0, 0, 'P'], [1008, 0, 'Miraidon', 9, 1, 0, 0, 'P'],
  [1009, 0, 'Walking Wake', 9, 1, 1, 0, ''], [1011, 0, 'Dipplin', 9, 0, 0, 2, 'K'], [1017, 0, 'Ogerpon', 9, 1, 0, 0, 'K'],
  [1017, 1, 'Ogerpon', 9, 1, 0, 0, 'K'], [1019, 0, 'Hydrapple', 9, 0, 0, 3, 'B'], [1024, 0, 'Terapagos', 9, 1, 0, 0, 'B'],
  [1025, 0, 'Pecharunt', 9, 1, 0, 0, ''],
];

const SPECIES_LIST: SpeciesEntry[] = ROWS.map(([national, form, name, generation, legendary, paradox, stage, dex]) => ({
  national,
  form,
  species_id: national,
  name,
  generation,
  legendary: legendary === 1,
  paradox: paradox === 1,
  stage,
  regional: [dex.includes('P'), dex.includes('K'), dex.includes('B')],
}));

const NEVER_RANDOM = [1007, 1008];
const TERA_LOCKED = [1017, 1024];

function matches(entry: SpeciesEntry, filter: PoolFilter): boolean {
  if (filter.allowed.includes(entry.national)) return true;
  if (filter.banned.includes(entry.national)) return false;
  const gen = Math.min(9, Math.max(1, entry.generation)) - 1;
  if (!filter.generations[gen]) return false;
  if (entry.legendary && !filter.legendaries[gen]) return false;
  if (entry.paradox && !filter.paradox) return false;
  if (entry.form !== 0 && !filter.alt_forms) return false;
  if (!filter.stages[entry.stage]) return false;
  if (filter.regional_dex.some(Boolean) && !filter.regional_dex.some((ticked, i) => ticked && entry.regional[i])) return false;
  return true;
}

const STAGE_WORDS = ['single-stage', 'first-stage', 'middle-stage', 'final-stage'];
const DEX_WORDS = ['Paldea', 'Kitakami', 'Blueberry'];

function joined(items: string[], word: string): string {
  if (items.length <= 1) return items.join('');
  return `${items.slice(0, -1).join(', ')} ${word} ${items[items.length - 1]}`;
}

function describePool(filter: PoolFilter, isGlobal: boolean): string {
  if (filter.inherit && !isGlobal) return 'Uses the global pool';
  const parts: string[] = [];
  const gens = (test: (i: number) => boolean) => [...Array(9).keys()].filter(test).map((i) => String(i + 1));
  const offGens = gens((i) => !filter.generations[i]);
  if (offGens.length === 9) parts.push('no generations at all');
  else if (offGens.length) parts.push(`no Gen ${joined(offGens, 'or')} Pokémon`);
  const offLegends = gens((i) => !!filter.generations[i] && !filter.legendaries[i]);
  if (offLegends.length) {
    parts.push(offLegends.length === 9 - offGens.length ? 'no legendaries' : `no legendaries from Gen ${joined(offLegends, 'or')}`);
  }
  if (!filter.paradox) parts.push('no Paradox Pokémon');
  const offStages = STAGE_WORDS.filter((_, i) => !filter.stages[i]);
  if (offStages.length) parts.push(`no ${joined(offStages, 'or')} Pokémon`);
  if (!filter.alt_forms) parts.push('no alternate forms');
  if (filter.irregular_forms) parts.push('irregular forms allowed');
  const dexes = DEX_WORDS.filter((_, i) => filter.regional_dex[i]);
  if (dexes.length) parts.push(`only Pokémon in the ${joined(dexes, 'and')} ${dexes.length === 1 ? 'dex' : 'dexes'}`);
  if (filter.banned.length) parts.push(`${filter.banned.length} species banned`);
  if (filter.allowed.length) parts.push(`${filter.allowed.length} species always allowed`);
  if (!parts.length) return 'All species';
  parts[0] = parts[0]!.charAt(0).toUpperCase() + parts[0]!.slice(1);
  return parts.join('; ');
}

function describeValue(def: OptionDef, value: OptValue, names: boolean): string {
  const kind = def.kind;
  switch (kind.type) {
    case 'toggle':
      return value ? 'On' : 'Off';
    case 'choice':
      return kind.choices.find((c) => c.value === value)?.label ?? String(value);
    case 'number': {
      const n = value as number;
      if (!kind.unit) return String(n);
      return /[\p{L}\p{N}]/u.test(kind.unit) ? `${n} ${kind.unit}` : `${n}${kind.unit}`;
    }
    case 'text':
      return value === '' ? '(empty)' : String(value);
    case 'pool':
      return describePool(value as PoolFilter, def.id === 'pool.global');
    case 'species': {
      const pick = value as SpeciesPick;
      if (pick.national === null) return 'Random';
      const entry = names ? SPECIES_LIST.find((e) => e.national === pick.national) : undefined;
      const name = entry?.name ?? `No. ${pick.national}`;
      return pick.form === 0 ? name : `${name} (form ${pick.form})`;
    }
  }
}

const APPDATA = 'C:\\Users\\Player\\AppData\\Roaming';
const EDEN_EXE = 'D:\\Emulators\\Eden\\eden.exe';
const EDEN_DATA = `${APPDATA}\\eden`;
const KEYS = `${EDEN_DATA}\\keys\\prod.keys`;
const LOAD = `${EDEN_DATA}\\load`;
const NAND = `${EDEN_DATA}\\nand`;
const APP_DATA = `${APPDATA}\\dev.svrandomizer.app`;
const LIBRARY = `${APP_DATA}\\seeds`;
const SCANNED = ['D:\\Switch\\Games', 'E:\\Switch Dumps'];
const BATCHES = 'D:\\Switch\\Batches';
const TITLE: Record<Edition, string> = { scarlet: '0100A3D008C5C000', violet: '01008F6008C5E000' };
const NAME: Record<Edition, string> = { scarlet: 'Scarlet', violet: 'Violet' };

let config: AppConfig = {
  eden_exe: null,
  eden_data_dir: null,
  keys_file: null,
  load_dir: null,
  nand_dir: null,
  game_dirs: null,
  scarlet: { base: null, update: null },
  violet: { base: null, update: null },
  library_dir: null,
  extra_seed_dirs: [BATCHES],
  next_batch_index: 7,
};

function resolved(override: string | null, detected: string, found: boolean): ResolvedPath {
  if (override) return { path: override, source: 'override', exists: true };
  return { path: detected, source: found ? 'detected' : 'default', exists: found };
}

function folderList(dirs: string[]): string {
  return joined(dirs.map((d) => `"${d}"`), 'and');
}

function gameStatus(edition: Edition, keysError: string | null, dirs: string[]): GameStatus {
  const name = NAME[edition];
  const chosen = config[edition];
  const base = chosen.base ?? `${SCANNED[0]}\\Pokemon ${name}.xci`;
  const update = chosen.update ?? `${SCANNED[0]}\\Pokemon ${name} v4.0.0.nsp`;
  const status = (state: GameStatus['state'], message: string | null, files = true): GameStatus => ({
    edition,
    title_id: TITLE[edition],
    state,
    base: files ? base : null,
    update: files && state === 'ready' ? update : null,
    message,
    mod_disabled: scenario === 'ready' && edition === 'violet',
  });
  if (keysError) return status('error', `The game files can be checked once your keys file loads. ${keysError}`, false);
  const addedFolder = dirs.some((dir) => !SCANNED.includes(dir));
  if (scenario === 'nogames' && !chosen.base && !addedFolder) {
    return status('missing', `No ${name} cartridge dump (.xci) was found in ${folderList(dirs)}. Put the .xci in one of these folders, or choose it in Settings.`, false);
  }
  if (scenario === 'wrongupdate' && edition === 'scarlet' && !chosen.update) {
    const wrong = `${SCANNED[0]}\\Pokemon Scarlet v3.0.1.nsp`;
    return {
      ...status('error', `"${wrong}" is the wrong update version. The randomizer needs Scarlet version 4.0.0, but this file has build ID 6C2F9A41D07B3E85. Install or dump update 4.0.0.`),
      update: wrong,
    };
  }
  return status('ready', null);
}

function environment(): Environment {
  const keysFound = config.keys_file !== null || scenario !== 'nokeys';
  const keys_error = keysFound
    ? null
    : `No keys file was found at "${KEYS}". Copy your prod.keys there, or choose where it is in Settings.`;
  const dirs = config.game_dirs ?? SCANNED;
  const edenFound = config.eden_exe !== null || scenario !== 'noeden';
  return {
    config: structuredClone(config),
    eden: {
      exe: resolved(config.eden_exe, EDEN_EXE, edenFound),
      data_dir: resolved(config.eden_data_dir, EDEN_DATA, true),
      portable: false,
      keys: resolved(config.keys_file, KEYS, keysFound),
      load_dir: resolved(config.load_dir, LOAD, true),
      nand_dir: resolved(config.nand_dir, NAND, true),
      game_dirs: dirs,
      game_dirs_source: config.game_dirs ? 'override' : 'detected',
    },
    keys_error,
    library_dir: config.library_dir ?? LIBRARY,
    games: [gameStatus('scarlet', keys_error, dirs), gameStatus('violet', keys_error, dirs)],
  };
}

function readyGame(edition: Edition): void {
  const status = environment().games.find((g) => g.edition === edition)!;
  if (status.state !== 'ready') throw status.message ?? `${NAME[edition]} is not ready yet. Check the game files in Settings.`;
}

const now = Math.floor(Date.now() / 1000);

function seedEntry(root: string, name: string, edition: Edition, seed: string, ageHours: number, files: number, extra: Partial<SeedEntry> = {}): SeedEntry {
  return {
    path: `${root}\\${name}`,
    name,
    group: root === LIBRARY ? null : root.split('\\').pop()!,
    edition,
    title_id: TITLE[edition],
    seed,
    created_unix: now - Math.round(ageHours * 3600),
    file_count: files,
    active: false,
    has_spoiler: true,
    ...extra,
  };
}

let seeds: SeedEntry[] = [
  seedEntry(LIBRARY, 'Scarlet 4821937705', 'scarlet', '4821937705', 2.2, 412, { active: true }),
  seedEntry(LIBRARY, 'Violet nuzlocke', 'violet', 'nuzlocke', 30, 398),
  seedEntry(BATCHES, 'Run 006', 'scarlet', '9093321187', 74, 405),
  seedEntry(BATCHES, 'Run 005', 'scarlet', '1187420936', 74.05, 409),
  seedEntry(BATCHES, 'Run 004', 'scarlet', '5502918843', 74.1, 404),
  seedEntry(LIBRARY, 'Scarlet 1029384756', 'scarlet', '1029384756', 220, 377, { has_spoiler: false }),
];

const SAVES: Record<Edition, string[]> = {
  scarlet: [`${NAND}\\user\\save\\0000000000000000\\8D2F11E3A6C04B7A9E05D1C2F3B4A596\\${TITLE.scarlet}`],
  violet: [],
};

let running = false;
let cancelled = false;

function randomSeed(): string {
  return String(Math.floor(1_000_000_000 + Math.random() * 8_999_999_999));
}

async function runJob(request: RandomizeRequest, emit: (event: RandoEvent) => void): Promise<JobSummary> {
  const batch = request.batch;
  if (batch) {
    if (batch.count < 1 || batch.count > 999) throw 'A batch makes between 1 and 999 seeds. Change the number of seeds.';
    if (!batch.prefix.trim()) throw 'Give the batch a name, for example "Run ", so its seed folders can be told apart.';
  }
  readyGame(request.edition);
  const root = batch?.output_dir ?? config.library_dir ?? LIBRARY;
  const typed = request.seed.trim();
  const total = batch ? batch.count : 1;
  const summary: JobSummary = { total, done: 0, failed: 0, cancelled: false, folders: [], next_batch_index: config.next_batch_index };
  emit({ kind: 'started', total });

  for (let position = 0; position < total; position++) {
    if (cancelled) {
      summary.cancelled = true;
      break;
    }
    const index = (batch?.start_index ?? 0) + position;
    const seed = typed ? (batch ? `${typed}-${index}` : typed) : randomSeed();
    const name = batch ? `${batch.prefix.trimStart()}${String(index).padStart(3, '0')}` : `${NAME[request.edition]} ${seed}`;
    const started = performance.now();
    emit({ kind: 'seed_started', position, name, seed });
    emit({ kind: 'seed_stage', position, stage: 'randomizing' });
    await wait(1100);
    if (batch && total >= 4 && position === 2) {
      summary.failed++;
      emit({
        kind: 'seed_failed',
        position,
        name,
        seed,
        error: 'The randomizer stopped: the species pool for wild Pokémon in Kitakami is empty. Loosen its filters.',
      });
      continue;
    }
    emit({ kind: 'seed_stage', position, stage: 'logging' });
    await wait(500);
    emit({ kind: 'seed_stage', position, stage: 'writing' });
    await wait(700);
    const folder = `${root}\\${name}`;
    summary.done++;
    summary.folders.push(folder);
    seeds = [seedEntry(root, name, request.edition, seed, 0, 400 + Math.floor(Math.random() * 20)), ...seeds];
    emit({ kind: 'seed_done', position, name, seed, folder, warnings: [], millis: Math.round(performance.now() - started) });
  }

  if (batch) {
    summary.next_batch_index = batch.start_index + summary.done + summary.failed;
    config.next_batch_index = summary.next_batch_index;
    if (batch.output_dir && batch.output_dir !== (config.library_dir ?? LIBRARY) && !config.extra_seed_dirs.includes(batch.output_dir)) {
      config.extra_seed_dirs.push(batch.output_dir);
    }
  }
  return summary;
}

function toBase64Url(textValue: string): string {
  const bytes = new TextEncoder().encode(textValue);
  let binary = '';
  bytes.forEach((b) => (binary += String.fromCharCode(b)));
  return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

function fromBase64Url(code: string): string {
  const binary = atob(code.replace(/-/g, '+').replace(/_/g, '/'));
  return new TextDecoder().decode(Uint8Array.from(binary, (c) => c.charCodeAt(0)));
}

function pickedFile(filters: FileFilter[], title: string): string {
  const extensions = filters.flatMap((f) => f.extensions);
  const edition = /violet/i.test(title) ? 'Violet' : 'Scarlet';
  if (extensions.includes('keys')) return 'C:\\Users\\Player\\Downloads\\prod.keys';
  if (extensions.includes('xci')) return `E:\\Switch Dumps\\Pokemon ${edition}.xci`;
  if (extensions.includes('nsp')) return `E:\\Switch Dumps\\Pokemon ${edition} v4.0.0.nsp`;
  if (extensions.includes('exe')) return 'C:\\Emulators\\Eden\\eden.exe';
  return 'C:\\Users\\Player\\Documents\\Randomizer\\nuzlocke-settings.json';
}

export const mockApi: Api = {
  getEnvironment: async () => {
    await wait(scenario === 'ready' ? 450 : 300);
    return environment();
  },
  saveConfig: async (next) => {
    await wait(250);
    config = structuredClone(next);
    return environment();
  },

  getRegistry: async () => {
    await wait(120);
    const registry: Registry = { pages: PAGES, options: OPTIONS, defaults: defaults(), presets: presets() };
    return structuredClone(registry);
  },
  normalizeSettings: async (settings) => normalized(settings),
  describeChanges: async (settings, edition) => {
    await wait(160);
    const names = edition !== null && environment().games.find((g) => g.edition === edition)?.state === 'ready';
    const { settings: clean } = normalized(settings);
    const lines: ChangeLine[] = [];
    for (const def of OPTIONS) {
      const value = clean.values[def.id]!;
      const fallback = defaultValue(def);
      const shown = describeValue(def, value, names);
      const shownDefault = describeValue(def, fallback, names);
      if (JSON.stringify(value) !== JSON.stringify(fallback) && shown !== shownDefault) {
        lines.push({ id: def.id, page: def.page, group: def.group, label: def.label, value: shown, default: shownDefault });
      }
    }
    return lines;
  },
  loadSettingsFile: async () => {
    await wait(200);
    const settings = defaults();
    settings.values['pool.global'] = { ...everything(), inherit: true, legendaries: Array(9).fill(false), banned: [25, 133, 778] };
    settings.values['demo.wild.paldea.on'] = true;
    settings.values['demo.wild.paldea.level'] = 20;
    settings.values['demo.trainers.mode'] = 'random';
    settings.values['wild.old_mode'] = 'swap';
    return normalized(settings);
  },
  saveSettingsFile: async () => {
    await wait(200);
  },
  encodeShareCode: async (settings, seed) => {
    const { settings: clean } = normalized(settings);
    const base = defaults().values;
    const changed = Object.fromEntries(Object.entries(clean.values).filter(([id, v]) => JSON.stringify(v) !== JSON.stringify(base[id])));
    const trimmed = seed?.trim();
    return `SVR1-${toBase64Url(JSON.stringify({ v: changed, ...(trimmed ? { s: trimmed } : {}) }))}`;
  },
  decodeShareCode: async (code) => {
    await wait(120);
    const damaged = 'This share code is damaged or incomplete. Copy the whole code again and paste it here.';
    const trimmed = code.trim();
    if (!trimmed.startsWith('SVR1-')) throw damaged;
    let payload: { v: Record<string, OptValue>; s?: string };
    try {
      payload = JSON.parse(fromBase64Url(trimmed.slice(5)));
    } catch {
      throw damaged;
    }
    const merged = defaults();
    Object.assign(merged.values, payload.v);
    const result = normalized(merged);
    const decoded: DecodedShare = { settings: result.settings, seed: payload.s ?? null, warnings: result.warnings };
    return decoded;
  },

  listSpecies: async (edition) => {
    await wait(300);
    readyGame(edition);
    return structuredClone(SPECIES_LIST);
  },
  poolCounts: async (edition, settings) => {
    await wait(140);
    readyGame(edition);
    const { settings: clean } = normalized(settings);
    const global = clean.values['pool.global'] as PoolFilter;
    const teraLocked = clean.values['pool.allow_tera_locked'] === true;
    const counts: Record<string, number> = {};
    for (const def of OPTIONS.filter((d) => d.kind.type === 'pool')) {
      const own = clean.values[def.id] as PoolFilter;
      const filter = own.inherit ? global : own;
      counts[def.id] = SPECIES_LIST.filter(
        (e) => !NEVER_RANDOM.includes(e.national) && (teraLocked || !TERA_LOCKED.includes(e.national)) && matches(e, filter),
      ).length;
    }
    const result: PoolCounts = { counts, total: SPECIES_LIST.length };
    return result;
  },

  randomize: async (request, onEvent) => {
    if (running) throw 'A randomizer run is already going. Wait for it to finish, or cancel it first.';
    running = true;
    cancelled = false;
    try {
      return await runJob(structuredClone(request), onEvent);
    } finally {
      running = false;
    }
  },
  cancelRandomize: async () => {
    cancelled = true;
  },

  listSeeds: async () => {
    await wait(220);
    return structuredClone([...seeds].sort((a, b) => b.created_unix - a.created_unix));
  },
  checkSwitch: async (path) => {
    await wait(160);
    const seed = seeds.find((s) => s.path === path);
    if (!seed?.edition) throw `"${path}" is not a randomizer seed folder: it has no seed.json.`;
    const check: SwitchCheck = {
      edition: seed.edition,
      active_seed: seeds.find((s) => s.active && s.edition === seed.edition)?.name ?? null,
      saves: SAVES[seed.edition],
    };
    return check;
  },
  activateSeed: async (path, backupSave) => {
    await wait(700);
    const seed = seeds.find((s) => s.path === path);
    if (!seed?.edition) throw `"${path}" is not a randomizer seed folder: it has no seed.json.`;
    const edition = seed.edition;
    seeds = seeds.map((s) => (s.edition === edition ? { ...s, active: s.path === path } : s));
    const result: ActivateResult = {
      installed_to: `${LOAD}\\${TITLE[edition]}\\SV Randomizer`,
      backup_dir: backupSave && SAVES[edition].length ? `${APP_DATA}\\save-backups\\${TITLE[edition]} 2026-11-07 14-22-05` : null,
    };
    return result;
  },
  deactivate: async (edition) => {
    await wait(200);
    seeds = seeds.map((s) => (s.edition === edition ? { ...s, active: false } : s));
  },
  deleteSeed: async (path) => {
    await wait(200);
    seeds = seeds.filter((s) => s.path !== path);
  },
  openPath: async (path) => {
    await wait(60);
    console.info('[mock] open', path);
  },
  launchEden: async (edition) => {
    await wait(200);
    readyGame(edition);
    if (!environment().eden.exe.exists) throw `Eden was not found at "${EDEN_EXE}". Choose eden.exe in Settings.`;
    console.info('[mock] launch Eden', edition);
  },

  pickFile: async (title, filters) => {
    await wait(150);
    return pickedFile(filters, title);
  },
  pickFolder: async (title) => {
    await wait(150);
    return /batch|output/i.test(title) ? BATCHES : 'F:\\Switch Games';
  },
  pickSaveFile: async () => {
    await wait(150);
    return 'C:\\Users\\Player\\Documents\\Randomizer\\my-settings.json';
  },
};
