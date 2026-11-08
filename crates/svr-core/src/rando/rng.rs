//! Deterministic randomness. Every section draws from its own stream derived from the seed and the
//! section name, so toggling one section never reshuffles another.

const FNV_OFFSET: u64 = 0xCBF2_9CE4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;

fn fnv1a(bytes: &[u8], start: u64) -> u64 {
    bytes.iter().fold(start, |h, &b| (h ^ u64::from(b)).wrapping_mul(FNV_PRIME))
}

fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// A seed as the user typed it. A plain number is used as-is; any other text is hashed.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Seed(pub String);

impl Seed {
    pub fn value(&self) -> u64 {
        let text = self.0.trim();
        text.parse::<u64>().unwrap_or_else(|_| fnv1a(text.as_bytes(), FNV_OFFSET))
    }

    pub fn random() -> Seed {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u64);
        let mut state = nanos ^ u64::from(std::process::id()).rotate_left(32);
        Seed((splitmix(&mut state) % 10_000_000_000).to_string())
    }
}

/// xoshiro256** seeded through splitmix64.
#[derive(Clone, Debug)]
pub struct Rng {
    s: [u64; 4],
}

impl Rng {
    pub fn new(seed: u64) -> Rng {
        let mut state = seed;
        Rng {
            s: [splitmix(&mut state), splitmix(&mut state), splitmix(&mut state), splitmix(&mut state)],
        }
    }

    pub fn stream(seed: &Seed, name: &str) -> Rng {
        Rng::new(fnv1a(name.as_bytes(), seed.value() ^ FNV_OFFSET))
    }

    pub fn next_u64(&mut self) -> u64 {
        let result = self.s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        result
    }

    /// Uniform in `0..n`, without modulo bias. `n` must be positive.
    pub fn below(&mut self, n: u64) -> u64 {
        assert!(n > 0, "Rng::below(0)");
        let zone = u64::MAX - (u64::MAX % n);
        loop {
            let v = self.next_u64();
            if v < zone {
                return v % n;
            }
        }
    }

    pub fn range(&mut self, low: i64, high_inclusive: i64) -> i64 {
        assert!(low <= high_inclusive, "Rng::range with an empty range");
        let span = (high_inclusive - low) as u64 + 1;
        low + self.below(span) as i64
    }

    pub fn index(&mut self, len: usize) -> usize {
        self.below(len as u64) as usize
    }

    /// True with probability `percent` out of 100.
    pub fn percent(&mut self, percent: u32) -> bool {
        self.below(100) < u64::from(percent.min(100))
    }

    pub fn chance(&mut self, numerator: u64, denominator: u64) -> bool {
        self.below(denominator) < numerator
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        (!items.is_empty()).then(|| &items[self.index(items.len())])
    }

    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.index(i + 1);
            items.swap(i, j);
        }
    }

    /// Picks an index with probability proportional to its weight. Returns None if all are zero.
    pub fn weighted(&mut self, weights: &[u64]) -> Option<usize> {
        let total: u64 = weights.iter().sum();
        if total == 0 {
            return None;
        }
        let mut roll = self.below(total);
        for (i, &w) in weights.iter().enumerate() {
            if roll < w {
                return Some(i);
            }
            roll -= w;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_and_stream_repeat() {
        let seed = Seed("hello".into());
        let a: Vec<u64> = (0..5).scan(Rng::stream(&seed, "wild"), |r, _| Some(r.next_u64())).collect();
        let b: Vec<u64> = (0..5).scan(Rng::stream(&seed, "wild"), |r, _| Some(r.next_u64())).collect();
        assert_eq!(a, b);
        let c: Vec<u64> = (0..5).scan(Rng::stream(&seed, "trainers"), |r, _| Some(r.next_u64())).collect();
        assert_ne!(a, c);
    }

    #[test]
    fn numeric_seeds_are_used_directly() {
        assert_eq!(Seed("12345".into()).value(), 12345);
        assert_ne!(Seed("abc".into()).value(), Seed("abd".into()).value());
    }

    #[test]
    fn below_stays_in_range_and_covers_it() {
        let mut rng = Rng::new(7);
        let mut seen = [false; 6];
        for _ in 0..1000 {
            let v = rng.below(6) as usize;
            seen[v] = true;
        }
        assert!(seen.iter().all(|&s| s));
    }

    #[test]
    fn weighted_never_picks_zero_weight() {
        let mut rng = Rng::new(1);
        for _ in 0..500 {
            assert_eq!(rng.weighted(&[0, 5, 0]), Some(1));
        }
        assert_eq!(rng.weighted(&[0, 0]), None);
    }
}
