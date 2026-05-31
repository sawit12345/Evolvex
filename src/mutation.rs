//! Mutation. Two flavours:
//!   * `mutate_archive` — germline mutation applied to a child's archive at
//!     reproduction (the main source of heritable variation).
//!   * `somatic_mutation` — a small per-tick (loop step 3f) tweak to a living
//!     organism's archive.
//!
//! Every operation keeps the genome *valid*: a module that mutates into
//! nonsense becomes a `Noop`, never a crash, and sizes stay bounded.

use rand::Rng;

use crate::config::{MAX_ARCHIVE_MODULES, MUTATION_COST};
use crate::module::{random_kind, random_module, ModuleKind};
use crate::organism::Archive;
use crate::tag::{mutate_tag, random_tag};

/// Germline mutation: tag bit-flips, kind drift, cost tweaks, duplication,
/// deletion, decoder-bias drift, membrane-token (decoder-bias) drift, and rare
/// mutation-rate / nonsense->Noop events. Returns the energy that should be
/// charged for the mutation event (`MUTATION_COST` if anything fired).
pub fn mutate_archive(archive: &mut Archive, next_module_id: &mut u64, rng: &mut impl Rng) -> i64 {
    let rate = archive.mutation_rate.clamp(0.0, 0.25);
    let mut fired = false;

    for m in &mut archive.dormant_modules {
        let before = (m.tag, m.kind, m.cost, m.payload);
        mutate_tag(&mut m.tag, rate, rng); // bit flips
        if rng.gen_bool(rate * 0.2) {
            m.kind = random_kind(rng); // kind drift
        }
        if rng.gen_bool(rate * 0.2) {
            m.cost = (m.cost + rng.gen_range(-1..=1)).clamp(1, 8);
        }
        if rng.gen_bool(rate * 0.2) {
            m.payload = (m.payload + rng.gen_range(-1..=1)).clamp(0, 12); // price drift
        }
        if rng.gen_bool(rate * 0.05) {
            m.kind = ModuleKind::Noop; // nonsense collapses to a no-op, never invalid
        }
        if (m.tag, m.kind, m.cost, m.payload) != before {
            fired = true;
        }
    }

    // Decoder-bias tags double as membrane tokens, so this also mutates the
    // exposed-API surface.
    for t in &mut archive.decoder_bias_tags {
        let before = *t;
        mutate_tag(t, rate, rng);
        if *t != before {
            fired = true;
        }
    }
    // Occasionally add a fresh membrane/decoder token (new exposable interface).
    if rng.gen_bool(rate * 0.15) && archive.decoder_bias_tags.len() < 6 {
        archive.decoder_bias_tags.push(random_tag(rng));
        fired = true;
    }

    // Duplicate a module (gene duplication, then diverge).
    if rng.gen_bool(rate * 0.5)
        && !archive.dormant_modules.is_empty()
        && archive.dormant_modules.len() < MAX_ARCHIVE_MODULES
    {
        let i = rng.gen_range(0..archive.dormant_modules.len());
        let mut dup = archive.dormant_modules[i].clone();
        dup.id = *next_module_id;
        *next_module_id += 1;
        mutate_tag(&mut dup.tag, rate.max(0.01), rng);
        archive.dormant_modules.push(dup);
        fired = true;
    }
    // Delete a module (gene loss), keeping a minimal genome.
    if rng.gen_bool(rate * 0.25) && archive.dormant_modules.len() > 2 {
        let i = rng.gen_range(0..archive.dormant_modules.len());
        archive.dormant_modules.remove(i);
        fired = true;
    }
    // Rare brand-new module (innovation).
    if rng.gen_bool(rate * 0.1) && archive.dormant_modules.len() < MAX_ARCHIVE_MODULES {
        archive.dormant_modules.push(random_module(*next_module_id, rng));
        *next_module_id += 1;
        fired = true;
    }
    // Drift the mutation rate itself (evolvable evolvability).
    if rng.gen_bool(rate * 0.2) {
        archive.mutation_rate = (archive.mutation_rate + rng.gen_range(-0.01..=0.01)).clamp(0.001, 0.2);
        fired = true;
    }

    if fired {
        MUTATION_COST
    } else {
        0
    }
}

/// A gentle somatic mutation for living organisms (loop step 3f): a single bit
/// flip on a random dormant module's tag. Cheap, occasionally exposes cryptic
/// variation when the next decode runs.
pub fn somatic_mutation(archive: &mut Archive, next_module_id: &mut u64, rng: &mut impl Rng) {
    if archive.dormant_modules.is_empty() {
        return;
    }
    let i = rng.gen_range(0..archive.dormant_modules.len());
    mutate_tag(&mut archive.dormant_modules[i].tag, 0.02, rng);
    // Rarely, a somatic duplication seeds new raw material.
    if rng.gen_bool(0.05) && archive.dormant_modules.len() < MAX_ARCHIVE_MODULES {
        let mut dup = archive.dormant_modules[i].clone();
        dup.id = *next_module_id;
        *next_module_id += 1;
        archive.dormant_modules.push(dup);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::organism::Archive;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    fn sample_archive() -> Archive {
        let mut rng = ChaCha8Rng::seed_from_u64(7);
        let dormant = (0..6).map(|i| random_module(i, &mut rng)).collect();
        Archive {
            dormant_modules: dormant,
            decoder_bias_tags: vec![random_tag(&mut rng), random_tag(&mut rng)],
            mutation_rate: 0.2,
            storage_cost_modifier: 1.0,
        }
    }

    #[test]
    fn high_rate_mutation_changes_archive() {
        let mut rng = ChaCha8Rng::seed_from_u64(11);
        let mut a = sample_archive();
        let before = a.dormant_modules.clone();
        let mut next = 100u64;
        a.mutation_rate = 1.0; // clamped to 0.25 internally
        mutate_archive(&mut a, &mut next, &mut rng);
        assert_ne!(a.dormant_modules, before);
    }

    #[test]
    fn mutation_keeps_archive_bounded_and_nonempty() {
        let mut rng = ChaCha8Rng::seed_from_u64(13);
        let mut a = sample_archive();
        let mut next = 100u64;
        for _ in 0..500 {
            mutate_archive(&mut a, &mut next, &mut rng);
            assert!(!a.dormant_modules.is_empty());
            assert!(a.dormant_modules.len() <= MAX_ARCHIVE_MODULES);
        }
    }
}
