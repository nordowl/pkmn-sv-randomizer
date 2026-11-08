# Scarlet/Violet randomizer feature list

Status: waiting for your confirmation. No application code exists yet.

Target: Scarlet and Violet with update 4.0.0 and both DLCs, played on Eden.

## How to confirm

Tick or untick boxes in this file, or tell me the changes in chat. A tick means "build this option". It does not mean "on by default", because every randomization option starts off in the app unless a preset turns it on.

I ticked what I recommend. Unticked items are ideas I would skip unless you want them.

Each option has two kinds of tag.

| Tag | Meaning |
| --- | --- |
| `UPR` | Universal Pokemon Randomizer ZX has it |
| `SV` | The Gonzalooo Scarlet/Violet randomizer has it |
| `pkNX` | The pkNX data editor has it. I added this as a third reference |
| `New` | No reference has it. Your game data makes it possible |
| `data` | I found the table and its fields in your files |
| `test` | The data exists, but I must confirm the behavior in-game |
| `research` | I have not found where the game stores this yet |

An option with no status tag is app behavior that does not touch game data.

If a `test` item fails in-game and I can't fix it, I will drop it and tell you.

## What the game inspection found

I read your base XCIs and 4.0.0 update NSPs read-only, using the keys in your Eden folder. These findings shaped the list.

| Data | v0 alone | Your 4.0.0 |
| --- | --- | --- |
| Files in the game's archive index | 273,862 | 408,641 |
| Playable species | 476 | 733 |
| Usable moves | 639 of 901 | 698 of 920 |
| Items | 909 | 1,148 |
| Trainers | 538 | 756 |
| Static overworld encounters | 720 | 1,117 |
| Wild spawn rows | 449 | 956 |
| Raid rows | 454 | 700 |
| Gift entries | 8 | 13 |
| Trade entries | 3 | 33 |
| Scripted battle entries | 55 | 117 |
| Shop lineup rows | 428 | 668 |
| TM crafting rows | 171 | 229 |

**The SV reference now matches your game.** Its file index is identical to yours, at 408,641 files in 23,589 packs. All 1,424 species rows match on species, form and base stats, and every trainer ID and first-Pokémon level matches. Its DLC options are therefore back on this list. We still read everything from your files and ship no game data. The reference's tables are useful during development as a check on our parser.

**The species pool is 733.** That is 99 from Gen 1, 82 from Gen 2, 87 from Gen 3, 85 from Gen 4, 86 from Gen 5, 47 from Gen 6, 55 from Gen 7, 72 from Gen 8 and 120 from Gen 9. By regional dex, 400 are in Paldea, 200 in Kitakami and 243 in Blueberry, with overlap, and 69 are in no regional dex. Bulbasaur, Lugia, Ho-Oh, Mew and Arceus are in the pool. Victini is not.

**The DLC files are licenses.** Each DLC NSP holds a 114,688-byte stub. All DLC content is inside the update's archive, so the app never needs the DLC files. Eden needs them installed, and its NAND already has both updates and both DLCs.

**Scarlet and Violet share data.** All 290 tables I compared are byte-identical, and both games list the same 408,641 files. Version exclusives are flags inside the tables. Only the executables differ.

**Your installed mods now apply.** The Dynamic FPS and Improved Graphics patches name build IDs that match your 4.0.0 games, which are `421C5411...` for Scarlet and `709BFD66...` for Violet. They only patch game code and contain no RomFS files, so they can't collide with a randomizer mod at the file level.

**What you have checked in game.** These were played in Eden on Scarlet 4.0.0 with your code mods on:

- The early story plays through without crashes, from the starter choice to the Lechonk catching lesson. This was tested with a seed that turns on every option at once.
- The starters can be alternate forms. They show in the Nemona's-house lineup and you receive them with their ball, held item, shiny status and Tera type.
- The new starters appear only at the lineup in front of Nemona's house.
- Nemona's first battle uses the right replacement starter.
- A scripted battle and its cutscene both show the new species. This was checked on the Lechonk lesson.
- Trainer teams change.
- The Pokémon Center counter keeps its standard items and shows the first random extra, the one available without badges. No "None" item appears. This was checked at Los Platos.
- The Pokémon Center counter sells all of its extra items from the start.
- A crash at save load was traced to the randomized Gimmighoul objects. Without them, the same save loaded cleanly 10 out of 10 times.

Everything else marked `test` still waits for a check in game. `data` means the table and fields exist, not that a change works in game.

