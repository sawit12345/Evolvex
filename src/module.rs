//! Modules: inert, *interpreted* code-like objects. There is no native code
//! execution anywhere in Evolvex; a module is a symbolic structure whose
//! `kind` the simulator interprets. `payload` is a functional semantic
//! parameter (e.g. the price a callee charges, or the magnitude of an effect).

use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::tag::{random_tag, tag_distance, Tag};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ModuleKind {
    Harvest,
    Attack,
    Defend,
    Copy,
    Decode,
    Trade,
    Repair,
    MoveEdge,
    Auth,
    Scavenge,
    Reproduce,
    Noop,
}

impl ModuleKind {
    pub const ALL: [ModuleKind; 12] = [
        ModuleKind::Harvest,
        ModuleKind::Attack,
        ModuleKind::Defend,
        ModuleKind::Copy,
        ModuleKind::Decode,
        ModuleKind::Trade,
        ModuleKind::Repair,
        ModuleKind::MoveEdge,
        ModuleKind::Auth,
        ModuleKind::Scavenge,
        ModuleKind::Reproduce,
        ModuleKind::Noop,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ModuleKind::Harvest => "Harvest",
            ModuleKind::Attack => "Attack",
            ModuleKind::Defend => "Defend",
            ModuleKind::Copy => "Copy",
            ModuleKind::Decode => "Decode",
            ModuleKind::Trade => "Trade",
            ModuleKind::Repair => "Repair",
            ModuleKind::MoveEdge => "MoveEdge",
            ModuleKind::Auth => "Auth",
            ModuleKind::Scavenge => "Scavenge",
            ModuleKind::Reproduce => "Reproduce",
            ModuleKind::Noop => "Noop",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Module {
    pub id: u64,
    pub tag: Tag,
    pub kind: ModuleKind,
    /// Energy to activate the module's behaviour.
    pub cost: i64,
    /// Semantic parameter. For an *exposed* module this is the price the callee
    /// charges callers; for effect modules it scales the effect magnitude.
    pub payload: i64,
}

/// Where a candidate module lives relative to the acting organism.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModuleOwner {
    SelfOrg,
    Neighbor(usize),
    Corpse(usize),
}

/// A resolved fuzzy match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModuleRef {
    pub owner: ModuleOwner,
    pub module_index: usize,
    pub distance: u32,
}

/// Draw a module kind from the spec's rough rarity distribution.
/// Note: every kind (including Repair) has non-zero probability, so no kind is
/// unreachable.
pub fn random_kind(rng: &mut impl Rng) -> ModuleKind {
    match rng.gen_range(0..100) {
        0..=19 => ModuleKind::Harvest,   // common
        20..=34 => ModuleKind::Noop,     // common
        35..=46 => ModuleKind::Decode,   // common
        47..=54 => ModuleKind::Reproduce,
        55..=62 => ModuleKind::Scavenge,
        63..=70 => ModuleKind::Copy,
        71..=78 => ModuleKind::Defend,
        79..=84 => ModuleKind::Repair,
        85..=90 => ModuleKind::Attack,
        91..=94 => ModuleKind::Trade,
        95..=97 => ModuleKind::MoveEdge,
        _ => ModuleKind::Auth, // rare
    }
}

pub fn random_module(id: u64, rng: &mut impl Rng) -> Module {
    Module {
        id,
        tag: random_tag(rng),
        kind: random_kind(rng),
        cost: rng.gen_range(1..=3),
        payload: rng.gen_range(0..=4),
    }
}

/// Find the nearest module to `desired` among `candidates`, considering only
/// matches within `max_distance`. When several qualify, pick one weighted by
/// closeness (nearer = more likely). Returns `None` if nothing is near enough.
pub fn fuzzy_select_module(
    desired: &Tag,
    candidates: &[(ModuleOwner, usize, Tag)],
    max_distance: u32,
    rng: &mut impl Rng,
) -> Option<ModuleRef> {
    let mut weighted: Vec<(ModuleOwner, usize, u32, u32)> = Vec::new();
    let mut total = 0u32;
    for (owner, idx, tag) in candidates {
        let d = tag_distance(desired, tag);
        if d <= max_distance {
            let w = max_distance - d + 1; // closer => heavier weight
            total += w;
            weighted.push((*owner, *idx, d, total));
        }
    }
    if total == 0 {
        return None;
    }
    let pick = rng.gen_range(0..total);
    weighted
        .into_iter()
        .find(|(_, _, _, cum)| pick < *cum)
        .map(|(owner, module_index, distance, _)| ModuleRef { owner, module_index, distance })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn fuzzy_selection_respects_max_distance() {
        let mut rng = ChaCha8Rng::seed_from_u64(2);
        let desired = [0u8; 8];
        let candidates = vec![
            (ModuleOwner::SelfOrg, 0, [0xff; 8]),
            (ModuleOwner::SelfOrg, 1, [1, 0, 0, 0, 0, 0, 0, 0]),
        ];
        let hit = fuzzy_select_module(&desired, &candidates, 1, &mut rng).unwrap();
        assert_eq!(hit.module_index, 1);
        assert!(fuzzy_select_module(&desired, &candidates, 0, &mut rng).is_none());
    }

    #[test]
    fn every_kind_is_reachable_from_random_kind() {
        let mut rng = ChaCha8Rng::seed_from_u64(99);
        let mut seen = std::collections::HashSet::new();
        for _ in 0..20_000 {
            seen.insert(random_kind(&mut rng));
        }
        for k in ModuleKind::ALL {
            assert!(seen.contains(&k), "kind {:?} was never generated", k);
        }
    }
}
