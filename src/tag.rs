use rand::Rng;
use serde::{Deserialize, Serialize};

pub type Tag = [u8; 8];

pub fn random_tag(rng: &mut impl Rng) -> Tag { rng.gen() }

pub fn tag_distance(a: &Tag, b: &Tag) -> u32 {
    a.iter().zip(b.iter()).map(|(x, y)| (x ^ y).count_ones()).sum()
}

pub fn mutate_tag(tag: &mut Tag, mutation_rate: f64, rng: &mut impl Rng) {
    let rate = mutation_rate.clamp(0.0, 1.0);
    for byte in tag.iter_mut() {
        for bit in 0..8 {
            if rng.gen_bool(rate) { *byte ^= 1 << bit; }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaggedCount { pub tag: Tag, pub count: usize }

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn hamming_distance_counts_bits() {
        assert_eq!(tag_distance(&[0; 8], &[0; 8]), 0);
        assert_eq!(tag_distance(&[0xff; 8], &[0; 8]), 64);
        assert_eq!(tag_distance(&[0b1010_0000,0,0,0,0,0,0,0], &[0b0011_0000,0,0,0,0,0,0,0]), 2);
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
}
