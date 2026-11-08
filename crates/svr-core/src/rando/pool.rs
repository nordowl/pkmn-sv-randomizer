use crate::data::facts::{self, Rarity};
use crate::data::{Dex, FormInfo, Pokemon};

use super::options::PoolFilter;

/// Evolution stage of a form for pool filtering: 0 single-stage, 1 first, 2 middle, 3 final.
pub fn stage_slot(dex: &Dex, form: &FormInfo) -> usize {
    let has_pre = dex.pre_evolution(form.id).is_some();
    let evolves = form.evolutions.iter().any(|e| dex.is_playable(e.into));
    match (has_pre, evolves) {
        (false, false) => 0,
        (false, true) => 1,
        (true, true) => 2,
        (true, false) => 3,
    }
}

/// Never placed by the randomizer, whatever the filter says.
pub fn always_excluded(form: &FormInfo, allow_tera_locked: bool) -> bool {
    let listed = |list: &[(u16, &str)]| list.iter().any(|&(n, _)| n == form.national);
    listed(facts::NEVER_RANDOM) || (!allow_tera_locked && listed(facts::TERA_LOCKED))
}

pub fn matches(dex: &Dex, form: &FormInfo, filter: &PoolFilter) -> bool {
    if !form.playable() {
        return false;
    }
    if filter.allowed.contains(&form.national) {
        return true;
    }
    if filter.banned.contains(&form.national) {
        return false;
    }
    let gen = usize::from(form.generation.clamp(1, 9)) - 1;
    if !filter.generations[gen] {
        return false;
    }
    if form.rarity != Rarity::Standard && !filter.legendaries[gen] {
        return false;
    }
    if form.paradox && !filter.paradox {
        return false;
    }
    if form.id.form != 0 && !filter.alt_forms {
        return false;
    }
    if !filter.irregular_forms && facts::is_irregular_form(form.national, form.id.form) {
        return false;
    }
    if !filter.stages[stage_slot(dex, form)] {
        return false;
    }
    if filter.regional_dex.iter().any(|&b| b) {
        let in_ticked = filter
            .regional_dex
            .iter()
            .zip(form.regional_dex)
            .any(|(&ticked, number)| ticked && number != 0);
        if !in_ticked {
            return false;
        }
    }
    true
}

/// Resolves an override against the global filter.
pub fn effective<'a>(global: &'a PoolFilter, section: &'a PoolFilter) -> &'a PoolFilter {
    if section.inherit {
        global
    } else {
        section
    }
}

pub fn build(dex: &Dex, filter: &PoolFilter, allow_tera_locked: bool) -> Vec<Pokemon> {
    dex.playable()
        .filter(|f| !always_excluded(f, allow_tera_locked) && matches(dex, f, filter))
        .map(|f| f.id)
        .collect()
}
