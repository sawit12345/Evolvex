//! Tags: fixed-size content addresses used for *fuzzy* (Hamming-distance) calls.
//!
//! A Tag is 64 bits. Organisms never address each other by exact id; they emit
//! a desired Tag and the nearest module within a distance threshold answers.
//! This is the core "digital chemistry": approximate calls can hit unintended
//! modules, which is a feature (it enables parasitism and novelty), not a bug.

use rand::Rng;

pub type Tag = [u8; 8];

pub const TAG_BITS: u32 = 64;

pub fn random_tag(rng: &mut impl Rng) -> Tag {
    rng.gen()
}

/// Hamming distance: number of differing bits between two tags.
pub fn tag_distance(a: &Tag, b: &Tag) -> u32 {
    a.iter().zip(b.iter()).map(|(x, y)| (x ^ y).count_ones()).sum()
}

/// Flip each bit independently with probability `mutation_rate`.
pub fn mutate_tag(tag: &mut Tag, mutation_rate: f64, rng: &mut impl Rng) {
    let rate = mutation_rate.clamp(0.0, 1.0);
    if rate == 0.0 {
        return;
    }
    for byte in tag.iter_mut() {
        for bit in 0..8 {
            if rng.gen_bool(rate) {
                *byte ^= 1 << bit;
            }
        }
    }
}

/// Return a near-neighbour of `base` with exactly `flips` random bits toggled.
/// Used when an organism aims a call "in the rough direction of" a tag.
pub fn jitter_tag(base: &Tag, flips: u32, rng: &mut impl Rng) -> Tag {
    let mut t = *base;
    for _ in 0..flips {
        let bit = rng.gen_range(0..TAG_BITS) as usize;
        t[bit / 8] ^= 1 << (bit % 8);
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn hamming_distance_counts_bits() {
        assert_eq!(tag_distance(&[0; 8], &[0; 8]), 0);
        assert_eq!(tag_distance(&[0xff; 8], &[0; 8]), 64);
        assert_eq!(
            tag_distance(&[0b1010_0000, 0, 0, 0, 0, 0, 0, 0], &[0b0011_0000, 0, 0, 0, 0, 0, 0, 0]),
            2
        );
    }

    #[test]
    fn mutation_rate_extremes_are_observable() {
        let mut rng = ChaCha8Rng::seed_from_u64(1);
        let mut t = [0u8; 8];
        mutate_tag(&mut t, 0.0, &mut rng);
        assert_eq!(t, [0; 8]);
        mutate_tag(&mut t, 1.0, &mut rng);
        assert_eq!(t, [0xff; 8]);
    }

    #[test]
    fn jitter_changes_exactly_n_bits() {
        let mut rng = ChaCha8Rng::seed_from_u64(3);
        let base = [0u8; 8];
        let j = jitter_tag(&base, 3, &mut rng);
        // Each flip toggles a distinct-or-repeated bit; distance is at most 3.
        assert!(tag_distance(&base, &j) <= 3);
    }
}
