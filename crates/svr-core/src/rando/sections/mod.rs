//! Each section file owns its options and the step that applies them. `ALL` is the run order:
//! species data first, then everything that places species, so strength checks see final stats.

mod encounter_common;
mod pool;
mod pokemon;
mod evolutions;
mod moves;
mod starters;
mod gifts;
mod wild;
mod statics;
mod bosses;
mod trainers;
mod raids;
mod items;
mod shops;
mod extras;
mod experimental;

use super::Section;

pub const ALL: &[Section] = &[pool::SECTION, pokemon::SECTION, evolutions::SECTION, moves::SECTION, starters::SECTION, gifts::SECTION, wild::SECTION, statics::SECTION, bosses::SECTION, trainers::SECTION, raids::SECTION, items::SECTION, shops::SECTION, extras::SECTION, experimental::SECTION];
