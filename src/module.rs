use rand::Rng;
use serde::{Deserialize, Serialize};
use crate::tag::{random_tag, tag_distance, Tag};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ModuleKind { Harvest, Attack, Defend, Copy, Decode, Trade, Repair, MoveEdge, Auth, Scavenge, Reproduce, Noop }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Module { pub id: u64, pub tag: Tag, pub kind: ModuleKind, pub cost: i64, pub payload: i64 }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModuleOwner { SelfOrg, Neighbor(usize), Corpse(usize) }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModuleRef { pub owner: ModuleOwner, pub module_index: usize, pub distance: u32 }

pub fn random_kind(rng: &mut impl Rng) -> ModuleKind {
    match rng.gen_range(0..100) {
        0..=23 => ModuleKind::Harvest, 24..=43 => ModuleKind::Noop, 44..=58 => ModuleKind::Decode,
        59..=67 => ModuleKind::Reproduce, 68..=75 => ModuleKind::Scavenge, 76..=82 => ModuleKind::Copy,
        83..=88 => ModuleKind::Defend, 89..=93 => ModuleKind::Attack, 94..=96 => ModuleKind::Trade,
        97..=98 => ModuleKind::MoveEdge, _ => ModuleKind::Auth,
    }
}

pub fn random_module(id: u64, rng: &mut impl Rng) -> Module {
    Module { id, tag: random_tag(rng), kind: random_kind(rng), cost: rng.gen_range(1..=4), payload: rng.gen_range(-3..=3) }
}

pub fn fuzzy_select_module(desired: &Tag, candidates: &[(ModuleOwner, usize, Tag)], max_distance: u32, rng: &mut impl Rng) -> Option<ModuleRef> {
    let mut weighted = Vec::new();
    let mut total = 0u32;
    for (owner, idx, tag) in candidates {
        let d = tag_distance(desired, tag);
        if d <= max_distance {
            let w = max_distance - d + 1;
            total += w;
            weighted.push((*owner, *idx, d, total));
        }
    }
    if total == 0 { return None; }
    let pick = rng.gen_range(0..total);
    weighted.into_iter().find(|(_,_,_,cum)| pick < *cum)
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
        let candidates = vec![(ModuleOwner::SelfOrg, 0, [0xff; 8]), (ModuleOwner::SelfOrg, 1, [1,0,0,0,0,0,0,0])];
        let hit = fuzzy_select_module(&desired, &candidates, 1, &mut rng).unwrap();
        assert_eq!(hit.module_index, 1);
        assert!(fuzzy_select_module(&desired, &candidates, 0, &mut rng).is_none());
    }
}
