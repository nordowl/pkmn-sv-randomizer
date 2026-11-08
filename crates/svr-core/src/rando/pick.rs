//! Species choice shared by every section that places Pokémon.

use crate::data::{Dex, FormInfo, Pokemon};

use super::Rng;

pub struct Picker<'a> {
    dex: &'a Dex,
    pool: Vec<Pokemon>,
}

impl<'a> Picker<'a> {
    pub fn new(dex: &'a Dex, pool: Vec<Pokemon>) -> Picker<'a> {
        Picker { dex, pool }
    }

    pub fn pool(&self) -> &[Pokemon] {
        &self.pool
    }

    fn info(&self, p: Pokemon) -> &FormInfo {
        self.dex.get(p).expect("pool only holds dex forms")
    }

    pub fn any(&self, rng: &mut Rng) -> Pokemon {
        self.pool[rng.index(self.pool.len())]
    }

    /// A random pool member satisfying `keep`, or None if none does.
    pub fn matching(&self, rng: &mut Rng, keep: impl Fn(&FormInfo) -> bool) -> Option<Pokemon> {
        let candidates: Vec<Pokemon> = self.pool.iter().copied().filter(|&p| keep(self.info(p))).collect();
        rng.pick(&candidates).copied()
    }

    /// UPR's similar-strength rule: start within 10% of `target_bst` and widen by 5% steps until
    /// something in the pool qualifies.
    pub fn similar_strength(&self, rng: &mut Rng, target_bst: u32, keep: impl Fn(&FormInfo) -> bool) -> Option<Pokemon> {
        let mut band = target_bst / 10;
        for _ in 0..40 {
            let low = target_bst.saturating_sub(band);
            let high = target_bst + band;
            if let Some(p) = self.matching(rng, |f| (low..=high).contains(&f.bst()) && keep(f)) {
                return Some(p);
            }
            band += (target_bst / 20).max(5);
        }
        self.matching(rng, keep)
    }

    pub fn with_type(&self, rng: &mut Rng, type_id: u8) -> Option<Pokemon> {
        self.matching(rng, |f| f.types.contains(&type_id))
    }
}

/// Evolves `p` along random playable branches while the evolution level is at most `level`.
/// With `force_full_at`, anything at or above that level is evolved fully regardless of method.
pub fn evolve_for_level(dex: &Dex, rng: &mut Rng, p: Pokemon, level: u16, force_full_at: Option<u16>) -> Pokemon {
    let mut current = p;
    for _ in 0..4 {
        let Some(info) = dex.get(current) else { break };
        let force = force_full_at.is_some_and(|at| level >= at);
        let options: Vec<Pokemon> = info
            .evolutions
            .iter()
            .filter(|e| dex.is_playable(e.into))
            .filter(|e| force || (e.level > 0 && e.level <= level))
            .map(|e| e.into)
            .collect();
        match rng.pick(&options) {
            Some(&next) => current = next,
            None => break,
        }
    }
    current
}