## Input and output

The app reads two files per game, the base XCI and the 4.0.0 update NSP. It refuses a base game with no update, or any other version.

The app produces a LayeredFS mod folder, not a new XCI. Eden loads these from `%APPDATA%\eden\load\<title id>\<mod name>\romfs\`, and your original files stay untouched. A seed should take around 21 MB, since the patched `data.trpfd` index is 20.3 MB and the changed tables add a little more. A rebuilt XCI would take about 10 GB per seed, because it has to contain the base game merged with the update.

## 1. App and workflow

- [x] Open the app, pick Scarlet or Violet, configure, press Randomize. `New`
- [x] Auto-detect the base XCI, update NSP, Eden keys and Eden mod folder. `New` `data`
- [x] Check title IDs and build IDs, and refuse other versions with a clear message. `New` `data`
- [x] Seed as text or number. The same seed, settings and game give identical output. `UPR` `SV`
- [x] Batch mode with count, name prefix, starting index, auto-advancing index, output folder and per-seed logs. `UPR` `SV`
- [x] Seed library. One click copies a seed into a single Eden mod folder, so exactly one seed is ever active. `New` `test`
- [x] Launch the game in Eden from the app. `New` `test`
- [x] Warn when switching seeds on an existing save, and offer a save backup. `New` `test`
- [x] Save and load settings as a file. `UPR` `SV`
- [x] Share code holding settings and, optionally, the seed. `UPR`
- [x] Built-in presets for Balanced, Chaos, Hard and Light. `New`
- [x] Spoiler log for every seed, as searchable HTML and plain text. `UPR`
- [ ] Race mode, which writes no log. `UPR`
- [x] Validator that re-reads every output file and rejects any seed with an illegal species, move, ability or item. `New` `data`
- [ ] Read the update from Eden's NAND, so you can delete the update NSP to save 10 GB. `New` `test`
- [ ] Standalone patched XCI export at about 10 GB per seed. `New` `test`
- [ ] Zip export of a seed for sharing. `New`
- [ ] Check for app updates at startup. Needs network access. `SV`

## 2. GUI

- [x] Custom dark design whose accent color follows the selected game. It uses no Nintendo artwork. `New`
- [x] Sidebar with one page per section of this list. `New`
- [x] Region tabs for Paldea, Kitakami and Blueberry on the pages that need them. `SV`
- [x] Search across all options. `New`
- [x] A plain-language explanation on every option. `UPR`
- [x] Conflicting options grey out and say why. `UPR`
- [x] Live count of allowed species per section. Randomize stays blocked if a pool is too small. `New`
- [x] Summary of changed settings before randomizing. `New`
- [x] Batch progress with per-seed status and a cancel button. `New`
- [x] Reset button per section. `New`

## 3. Species pool

These filters apply globally, and each section can override them. The pool includes 69 species that are in no regional dex, and I still need to test how the Pokédex handles catching one.

- [x] Allowed generations. `UPR` `SV`
- [x] Allowed legendaries by generation. `SV`
- [x] Allowed evolution stages, including single-stage Pokémon. `SV`
- [x] Allow or ban Paradox Pokémon. `SV`
- [x] Allow alternate forms, with a "no irregular forms" switch. `UPR` `SV`
- [x] Limit to the Paldea, Kitakami or Blueberry dex. `New` `data`
- [x] Manual ban list and allow list. This also covers pkNX's Shedinja toggle. `New`
- [x] Per-section pool overrides. `SV`
- [ ] Allow Ogerpon and Terapagos in random pools. The SV reference warns not to Terastallize them if caught. `SV` `test`
- [ ] Legendary appearance chance as a percentage. `pkNX`

## 4. Base stats, types and abilities

- [x] Base stats unchanged, shuffled or random. `UPR` `SV` `data`
- [x] Keep each Pokémon's base stat total. `SV` `data`
- [x] Stats follow evolutions. `UPR` `data`
- [x] Randomize the stats added on evolution. `UPR` `data`
- [x] Standardize EXP curves for slow legendaries, slow strong legendaries or all Pokémon. `UPR` `data`
- [x] Types unchanged, random following evolutions, or fully random. `UPR` `SV` `data`
- [x] Force dual typing, or grant a second type at random. `UPR` `SV` `data`
- [x] Random abilities, optionally following evolutions. `UPR` `SV` `data`
- [x] Allow or ban Wonder Guard. `UPR` `SV` `data`
- [x] Ban exit abilities, Wimp Out and Emergency Exit. `SV` `data`
- [x] Ban trapping, negative and bad abilities as separate switches. `UPR` `data`
- [x] Ensure two abilities. `UPR` `data`
- [x] Combine duplicate abilities in the random pool. `UPR` `data`
- [x] Minimum catch rate. `UPR` `data`
- [ ] Random EV yields. `New` `data`
- [ ] Random gender ratios. `New` `data`
- [x] Faster egg hatching. `New` `data`

## 5. Evolutions

- [x] Evolutions unchanged, random, or random at every level. `UPR` `SV` `data`
- [x] Similar strength. `UPR` `data`
- [x] Same typing. `UPR` `data`
- [x] Limit to three stages. `UPR` `data`
- [x] Force a change. `UPR` `data`
- [x] Allow alternate forms. `UPR` `data`
- [x] Replace trade evolutions with level-ups. `UPR` `SV` `data`
- [x] Replace time-of-day and regional evolutions with level-ups. `UPR` `SV` `data`
- [x] Make evolutions easier. `UPR` `data`

## 6. Moves

- [x] Level-up moves unchanged, random preferring same type, or fully random. `UPR` `SV` `data`
- [x] Metronome-only mode. `UPR` `test`
- [x] Guaranteed level 1 moves. `UPR` `data`
- [x] Reorder damaging moves by strength. `UPR` `data`
- [x] Ban game-breaking moves. `UPR` `data`
- [x] Force a percentage of good damaging moves. `UPR` `data`
- [x] Evolution moves for all Pokémon. `UPR` `data`
- [x] Minimum STAB and damaging moves per learnset. `pkNX` `data`
- [x] Expand learnset size and spread moves across levels. `pkNX` `data`
- [x] Random egg moves and reminder moves. `New` `data`
- [x] Random move power, accuracy, PP, type and category. `UPR` `data`
- [ ] Random move priority. `New` `data`
- [x] Random TM moves, with good-damage percentage and game-breaking bans. `UPR` `SV` `data`
- [x] TM compatibility unchanged, random preferring same type, fully random or full. `UPR` `data`
- [x] TM compatibility follows evolutions. `UPR` `data`
- [x] TM and level-up move sanity. `UPR` `data`
- [x] TM crafting cost unchanged, reduced or free, for LP and materials. `New` `data`
- [ ] Random TM crafting materials. `New` `data`
- [ ] Remove TM unlock conditions. `New` `data`

## 7. Starters

- [x] Starters unchanged, custom, fully random, or random basics with two evolutions. `UPR` `SV` `test`
- [x] Per-starter species, form, gender, Poké Ball and shiny. `SV` `test`
- [x] Force all shiny, force one shiny, or a shiny percentage. `SV` `test`
- [x] Random Tera types. `SV` `data`
- [x] Random held items, with a bad-item ban. `UPR` `data`
- [x] Reveal the new starters at Clavell's visit, at Nemona's house (default), or only after choosing. Before the reveal, the cutscenes show the original three. `SV` `test`
- [x] Allow alternate forms. `UPR` `test`
- [ ] Custom starter nickname. `SV` `test`

## 8. Gifts and trades

There are 8 randomizable gifts out of 13 entries. The 3 starters are in section 7, and the 2 Koraidon and Miraidon entries stay unchanged. There are 33 trade entries, split into 3 story trades and 30 League Club guest trades.

- [x] Random gifts for Quagsire, Yanma, Meowth, Munchlax and Hisuian Growlithe. `SV` `test`
- [x] Random gift eggs for Turtwig, Chimchar and Piplup. `SV` `test`
- [x] Gift shiny percentage and random Tera types. `SV` `data`
- [x] Story trades unchanged, given Pokémon only, or both given and requested. `UPR` `data`
- [x] Random trade nicknames, original trainers, IVs and held items. `UPR` `test`
- [x] Random League Club guest trades. `New` `test`

## 9. Wild Pokémon

There are 956 rows, with 449 in Paldea, 252 in Kitakami, 254 in Blueberry and 1 in a small `lc` table. All four tables use the same fields, including biome weights, areas, levels, time of day and version flags.

- [x] Separate settings for Paldea, Kitakami and Blueberry, with "use Paldea settings for all". `SV` `data`
- [x] Row swap, which replaces species and keeps areas, biomes and levels. `New` `data`
- [x] Global 1-to-1 mapping, so each species always becomes the same replacement. `UPR` `data`
- [x] Area 1-to-1 mapping, by splitting rows per area. `UPR` `test`
- [x] Full shuffle, where any species can appear anywhere. `SV` `test`
- [x] Similar strength rule. `UPR` `data`
- [x] Catch-'em-all rule, so every allowed species appears at least once. `UPR` `data`
- [x] Type-themed areas. `UPR` `test`
- [x] Keep or remove time-of-day restrictions. `UPR` `data`
- [x] Exclude legendaries. `UPR` `SV` `data`
- [x] Allow alternate forms. `UPR` `data`
- [x] Random wild held items, with a bad-item ban. `UPR` `data`
- [x] Wild level modifier as a percentage. `UPR` `data`
- [x] Remove version exclusives, so both games have both sets. `New` `data`
- [x] Fliers and swimmers spawn only where their movement works. `SV` `test`
- [ ] Random group-spawn companions. `New` `test`

## 10. Static overworld encounters

There are 1,117 rows.

- [x] Random with its own species pool. `SV` `data`
- [x] Swap legendaries with legendaries and standards with standards. `UPR` `data`
- [x] Random at similar strength. `UPR` `data`
- [x] Treat 600+ total stat Pokémon as legendary. `UPR` `data`
- [x] Allow alternate forms. `UPR` `data`
- [x] Level modifier as a percentage. `UPR` `data`
- [x] Keep or randomize Tera types. `SV` `data`
- [x] Shiny percentage. `SV` `data`

## 11. Titans, legendaries and scripted battles

There are 117 scripted battle entries, and I found scene files on 4.0.0 for every group. Scene edits make this the riskiest section. Each group below gets its own on/off switch and species pool. The SV reference has one switch for all of them and left per-group switches disabled in its UI.

- [x] Titans, 16 entries covering 7 species. `SV` `test`
- [x] Treasures of Ruin, 4 entries. `SV` `test`
- [x] Area Zero multi-battles, 11 entries. `SV` `test`
- [ ] ~~Gimmighoul coin encounters, 13 entries.~~ Dropped after an in-game test. Replacing the chest and roaming Gimmighoul with another species crashed the game's scripts on about half of all save loads, and keeping the Gimmighoul model while changing only the battle would show one Pokémon and fight another. Gimmighoul's battles and objects now stay vanilla, and wild and static spawns never use Gimmighoul either. `SV`
- [x] Lechonk tutorial and Houndoom story fights, 2 entries. `SV` `test`
- [x] Sunflora gym test Pokémon, 5 entries. `SV` `test`
- [x] Loyal Three and Ogerpon story fights in Kitakami, 21 entries. `SV` `test`
- [x] Kitakami side quest battles, 2 entries. `SV` `test`
- [x] Indigo Disk story fights, including the Underdepths and Terapagos, 12 entries. `SV` `test`
- [x] Snacksworth legendaries, 25 entries. `SV` `test`
- [x] Meloetta and Pecharunt. `SV` `test`
- [x] Level modifier as a percentage. `New` `data`
- [x] Ban Wonder Guard in must-win fights to prevent softlocks, including Terapagos. `SV` `data`
- [x] Random moves, held items and Tera types for these Pokémon. `New` `data`
- [ ] Koraidon and Miraidon battles, including the final fight, 4 entries. `SV` `test`

## 12. Trainers

There are 756 rows, and I skip the test rows. Groups are 471 route trainers, Nemona, Arven and Penny, Clavell and school staff, 32 gym rows, the Elite Four and Geeta, 17 Team Star rows, Kieran and Carmine, the Blueberry Elite Four and their challenges, 33 League Club guests, and 52 raid helpers.

- [x] Trainers unchanged, random, or random with even distribution. `UPR` `SV` `data`
- [x] Even distribution limited to main-game Pokémon. `UPR` `data`
- [x] Type-themed teams for all trainers, or gyms and Elite Four only. `UPR` `pkNX` `data`
- [x] Separate settings per group and region, with "use rival settings for all" and "use Paldea settings for all". `SV` `data`
- [x] Rival carries your starter through the game. Nemona has separate rows per starter. `UPR` `test`
- [x] Similar strength. `UPR` `data`
- [x] Weight types by species count. `UPR` `data`
- [x] Exclude legendaries. `UPR` `data`
- [x] No early Wonder Guard. `UPR` `SV` `data`
- [x] Force fully evolved at a chosen level. `UPR` `pkNX` `data`
- [x] Trainer level modifier as a percentage. `UPR` `pkNX` `data`
- [x] Allow alternate forms. `UPR` `data`
- [x] Pokémon League has unique Pokémon. `UPR` `data`
- [x] Better movesets. `UPR` `data`
- [x] Extra Pokémon per group, or force six. `UPR` `SV` `pkNX` `test`
- [x] Double battles for all possible trainers, or random singles and doubles. `UPR` `SV` `pkNX` `test`
- [x] Held items per group, with consumable-only, sensible-only and highest-level-only rules. `UPR` `pkNX` `data`
- [x] Shiny trainer Pokémon. `UPR` `SV` `pkNX` `data`
- [x] Allow Tera per group, and random Tera types. `SV` `test`
- [x] Force perfect IVs per group. `SV` `pkNX` `data`
- [x] Smart AI per group. `SV` `pkNX` `data`
- [x] Random abilities. `pkNX` `data`
- [ ] Prize money multiplier. `New` `data`
- [x] Random trainer names, with a custom name list. `UPR` `test`
- [x] Random trainer class names, with a custom name list. `UPR` `test`
- [x] Random trainer classes. `pkNX` `test`

## 13. Tera Raids

There are 700 rows, with 454 in Paldea, 133 in Kitakami and 113 in Blueberry, across six star tiers.

- [x] Random raid bosses per region, with "use Paldea settings for all". `SV` `data`
- [x] Boss strength scales with star tier. The SV reference has an empty checkbox for this. `New` `data`
- [x] Per-tier on/off switches. `New` `data`
- [x] Force shiny or shiny percentage. `SV` `test`
- [x] Random Tera types. `New` `data`
- [x] Legal moves for the new species. `New` `data`
- [x] Random fixed and lottery raid rewards. `New` `data`
- [ ] Raid capture rate. `New` `data`
- [ ] Raid time limit. `New` `test`
- [ ] Random raid boss size. `New` `data`

## 14. Items

- [x] Hidden and sparkling ground items unchanged, shuffled, random or evenly distributed, across all 93 tables. `UPR` `SV` `data`
- [x] Pickup ability items. `UPR` `SV` `data`
- [x] Let's Go auto-battle items. `SV` `data`
- [x] Pokémon material drops, 580 rows, keeping TMs craftable. The SV reference removes materials and can make TMs impossible to craft. `SV` `data`
- [x] Ban bad items, per section. `UPR` `data`
- [x] Ban Lucky Egg and big-money items. `UPR` `data`
- [ ] Visible overworld item balls. Likely stored in scene files. `UPR` `research`

## 15. Shops

There are 26 item shops with 668 lineup rows. The SV reference has a disabled Shop tab, so no reference has proven this on Scarlet and Violet.

- [x] Shop stock unchanged, shuffled or random. `UPR` `test`
- [x] The Pokémon Center counter keeps its standard items (Potions, Poké Balls, status heals) and sells random extras after them: 0 to 16 from the start (8 by default) plus 0 to 4 more with each gym badge (2 by default), so 24 extras after the eighth badge. You can also randomize the counter like the other shops. This is the default because you asked for it after the first test. `UPR` `New` `test`
- [x] Random items only come from the items Scarlet and Violet actually support. The item table also keeps old items such as the Apricorns, which the game shows as "None". The app reads the real list from the game's own item enum. `New` `data`
- [x] Food and picnic items (sandwich ingredients, condiments, picnic gear) stay unchanged by default, so recipes always work. A switch randomizes them too, and even then food may appear in several shops. `New` `data`
- [x] No repeats across shop kinds: in random mode each random item is sold by only one kind of shop. On by default. Shops of the same kind (every Pokémon Center, every bakery) share one list in the game, so they always match each other. With about 370 goods slots and 290 to 345 sellable items, roughly 10% of slots still repeat, and the spoiler says how many. `New` `data`
- [x] Ban overpowered items, bad items and regular shop items. `UPR` `data`
- [x] Balance item prices. `UPR` `data`
- [x] Guarantee evolution items and X items. `UPR` `data`
- [x] Keep badge gating, or remove it. `New` `data`

## 16. Other game content

- [x] Team Star base Pokémon, 111 rows. `New` `test`
- [ ] Starmobile moves, types and stats, with the form unchanged. `New` `test`
- [ ] Random League Club coach rewards, 30 rows. `New` `test`
- [ ] Sandwich recipes and powers. `New` `test`
- [ ] Random Pokémon sizes on static, raid and trainer Pokémon. `New` `data`

## 17. Adding missing species

This is a separate project that would come after the randomizer works. Nobody has shown it working on Scarlet and Violet as far as I could find, and it would need your own dumps of the source games. Each item depends on the one before it.

- [x] Engine test. Turn on one missing species with a borrowed model and check that the game accepts it in the overworld, battle, boxes and Pokédex. `New` `test`
  - Result on 2026-11-07, with Victini using Pikachu's model and data as the grass starter: receiving it, its name, typing, moves, icon and summary all work. Battle start aborts. The abort is a Lua script panic in the game's script engine, the same one an empty party triggers. The game read every Victini data file we supplied before it aborted. The script error text could not be captured, because Eden's debugger slows the game too much to reach the battle. Finding what the battle script needs is open research.
- [ ] Pilot with Legends Arceus models, which use the closest format. Covers up to 55 missing species. `New` `research`
- [ ] Sword/Shield converter for models and animations. Covers at least 219 missing species. `New` `research`

## Not offered

| Option | Source | Why |
| --- | --- | --- |
| Allow moves without animation | `SV` | 222 of 920 moves are flagged unusable, and assigning them risks crashes |
| Species outside the 733, in the first version | any | 292 species have stats, moves and names in the game, but no models, animations or icons. Section 17 covers this |
| Replacing the Koraidon or Miraidon you ride | `SV` | It is story-scripted, and the reference itself marks it "Not Recommended" and disables it |
| Event raids and event outbreaks | `New` | The game gets these events through online news, which I don't expect to work on Eden. I have not tested this |
| Tree-shake encounters | `New` | The table holds one test row |
| Wild shiny odds, type chart, text speed, EXP formula | `UPR` | I found no data table for these. They would need code patches for two game builds |
| "Kaizo Randomizer" checkbox | `SV` | It sets a flag that nothing reads. The Hard preset replaces it |
| "Balance on BST" and "Same Moves as Type" | `SV` | The reference labels them "Useless". Sections 6, 9, 10 and 13 have working versions |

## UPR options that don't apply to this game

| Options | Why |
| --- | --- |
| Mega evolution follow and swap options | Scarlet and Violet have no Megas |
| Move tutors and tutor compatibility | No tutors. Reminder moves in section 6 are the closest match |
| HM compatibility, keep field move TMs | No HMs |
| Totem Pokémon, allies and auras | Gen 7 only. Titans are in section 11 |
| Update base stats or moves to a generation | The data is already Gen 9 |
| Fix music, balance shaking grass, SOS battles, fast Distortion World, balance static levels | Mechanics from older games |
| B/W EXP patch, X Accuracy nerf, crit fix, running shoes, PC potion, challenge mode, lowercase names, Rotom typing, low HP music, national dex, Pikachu evolution, catching tutorial | Older-game code patches, or the feature doesn't exist here |
| Load game update, keep or unload game after randomizing | The app always uses 4.0.0 and remembers your game |

## Build order after you confirm

1. Boot test. A hand-built mod changes one trainer and one starter on both games and runs in Eden on top of 4.0.0, with your code mods enabled. Everything else depends on this result.
2. Core reader and writer. This covers XCI, NSP, the update patch layer, RomFS, archive and table parsing, with a round-trip test on every table and a comparison against the reference's tables.
3. Randomizers section by section, each checked by the validator.
4. GUI and batch workflow.
5. In-game test pass for every `test` item.

I can boot the games and check early-game behavior. I can't play the main story and both DLCs for every option combination, so issues deep in the story may only show up when you play. The spoiler log and share code will make those reproducible.

## Technical approach

No action needed from you.

- The core is Rust and the desktop UI uses Tauri, which gives one Windows app with a web-based interface.
- Reading through the update's patch layer already works in my Python probe, on both games. The app will use a Rust version of the same logic.
- Table layouts come from the `.bfbs` schema files inside your game. The three `avalon` tables have no schema file, so I define their layouts by hand and check them against the reference's matching layouts.
- Oodle Kraken decompression uses the MIT-licensed `oozextract` crate. I built it and it decompressed every table I read, so we don't need the proprietary Oodle DLL.
- The repo holds no game data or keys. The app reads both from your machine at runtime.
