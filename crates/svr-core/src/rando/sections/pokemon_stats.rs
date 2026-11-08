//! Base stat arithmetic. Every function returns stats between 1 and 255, and the ones given a
//! total hit it exactly when that total is reachable.

use crate::rando::Rng;

pub type Stats = [u8; 6];

fn unit(rng: &mut Rng) -> f64 {
    (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64
}

fn weights(rng: &mut Rng) -> [f64; 6] {
    let mut w = [0.0; 6];
    for slot in &mut w {
        *slot = unit(rng);
    }
    w
}

fn sum(stats: Stats) -> u32 {
    stats.iter().map(|&s| u32::from(s)).sum()
}

/// Splits `total` points over six stats in proportion to `weights`, giving stat `i` at most
/// `caps[i]`. Rounds by largest remainder, so the result sums to `min(total, sum(caps))`.
pub fn apportion(total: u32, weights: [f64; 6], caps: [u32; 6]) -> [u32; 6] {
    let mut out = [0u32; 6];
    let mut left = total.min(caps.iter().sum());
    while left > 0 {
        let open: Vec<usize> = (0..6).filter(|&i| out[i] < caps[i]).collect();
        let mut w: Vec<f64> = open.iter().map(|&i| weights[i].max(0.0)).collect();
        if w.iter().sum::<f64>() <= 0.0 {
            w = vec![1.0; open.len()];
        }
        let total_w: f64 = w.iter().sum();
        let mut fractions = Vec::new();
        let mut given = 0;
        for (&i, &wi) in open.iter().zip(&w) {
            let ideal = f64::from(left) * wi / total_w;
            let share = (ideal.floor() as u32).min(caps[i] - out[i]);
            out[i] += share;
            given += share;
            if out[i] < caps[i] {
                fractions.push((ideal - ideal.floor(), i));
            }
        }
        left -= given;
        fractions.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        for (_, i) in fractions {
            if left == 0 {
                break;
            }
            out[i] += 1;
            left -= 1;
        }
    }
    out
}

fn to_stats(points: [u32; 6]) -> Stats {
    points.map(|p| p.min(255) as u8)
}

/// Keeps HP at 1 for Shedinja-like rows; the other five share the rest.
fn hp_mask(hp_is_one: bool, mut w: [f64; 6]) -> [f64; 6] {
    if hp_is_one {
        w[0] = 0.0;
    }
    w
}

pub fn random_permutation(rng: &mut Rng, hp_is_one: bool) -> [usize; 6] {
    let mut perm = [0, 1, 2, 3, 4, 5];
    let start = usize::from(hp_is_one);
    rng.shuffle(&mut perm[start..]);
    perm
}

pub fn permute(stats: Stats, perm: [usize; 6]) -> Stats {
    perm.map(|from| stats[from])
}

/// UPR's rule: HP at least 20, the rest at least 10, total kept.
pub fn random_with_total(rng: &mut Rng, total: u32, hp_is_one: bool) -> Stats {
    let floors: [u32; 6] = if hp_is_one { [1, 10, 10, 10, 10, 10] } else { [20, 10, 10, 10, 10, 10] };
    let caps = floors.map(|f| 255 - f);
    let caps = if hp_is_one { [0, caps[1], caps[2], caps[3], caps[4], caps[5]] } else { caps };
    let floor_sum: u32 = floors.iter().sum();
    let extra = apportion(total.saturating_sub(floor_sum), hp_mask(hp_is_one, weights(rng)), caps);
    to_stats([0, 1, 2, 3, 4, 5].map(|i| floors[i] + extra[i]))
}

/// Each stat uniform in 15..=255, so the total changes too.
pub fn random_any(rng: &mut Rng, hp_is_one: bool) -> Stats {
    let mut out = [0u8; 6];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = if i == 0 && hp_is_one { 1 } else { rng.range(15, 255) as u8 };
    }
    out
}

/// The parent's shape scaled to `total`, as UPR does for evolutions that follow their parent.
pub fn scaled(parent: Stats, total: u32, hp_is_one: bool) -> Stats {
    let w = hp_mask(hp_is_one, parent.map(f64::from));
    let caps = if hp_is_one { [0, 254, 254, 254, 254, 254] } else { [254; 6] };
    let extra = apportion(total.saturating_sub(6), w, caps);
    to_stats(extra.map(|e| e + 1))
}

/// The parent's stats plus a random split of the difference to `total`, which is how UPR
/// handles split evolutions and its "random added stats" option.
pub fn with_random_gains(rng: &mut Rng, parent: Stats, total: u32, hp_is_one: bool) -> Stats {
    let base = if hp_is_one { [1, parent[1], parent[2], parent[3], parent[4], parent[5]] } else { parent };
    let w = hp_mask(hp_is_one, weights(rng));
    let current = sum(base);
    let fixed_hp = |caps: [u32; 6]| if hp_is_one { [0, caps[1], caps[2], caps[3], caps[4], caps[5]] } else { caps };
    if total >= current {
        let caps = fixed_hp(base.map(|s| 255 - u32::from(s)));
        let gain = apportion(total - current, w, caps);
        to_stats([0, 1, 2, 3, 4, 5].map(|i| u32::from(base[i]) + gain[i]))
    } else {
        let caps = fixed_hp(base.map(|s| u32::from(s) - 1));
        let loss = apportion(current - total, w, caps);
        to_stats([0, 1, 2, 3, 4, 5].map(|i| u32::from(base[i]) - loss[i]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apportion_hits_the_total_and_respects_caps() {
        let out = apportion(500, [1.0, 2.0, 3.0, 0.0, 1.0, 1.0], [255, 255, 100, 255, 255, 255]);
        assert_eq!(out.iter().sum::<u32>(), 500);
        assert!(out[2] <= 100);
        let full = apportion(5000, [1.0; 6], [10; 6]);
        assert_eq!(full, [10; 6]);
    }

    #[test]
    fn random_stats_keep_their_total_and_floors() {
        let mut rng = Rng::new(3);
        for total in [180, 318, 525, 680, 720] {
            let s = random_with_total(&mut rng, total, false);
            assert_eq!(sum(s), total);
            assert!(s[0] >= 20 && s[1..].iter().all(|&v| v >= 10));
        }
        let shedinja = random_with_total(&mut rng, 236, true);
        assert_eq!(shedinja[0], 1);
        assert_eq!(sum(shedinja), 236);
    }

    #[test]
    fn evolutions_keep_the_parent_shape_or_add_to_it() {
        let parent = [45, 49, 49, 65, 65, 45];
        let child = scaled(parent, 625, false);
        assert_eq!(sum(child), 625);
        assert!(child[3] > child[0]);
        let mut rng = Rng::new(9);
        let grown = with_random_gains(&mut rng, parent, 525, false);
        assert_eq!(sum(grown), 525);
        assert!(grown.iter().zip(parent).all(|(&g, p)| g >= p));
        let shrunk = with_random_gains(&mut rng, parent, 250, false);
        assert_eq!(sum(shrunk), 250);
    }
}
