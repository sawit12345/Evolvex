use rand::{seq::SliceRandom, Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use crate::{
    config::*, corpse::Corpse, metrics::Counters, module::{fuzzy_select_module, random_module, Module, ModuleKind, ModuleOwner},
    mutation::mutate_archive, organism::{Archive, Organism, Phenotype}, tag::{random_tag, tag_distance},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeOccupant { Empty, Organism(usize), Corpse(usize) }

pub struct World {
    pub tick: u64, pub rng: ChaCha8Rng, pub nodes: Vec<NodeOccupant>, pub edges: Vec<Vec<usize>>, pub organisms: Vec<Organism>, pub corpses: Vec<Corpse>,
    pub max_degree: usize, pub counters: Counters, pub next_org_id: u64, pub next_corpse_id: u64, pub next_module_id: u64,
}

impl World {
    pub fn new(seed: u64, nodes: usize, max_degree: usize) -> Self {
        Self { tick: 0, rng: ChaCha8Rng::seed_from_u64(seed), nodes: vec![NodeOccupant::Empty; nodes], edges: vec![Vec::new(); nodes], organisms: Vec::new(), corpses: Vec::new(), max_degree, counters: Counters::default(), next_org_id: 1, next_corpse_id: 1, next_module_id: 1 }
    }

    pub fn randomize_edges(&mut self) {
        let n = self.nodes.len();
        if n < 2 { return; }
        for i in 0..n { for _ in 0..self.max_degree { let j = self.rng.gen_range(0..n); if i != j { self.add_edge(i, j); } } }
    }

    pub fn add_edge(&mut self, a: usize, b: usize) -> bool {
        if a == b || self.edges[a].contains(&b) || self.edges[a].len() >= self.max_degree || self.edges[b].len() >= self.max_degree { return false; }
        self.edges[a].push(b); self.edges[b].push(a); true
    }

    pub fn remove_edge(&mut self, a: usize, b: usize) {
        self.edges[a].retain(|x| *x != b); self.edges[b].retain(|x| *x != a);
    }

    pub fn seed_random_organisms(&mut self, count: usize) {
        let mut sites: Vec<_> = (0..self.nodes.len()).collect(); sites.shuffle(&mut self.rng);
        for node in sites.into_iter().take(count.min(self.nodes.len())) { self.spawn_random_at(node, None, None); }
    }

    pub fn spawn_random_at(&mut self, node: usize, lineage: Option<u64>, parent: Option<u64>) -> usize {
        let id = self.next_org_id; self.next_org_id += 1;
        let lineage_id = lineage.unwrap_or(id);
        let mut dormant = Vec::new();
        for _ in 0..self.rng.gen_range(8..=14) { dormant.push(random_module(self.alloc_module_id(), &mut self.rng)); }
        if !dormant.iter().any(|m| m.kind == ModuleKind::Harvest) { dormant.push(Module { id: self.alloc_module_id(), tag: random_tag(&mut self.rng), kind: ModuleKind::Harvest, cost: 1, payload: 0 }); }
        let archive = Archive { dormant_modules: dormant, decoder_bias_tags: (0..3).map(|_| random_tag(&mut self.rng)).collect(), mutation_rate: self.rng.gen_range(0.005..=0.04), storage_cost_modifier: 0.25 };
        let mut org = Organism { id, energy: STARTING_ENERGY, age: 0, archive, phenotype: empty_pheno(), alive: true, lineage_id, parent_id: parent, symbiont_links: Vec::new(), local_memory: Vec::new(), node };
        decode_organism(&mut org);
        let idx = self.organisms.len(); self.organisms.push(org); self.nodes[node] = NodeOccupant::Organism(idx); idx
    }

    fn alloc_module_id(&mut self) -> u64 { let id = self.next_module_id; self.next_module_id += 1; id }

    pub fn step(&mut self) {
        self.tick += 1;
        let mut order: Vec<usize> = self.organisms.iter().enumerate().filter(|(_,o)| o.alive).map(|(i,_)| i).collect();
        order.shuffle(&mut self.rng);
        for idx in order { if idx < self.organisms.len() && self.organisms[idx].alive { self.organism_tick(idx); } }
        self.decay_corpses();
    }

    fn organism_tick(&mut self, idx: usize) {
        self.pay_maintenance(idx);
        if self.organisms[idx].energy <= 0 { self.kill(idx); return; }
        self.organisms[idx].age += 1; decode_organism(&mut self.organisms[idx]);
        let desired = self.choose_desired_tag(idx);
        let candidates = self.call_candidates(idx);
        if let Some(hit) = fuzzy_select_module(&desired, &candidates, MAX_FUZZY_DISTANCE, &mut self.rng) { self.execute_module(idx, hit.owner, hit.module_index); } else { self.harvest(idx); }
        if self.organisms[idx].alive && self.organisms[idx].energy <= 0 { self.kill(idx); }
    }

    fn pay_maintenance(&mut self, idx: usize) {
        let ph = self.organisms[idx].phenotype.active_modules.len() as i64 * ACTIVE_MODULE_COST;
        let ar = ((self.organisms[idx].archive.dormant_modules.len() as f64 * self.organisms[idx].archive.storage_cost_modifier).ceil() as i64).max(0) * ARCHIVE_MODULE_STORAGE_COST;
        self.organisms[idx].energy -= BASE_METABOLISM_COST + ph + ar;
    }

    fn choose_desired_tag(&mut self, idx: usize) -> [u8;8] {
        let node = self.organisms[idx].node;
        let mut neighbor_tags = Vec::new();
        for &n in &self.edges[node] {
            if let NodeOccupant::Organism(oi) = self.nodes[n] {
                if self.organisms[oi].alive {
                    neighbor_tags.extend(self.organisms[oi].phenotype.active_modules.iter().map(|m| m.tag));
                }
            }
        }
        if !neighbor_tags.is_empty() && self.rng.gen_bool(0.5) {
            *neighbor_tags.choose(&mut self.rng).unwrap()
        } else if let Some(t) = self.organisms[idx].phenotype.call_preferences.choose(&mut self.rng) { *t } else { random_tag(&mut self.rng) }
    }

    pub fn local_candidates(&self, idx: usize) -> Vec<(ModuleOwner, usize, [u8;8])> {
        let mut out = Vec::new();
        for (mi, m) in self.organisms[idx].phenotype.active_modules.iter().enumerate() { out.push((ModuleOwner::SelfOrg, mi, m.tag)); }
        for &n in &self.edges[self.organisms[idx].node] {
            match self.nodes[n] {
                NodeOccupant::Organism(oi) if self.organisms[oi].alive => for (mi, m) in self.organisms[oi].phenotype.active_modules.iter().enumerate() { out.push((ModuleOwner::Neighbor(oi), mi, m.tag)); },
                NodeOccupant::Corpse(ci) => for (mi, m) in self.corpses[ci].modules.iter().enumerate() { out.push((ModuleOwner::Corpse(ci), mi, m.tag)); },
                _ => {}
            }
        }
        out
    }

    pub fn call_candidates(&self, idx: usize) -> Vec<(ModuleOwner, usize, [u8;8])> {
        self.local_candidates(idx).into_iter().filter(|(owner, _, _)| !matches!(owner, ModuleOwner::Corpse(_))).collect()
    }

    fn execute_module(&mut self, caller: usize, owner: ModuleOwner, module_index: usize) {
        let module = match owner { ModuleOwner::SelfOrg => self.organisms[caller].phenotype.active_modules.get(module_index).cloned(), ModuleOwner::Neighbor(o) => self.organisms[o].phenotype.active_modules.get(module_index).cloned(), ModuleOwner::Corpse(_) => None };
        let Some(module) = module else { return; };
        self.organisms[caller].energy -= module.cost.max(0);
        if let ModuleOwner::Neighbor(provider) = owner { self.organisms[caller].energy -= CALL_COST; self.organisms[provider].energy += SERVICE_REWARD; self.counters.calls += 1; }
        match module.kind {
            ModuleKind::Harvest => self.harvest(caller), ModuleKind::Attack => self.attack(caller), ModuleKind::Defend => self.organisms[caller].phenotype.defense_level += 1,
            ModuleKind::Copy => self.copy_local_module(caller), ModuleKind::Decode => decode_organism(&mut self.organisms[caller]), ModuleKind::Trade => self.trade(caller),
            ModuleKind::Repair => self.organisms[caller].phenotype.defense_level += 1, ModuleKind::MoveEdge => self.rewire(caller), ModuleKind::Auth => self.organisms[caller].phenotype.defense_level += 2,
            ModuleKind::Scavenge => self.scavenge(caller), ModuleKind::Reproduce => self.try_reproduce(caller), ModuleKind::Noop => {}
        }
    }

    fn harvest(&mut self, idx: usize) { self.organisms[idx].energy += HARVEST_GAIN; }

    fn attack(&mut self, idx: usize) {
        self.organisms[idx].energy -= ATTACK_COST;
        let victims: Vec<_> = self.edges[self.organisms[idx].node].iter().filter_map(|&n| match self.nodes[n] { NodeOccupant::Organism(oi) if self.organisms[oi].alive => Some(oi), _ => None }).collect();
        if let Some(&v) = victims.choose(&mut self.rng) { let damage = (ATTACK_DAMAGE - self.organisms[v].defense() * DEFENSE_REDUCTION / 2).max(1); self.organisms[v].energy -= damage; self.counters.attacks += 1; if self.organisms[v].energy <= 0 { self.kill(v); } }
    }

    fn trade(&mut self, idx: usize) {
        let partners: Vec<_> = self.edges[self.organisms[idx].node].iter().filter_map(|&n| match self.nodes[n] { NodeOccupant::Organism(oi) if self.organisms[oi].alive => Some(oi), _ => None }).collect();
        if let Some(&p) = partners.choose(&mut self.rng) { let partner_id = self.organisms[p].id; self.organisms[idx].energy += 2; self.organisms[p].energy -= 2; if !self.organisms[idx].symbiont_links.contains(&partner_id) { self.organisms[idx].symbiont_links.push(partner_id); } }
    }

    fn scavenge(&mut self, idx: usize) {
        let corpses: Vec<_> = self.edges[self.organisms[idx].node].iter().filter_map(|&n| match self.nodes[n] { NodeOccupant::Corpse(ci) if self.corpses[ci].energy_value > 0 => Some(ci), _ => None }).collect();
        if let Some(&ci) = corpses.choose(&mut self.rng) { let gain = SCAVENGE_GAIN.min(self.corpses[ci].energy_value); self.corpses[ci].energy_value -= gain; self.organisms[idx].energy += gain; self.counters.scavenges += 1; if self.rng.gen_bool(0.25) { self.copy_from_corpse(idx, ci); } }
    }

    fn copy_local_module(&mut self, idx: usize) {
        self.organisms[idx].energy -= COPY_MODULE_COST;
        let mut corpse_hits = Vec::new();
        let mut neigh = Vec::new();
        for &n in &self.edges[self.organisms[idx].node] {
            match self.nodes[n] {
                NodeOccupant::Organism(oi) if self.organisms[oi].alive && !self.organisms[oi].archive.dormant_modules.is_empty() => neigh.push(oi),
                NodeOccupant::Corpse(ci) if !self.corpses[ci].modules.is_empty() => corpse_hits.push(ci),
                _ => {}
            }
        }
        if let Some(&ci) = corpse_hits.choose(&mut self.rng) { self.copy_from_corpse(idx, ci); return; }
        if let Some(&oi) = neigh.choose(&mut self.rng) { let m = self.organisms[oi].archive.dormant_modules.choose(&mut self.rng).unwrap().clone(); self.add_copied_module(idx, m); }
    }

    fn copy_from_corpse(&mut self, idx: usize, ci: usize) { if let Some(m) = self.corpses[ci].modules.choose(&mut self.rng).cloned() { self.organisms[idx].energy -= COPY_MODULE_COST; self.add_copied_module(idx, m); } }
    fn add_copied_module(&mut self, idx: usize, mut m: Module) { m.id = self.alloc_module_id(); self.organisms[idx].archive.dormant_modules.push(m); self.counters.hgt += 1; }

    fn rewire(&mut self, idx: usize) {
        self.organisms[idx].energy -= EDGE_REWIRE_COST; let node = self.organisms[idx].node;
        if let Some(&old) = self.edges[node].choose(&mut self.rng) { self.remove_edge(node, old); }
        for _ in 0..16 { let new = self.rng.gen_range(0..self.nodes.len()); if self.add_edge(node, new) { break; } }
    }

    fn try_reproduce(&mut self, idx: usize) {
        if self.organisms[idx].energy < REPRODUCTION_THRESHOLD { return; }
        let parent_node = self.organisms[idx].node;
        let sites: Vec<_> = self.edges[parent_node].iter().copied().filter(|&n| self.nodes[n] == NodeOccupant::Empty).collect();
        let Some(&site) = sites.choose(&mut self.rng) else { return; };
        self.organisms[idx].energy -= REPRODUCTION_COST;
        let child_energy = self.organisms[idx].energy / 2; self.organisms[idx].energy -= child_energy;
        let mut archive = self.organisms[idx].archive.clone(); mutate_archive(&mut archive, &mut self.next_module_id, &mut self.rng);
        let id = self.next_org_id; self.next_org_id += 1;
        let mut child = Organism { id, energy: child_energy.max(1), age: 0, archive, phenotype: empty_pheno(), alive: true, lineage_id: self.organisms[idx].lineage_id, parent_id: Some(self.organisms[idx].id), symbiont_links: self.organisms[idx].symbiont_links.clone(), local_memory: Vec::new(), node: site };
        decode_organism(&mut child); let ci = self.organisms.len(); self.organisms.push(child); self.nodes[site] = NodeOccupant::Organism(ci); self.counters.births += 1;
    }

    pub fn kill(&mut self, idx: usize) {
        if !self.organisms[idx].alive { return; }
        self.organisms[idx].alive = false; let node = self.organisms[idx].node; let energy = self.organisms[idx].energy.max(0) + 15;
        let mut modules = self.organisms[idx].archive.dormant_modules.clone(); modules.extend(self.organisms[idx].phenotype.active_modules.clone()); modules.truncate(32);
        let tags = modules.iter().take(8).map(|m| m.tag).collect();
        let corpse = Corpse { id: self.next_corpse_id, former_organism_id: self.organisms[idx].id, modules, energy_value: energy, decay_timer: CORPSE_DECAY_TICKS, hardness: self.organisms[idx].defense(), tags, node };
        self.next_corpse_id += 1; let ci = self.corpses.len(); self.corpses.push(corpse); self.nodes[node] = NodeOccupant::Corpse(ci); self.counters.deaths += 1;
    }

    fn decay_corpses(&mut self) {
        for (ci, c) in self.corpses.iter_mut().enumerate() { if c.decay_timer > 0 { c.decay_timer -= 1; if self.tick % 5 == 0 { c.energy_value = c.energy_value.saturating_sub(1); } if c.decay_timer == 0 && self.nodes[c.node] == NodeOccupant::Corpse(ci) { self.nodes[c.node] = NodeOccupant::Empty; } } }
    }
}

pub fn empty_pheno() -> Phenotype { Phenotype { active_modules: Vec::new(), membrane_tokens: Vec::new(), call_preferences: Vec::new(), max_calls_per_tick: 2, decoder_strength: 18, defense_level: 0 } }

pub fn decode_organism(org: &mut Organism) {
    let mut scored = Vec::new();
    for m in &org.archive.dormant_modules {
        let d = org.archive.decoder_bias_tags.iter().map(|t| tag_distance(t, &m.tag)).min().unwrap_or(64);
        if d <= org.phenotype.decoder_strength.max(18) { scored.push((d, m.clone())); }
    }
    scored.sort_by_key(|(d,_)| *d);
    let limit = 6.min(scored.len()); org.phenotype.active_modules = scored.into_iter().take(limit).map(|(_,m)| m).collect();
    if org.phenotype.active_modules.is_empty() { org.phenotype.active_modules = org.archive.dormant_modules.iter().take(2).cloned().collect(); }
    org.phenotype.call_preferences = org.phenotype.active_modules.iter().map(|m| m.tag).take(4).collect();
    org.phenotype.membrane_tokens = org.archive.decoder_bias_tags.iter().take(2).copied().collect();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn death_creates_corpse_at_same_node() {
        let mut w = World::new(1, 4, 3); w.spawn_random_at(0, None, None); w.organisms[0].energy = -1; w.kill(0);
        assert!(!w.organisms[0].alive); assert!(matches!(w.nodes[0], NodeOccupant::Corpse(0))); assert_eq!(w.corpses[0].former_organism_id, 1);
    }
    #[test]
    fn reproduction_creates_mutated_child_archive() {
        let mut w = World::new(2, 5, 4); w.randomize_edges(); w.spawn_random_at(0, None, None); w.nodes[1] = NodeOccupant::Empty; w.add_edge(0,1); w.organisms[0].energy = 200; w.organisms[0].archive.mutation_rate = 1.0; let before = w.organisms[0].archive.dormant_modules.clone(); w.try_reproduce(0);
        assert_eq!(w.counters.births, 1); assert_ne!(w.organisms[1].archive.dormant_modules, before);
    }
    #[test]
    fn maintenance_applies_resource_costs() {
        let mut w = World::new(3, 3, 2); w.spawn_random_at(0, None, None); let e = w.organisms[0].energy; w.pay_maintenance(0); assert!(w.organisms[0].energy < e);
    }
    #[test]
    fn local_candidates_exclude_non_neighbors() {
        let mut w = World::new(4, 3, 2); w.spawn_random_at(0, None, None); w.spawn_random_at(2, None, None); let c = w.local_candidates(0); assert!(!c.iter().any(|(owner,_,_)| *owner == ModuleOwner::Neighbor(1))); w.add_edge(0,2); let c = w.local_candidates(0); assert!(c.iter().any(|(owner,_,_)| *owner == ModuleOwner::Neighbor(1)));
    }

    #[test]
    fn corpse_modules_are_scan_candidates_but_not_call_candidates() {
        let mut w = World::new(5, 3, 2); w.spawn_random_at(0, None, None); w.spawn_random_at(1, None, None); w.add_edge(0, 1); w.kill(1);
        assert!(w.local_candidates(0).iter().any(|(owner,_,_)| matches!(owner, ModuleOwner::Corpse(0))));
        assert!(!w.call_candidates(0).iter().any(|(owner,_,_)| matches!(owner, ModuleOwner::Corpse(_))));
    }

    #[test]
    fn executing_corpse_module_has_no_effect() {
        let mut w = World::new(6, 3, 2); w.spawn_random_at(0, None, None); w.spawn_random_at(1, None, None); w.add_edge(0, 1); w.kill(1);
        let energy = w.organisms[0].energy; w.execute_module(0, ModuleOwner::Corpse(0), 0);
        assert_eq!(w.organisms[0].energy, energy);
    }

    #[test]
    fn reproduction_requires_neighboring_empty_site() {
        let mut w = World::new(7, 4, 3); w.spawn_random_at(0, None, None); w.spawn_random_at(1, None, None); w.add_edge(0, 1); w.organisms[0].energy = 200; w.try_reproduce(0);
        assert_eq!(w.counters.births, 0);
        assert_eq!(w.organisms.len(), 2);
    }

    #[test]
    fn trade_conserves_pair_energy_before_module_costs() {
        let mut w = World::new(8, 3, 2); w.spawn_random_at(0, None, None); w.spawn_random_at(1, None, None); w.add_edge(0, 1);
        let before = w.organisms[0].energy + w.organisms[1].energy; w.trade(0); let after = w.organisms[0].energy + w.organisms[1].energy;
        assert_eq!(after, before);
    }

    #[test]
    fn same_seed_reproducible_for_core_counters() {
        fn run() -> (usize, usize, u64, u64, u64) { let mut w = World::new(9, 30, 4); w.randomize_edges(); w.seed_random_organisms(10); for _ in 0..25 { w.step(); } let living = w.organisms.iter().filter(|o| o.alive).count(); (living, w.corpses.len(), w.counters.births, w.counters.deaths, w.counters.hgt) }
        assert_eq!(run(), run());
    }

    #[test]
    fn max_degree_survives_rewiring_and_reproduction() {
        let mut w = World::new(10, 40, 4); w.randomize_edges(); w.seed_random_organisms(15); for _ in 0..50 { w.step(); assert!(w.edges.iter().all(|e| e.len() <= w.max_degree)); }
    }

    #[test]
    fn corpse_decay_clears_node() {
        let mut w = World::new(11, 3, 2); w.spawn_random_at(0, None, None); w.kill(0); let node = w.corpses[0].node; for _ in 0..CORPSE_DECAY_TICKS { w.tick += 1; w.decay_corpses(); }
        assert_eq!(w.nodes[node], NodeOccupant::Empty);
    }

    #[test]
    fn fuzzy_selection_can_select_neighbor_module() {
        let mut w = World::new(12, 3, 2); w.spawn_random_at(0, None, None); w.spawn_random_at(1, None, None); w.add_edge(0, 1);
        let tag = [7u8;8]; w.organisms[0].phenotype.active_modules.clear(); w.organisms[1].phenotype.active_modules[0].tag = tag;
        let c = w.call_candidates(0); let hit = fuzzy_select_module(&tag, &c, 0, &mut w.rng).unwrap();
        assert_eq!(hit.owner, ModuleOwner::Neighbor(1));
    }
}
