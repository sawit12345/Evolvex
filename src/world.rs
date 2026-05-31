//! The world: a bounded-degree graph of nodes, each holding an organism, a
//! corpse, or nothing, plus a regenerating environmental `resource`. This file
//! owns the simulation loop and the low-level mechanics (maintenance, harvest,
//! predation, infection, scavenging, reproduction, death, decay). The decision
//! layer (which action to take, and the fuzzy-call dispatch) lives in
//! `actions.rs` as additional `impl World` methods.

use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use crate::config::*;
use crate::corpse::Corpse;
use crate::metrics::Counters;
use crate::module::{random_module, Module, ModuleKind};
use crate::mutation::{mutate_archive, somatic_mutation};
use crate::organism::{Archive, Organism, Phenotype};
use crate::tag::random_tag;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeOccupant {
    Empty,
    Organism(usize),
    Corpse(usize),
}

pub struct World {
    pub tick: u64,
    pub rng: ChaCha8Rng,
    pub nodes: Vec<NodeOccupant>,
    /// Per-node environmental energy. The *only* primary production: it
    /// regenerates slowly and is the global bound on free energy.
    pub resources: Vec<i64>,
    pub edges: Vec<Vec<usize>>,
    pub organisms: Vec<Organism>,
    pub corpses: Vec<Corpse>,
    pub max_degree: usize,
    pub counters: Counters,
    pub next_org_id: u64,
    pub next_corpse_id: u64,
    pub next_module_id: u64,
}

impl World {
    pub fn new(seed: u64, nodes: usize, max_degree: usize) -> Self {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let resources = (0..nodes).map(|_| rng.gen_range(0..=RESOURCE_MAX)).collect();
        Self {
            tick: 0,
            rng,
            nodes: vec![NodeOccupant::Empty; nodes],
            resources,
            edges: vec![Vec::new(); nodes],
            organisms: Vec::new(),
            corpses: Vec::new(),
            max_degree,
            counters: Counters::default(),
            next_org_id: 1,
            next_corpse_id: 1,
            next_module_id: 1,
        }
    }

    // ---- graph -------------------------------------------------------------

    pub fn randomize_edges(&mut self) {
        let n = self.nodes.len();
        if n < 2 {
            return;
        }
        for i in 0..n {
            for _ in 0..self.max_degree {
                let j = self.rng.gen_range(0..n);
                if i != j {
                    self.add_edge(i, j);
                }
            }
        }
    }

    pub fn add_edge(&mut self, a: usize, b: usize) -> bool {
        if a == b
            || self.edges[a].contains(&b)
            || self.edges[a].len() >= self.max_degree
            || self.edges[b].len() >= self.max_degree
        {
            return false;
        }
        self.edges[a].push(b);
        self.edges[b].push(a);
        true
    }

    pub fn remove_edge(&mut self, a: usize, b: usize) {
        self.edges[a].retain(|x| *x != b);
        self.edges[b].retain(|x| *x != a);
    }

    pub fn alloc_module_id(&mut self) -> u64 {
        let id = self.next_module_id;
        self.next_module_id += 1;
        id
    }

    // ---- seeding -----------------------------------------------------------

    pub fn seed_random_organisms(&mut self, count: usize) {
        let mut sites: Vec<_> = (0..self.nodes.len())
            .filter(|&n| self.nodes[n] == NodeOccupant::Empty)
            .collect();
        sites.shuffle(&mut self.rng);
        for node in sites.into_iter().take(count) {
            self.spawn_random_at(node, None, None);
        }
    }

    pub fn spawn_random_at(&mut self, node: usize, lineage: Option<u64>, parent: Option<u64>) -> usize {
        let id = self.next_org_id;
        self.next_org_id += 1;
        let lineage_id = lineage.unwrap_or(id);

        let mut dormant = Vec::new();
        let n_mods = self.rng.gen_range(8..=14);
        for _ in 0..n_mods {
            let mid = self.alloc_module_id();
            dormant.push(random_module(mid, &mut self.rng));
        }
        // Guarantee at least one primary-production gene so naive life can start.
        if !dormant.iter().any(|m| m.kind == ModuleKind::Harvest) {
            let mid = self.alloc_module_id();
            dormant.push(Module {
                id: mid,
                tag: random_tag(&mut self.rng),
                kind: ModuleKind::Harvest,
                cost: 1,
                payload: 0,
            });
        }
        let archive = Archive {
            dormant_modules: dormant,
            decoder_bias_tags: (0..3).map(|_| random_tag(&mut self.rng)).collect(),
            mutation_rate: self.rng.gen_range(0.01..=0.05),
            storage_cost_modifier: 1.0,
        };
        let mut org = Organism {
            id,
            energy: STARTING_ENERGY,
            age: 0,
            archive,
            phenotype: new_phenotype(self.rng.gen_range(12..=26)),
            alive: true,
            lineage_id,
            parent_id: parent,
            symbiont_links: Vec::new(),
            local_memory: Vec::new(),
            node,
        };
        org.decode();
        let idx = self.organisms.len();
        self.organisms.push(org);
        self.nodes[node] = NodeOccupant::Organism(idx);
        idx
    }

    /// Insert a fully-formed organism (used by hand-seeded scenario creatures).
    pub fn insert_organism(&mut self, mut org: Organism, node: usize) -> usize {
        org.node = node;
        org.decode();
        let idx = self.organisms.len();
        self.organisms.push(org);
        self.nodes[node] = NodeOccupant::Organism(idx);
        idx
    }

    // ---- main loop ---------------------------------------------------------

    pub fn step(&mut self) {
        self.tick += 1;

        // Primary production: every node regenerates a little energy.
        for r in &mut self.resources {
            *r = (*r + RESOURCE_REGEN).min(RESOURCE_MAX);
        }

        // Deterministic shuffle of the living for fair, reproducible ordering.
        let mut order: Vec<usize> = self
            .organisms
            .iter()
            .enumerate()
            .filter(|(_, o)| o.alive)
            .map(|(i, _)| i)
            .collect();
        order.shuffle(&mut self.rng);

        for idx in order {
            if idx < self.organisms.len() && self.organisms[idx].alive {
                self.organism_tick(idx);
            }
        }

        self.decay_corpses();
    }

    fn organism_tick(&mut self, idx: usize) {
        // (a) maintenance + passive feeding
        self.pay_maintenance(idx);
        // (b) starvation
        if self.organisms[idx].energy <= 0 {
            self.kill(idx);
            return;
        }
        self.organisms[idx].age += 1;
        // defense bleeds off (transient)
        let d = &mut self.organisms[idx].phenotype.defense_level;
        *d = (*d - DEFENSE_DECAY).max(0);

        // (c) re-decode archive -> phenotype (cheap drift in expression)
        self.organisms[idx].decode();

        // (d,e) take up to max_calls_per_tick actions, each resolved with cost
        let turns = self.organisms[idx].phenotype.max_calls_per_tick.clamp(1, 3);
        for _ in 0..turns {
            if !self.organisms[idx].alive {
                return;
            }
            let action = self.choose_action(idx);
            self.resolve_action(idx, action);
            if self.organisms[idx].alive && self.organisms[idx].energy <= 0 {
                self.kill(idx);
                return;
            }
        }

        // (f) possible somatic mutation of the (cheap) archive
        if self.rng.gen_bool(SOMATIC_MUTATION_CHANCE) {
            somatic_mutation(
                &mut self.organisms[idx].archive,
                &mut self.next_module_id,
                &mut self.rng,
            );
        }
    }

    fn pay_maintenance(&mut self, idx: usize) {
        let active = self.organisms[idx].phenotype.active_modules.len() as i64 * ACTIVE_MODULE_COST;
        let stored = self.organisms[idx].archive.dormant_modules.len() as i64;
        let archive_cost =
            ((stored + ARCHIVE_STORAGE_DIVISOR - 1) / ARCHIVE_STORAGE_DIVISOR).max(0);
        self.organisms[idx].energy -= BASE_METABOLISM_COST + active + archive_cost;

        // Passive sessile feeding from the local node resource.
        let node = self.organisms[idx].node;
        let absorb = PASSIVE_ABSORB.min(self.resources[node]).max(0);
        self.resources[node] -= absorb;
        self.organisms[idx].energy += absorb;
    }

    // ---- mechanics ---------------------------------------------------------

    pub fn neighbor_orgs(&self, idx: usize) -> Vec<usize> {
        let node = self.organisms[idx].node;
        self.edges[node]
            .iter()
            .filter_map(|&n| match self.nodes[n] {
                NodeOccupant::Organism(oi) if oi != idx && self.organisms[oi].alive => Some(oi),
                _ => None,
            })
            .collect()
    }

    pub fn neighbor_corpses(&self, idx: usize) -> Vec<usize> {
        let node = self.organisms[idx].node;
        self.edges[node]
            .iter()
            .filter_map(|&n| match self.nodes[n] {
                NodeOccupant::Corpse(ci) if self.corpses[ci].energy_value > 0 => Some(ci),
                _ => None,
            })
            .collect()
    }

    /// Harvest weak primary energy from the local node resource.
    pub fn harvest(&mut self, idx: usize) {
        let node = self.organisms[idx].node;
        let draw = HARVEST_DRAW_MAX.min(self.resources[node]).max(0);
        self.resources[node] -= draw;
        self.organisms[idx].energy += draw + HARVEST_FLAT;
        self.counters.harvests += 1;
    }

    /// Predation: damage a neighbour and steal part of the damage as energy.
    pub fn attack(&mut self, attacker: usize) {
        self.organisms[attacker].energy -= ATTACK_COST;
        let victims = self.neighbor_orgs(attacker);
        let Some(&v) = victims.choose(&mut self.rng) else {
            return;
        };
        self.bite(attacker, v);
    }

    /// `attacker` bites `victim` (used both for self-attacks and for the
    /// "anglerfish" case where a caller hits an exposed Attack module and the
    /// provider bites back).
    pub fn bite(&mut self, attacker: usize, victim: usize) {
        if attacker == victim || !self.organisms[victim].alive {
            return;
        }
        let dmg = (ATTACK_DAMAGE - self.organisms[victim].defense() * DEFENSE_REDUCTION).max(1);
        let steal = (dmg * ATTACK_STEAL_PERCENT / 100).min(self.organisms[victim].energy.max(0));
        self.organisms[victim].energy -= dmg;
        self.organisms[attacker].energy += steal;
        self.counters.attacks += 1;
        if self.organisms[victim].energy <= 0 {
            self.kill(victim);
        }
    }

    /// Eat a neighbouring corpse; occasionally salvage a module (HGT from dead).
    pub fn scavenge(&mut self, idx: usize) {
        let corpses = self.neighbor_corpses(idx);
        let Some(&ci) = corpses.choose(&mut self.rng) else {
            return;
        };
        let bite = self.corpses[ci].bite_size(SCAVENGE_GAIN_MAX);
        self.corpses[ci].energy_value -= bite;
        self.organisms[idx].energy += bite;
        self.counters.scavenges += 1;
        if self.rng.gen_bool(0.25) {
            self.salvage_from_corpse(idx, ci);
        }
    }

    fn salvage_from_corpse(&mut self, idx: usize, ci: usize) {
        if self.organisms[idx].archive.dormant_modules.len() >= MAX_ARCHIVE_MODULES {
            return;
        }
        if let Some(m) = self.corpses[ci].modules.choose(&mut self.rng).cloned() {
            self.organisms[idx].energy -= COPY_MODULE_COST;
            self.add_archive_module(idx, m);
        }
    }

    /// Self-initiated horizontal gene transfer: pull a module from a living
    /// neighbour or a corpse into our own (cheap, dormant) archive.
    pub fn copy_into_archive(&mut self, idx: usize) {
        if self.organisms[idx].archive.dormant_modules.len() >= MAX_ARCHIVE_MODULES {
            return;
        }
        self.organisms[idx].energy -= COPY_MODULE_COST;
        let corpses = self.neighbor_corpses(idx);
        if let Some(&ci) = corpses.choose(&mut self.rng) {
            if let Some(m) = self.corpses[ci].modules.choose(&mut self.rng).cloned() {
                self.add_archive_module(idx, m);
                return;
            }
        }
        let neigh: Vec<usize> = self
            .neighbor_orgs(idx)
            .into_iter()
            .filter(|&o| !self.organisms[o].archive.dormant_modules.is_empty())
            .collect();
        if let Some(&oi) = neigh.choose(&mut self.rng) {
            let m = self.organisms[oi]
                .archive
                .dormant_modules
                .choose(&mut self.rng)
                .unwrap()
                .clone();
            self.add_archive_module(idx, m);
        }
    }

    /// Infection / injection: `provider` pushes one of its *expressed* modules
    /// into `target`'s archive. This is how a parasitic gene (e.g. a Copy
    /// module that taxes callers) spreads horizontally through the population.
    pub fn infect(&mut self, provider: usize, target: usize) {
        if target == provider
            || !self.organisms[target].alive
            || self.organisms[target].archive.dormant_modules.len() >= MAX_ARCHIVE_MODULES
        {
            return;
        }
        let resist = (self.organisms[target].defense() as f64 * INFECTION_RESIST_PER_DEFENSE)
            .clamp(0.0, 1.0);
        let chance = (INFECTION_BASE_CHANCE - resist).clamp(0.0, 1.0);
        if !self.rng.gen_bool(chance) {
            return;
        }
        let pool = &self.organisms[provider].phenotype.active_modules;
        if pool.is_empty() {
            return;
        }
        let m = pool[self.rng.gen_range(0..pool.len())].clone();
        self.add_archive_module(target, m);
        self.counters.infections += 1;
    }

    fn add_archive_module(&mut self, idx: usize, mut m: Module) {
        m.id = self.alloc_module_id();
        self.organisms[idx].archive.dormant_modules.push(m);
        if self.organisms[idx].archive.dormant_modules.len() > MAX_ARCHIVE_MODULES {
            self.organisms[idx].archive.dormant_modules.remove(0);
        }
        self.counters.hgt += 1;
    }

    /// Cooperative redistribution. `provider` shares a slice of its surplus
    /// with the caller; the caller has already paid the provider's price, so a
    /// cheap-priced Trade module is mutualistic and a steep one is exploitative.
    pub fn trade_dividend(&mut self, provider: usize, caller: usize) {
        let surplus = self.organisms[provider].energy;
        if surplus <= 0 {
            return;
        }
        let dividend = (surplus / 8).clamp(0, TRADE_MAX);
        self.organisms[provider].energy -= dividend;
        self.organisms[caller].energy += dividend;
    }

    pub fn rewire(&mut self, idx: usize) {
        self.organisms[idx].energy -= EDGE_REWIRE_COST;
        let node = self.organisms[idx].node;
        if let Some(&old) = self.edges[node].choose(&mut self.rng) {
            self.remove_edge(node, old);
        }
        for _ in 0..16 {
            let new = self.rng.gen_range(0..self.nodes.len());
            if self.add_edge(node, new) {
                break;
            }
        }
    }

    pub fn try_reproduce(&mut self, idx: usize) {
        if self.organisms[idx].energy < REPRODUCTION_THRESHOLD {
            return;
        }
        let parent_node = self.organisms[idx].node;
        // Prefer empty sites adjacent to a recorded symbiont (place child near
        // a beneficial partner); otherwise any empty neighbour.
        let empty: Vec<usize> = self.edges[parent_node]
            .iter()
            .copied()
            .filter(|&n| self.nodes[n] == NodeOccupant::Empty)
            .collect();
        let Some(&site) = self.pick_birth_site(idx, &empty) else {
            return;
        };

        self.organisms[idx].energy -= REPRODUCTION_COST;
        let child_energy = (self.organisms[idx].energy / 2).max(1);
        self.organisms[idx].energy -= child_energy;

        let mut archive = self.organisms[idx].archive.clone();
        let mut_cost = mutate_archive(&mut archive, &mut self.next_module_id, &mut self.rng);
        self.organisms[idx].energy -= mut_cost; // charge for germline mutation events

        // Symbiosis: occasionally fold a module copied from a symbiont into the
        // child's archive (vertical inheritance of a partnership gene).
        self.maybe_inherit_symbiont_gene(idx, &mut archive);

        let id = self.next_org_id;
        self.next_org_id += 1;
        let mut child = Organism {
            id,
            energy: child_energy,
            age: 0,
            archive,
            phenotype: new_phenotype(self.organisms[idx].phenotype.decoder_strength),
            alive: true,
            lineage_id: self.organisms[idx].lineage_id,
            parent_id: Some(self.organisms[idx].id),
            symbiont_links: self.organisms[idx].symbiont_links.clone(),
            local_memory: Vec::new(),
            node: site,
        };
        child.decode();
        let ci = self.organisms.len();
        self.organisms.push(child);
        self.nodes[site] = NodeOccupant::Organism(ci);
        self.counters.births += 1;
    }

    fn pick_birth_site<'a>(&self, idx: usize, empty: &'a [usize]) -> Option<&'a usize> {
        if empty.is_empty() {
            return None;
        }
        // Site bordering a symbiont is preferred.
        let symbionts = &self.organisms[idx].symbiont_links;
        let preferred = empty.iter().find(|&&site| {
            self.edges[site].iter().any(|&n| match self.nodes[n] {
                NodeOccupant::Organism(oi) => symbionts.contains(&self.organisms[oi].id),
                _ => false,
            })
        });
        preferred.or_else(|| empty.first())
    }

    fn maybe_inherit_symbiont_gene(&mut self, idx: usize, archive: &mut Archive) {
        if archive.dormant_modules.len() >= MAX_ARCHIVE_MODULES {
            return;
        }
        if self.organisms[idx].symbiont_links.is_empty() || !self.rng.gen_bool(0.3) {
            return;
        }
        let partner_id = *self.organisms[idx]
            .symbiont_links
            .choose(&mut self.rng)
            .unwrap();
        if let Some(p) = self.organisms.iter().find(|o| o.id == partner_id && o.alive) {
            if let Some(m) = p.phenotype.active_modules.choose(&mut self.rng).cloned() {
                let mut m = m;
                m.id = self.next_module_id;
                self.next_module_id += 1;
                archive.dormant_modules.push(m);
            }
        }
    }

    // ---- death & necromass -------------------------------------------------

    pub fn kill(&mut self, idx: usize) {
        if !self.organisms[idx].alive {
            return;
        }
        self.organisms[idx].alive = false;
        let node = self.organisms[idx].node;
        // Corpse retains a guaranteed base energy plus any leftover (floored).
        let energy = self.organisms[idx].energy.max(0) + CORPSE_BASE_ENERGY;
        let mut modules = self.organisms[idx].archive.dormant_modules.clone();
        modules.extend(self.organisms[idx].phenotype.active_modules.clone());
        modules.truncate(32);
        let tags = modules.iter().take(8).map(|m| m.tag).collect();
        let corpse = Corpse {
            id: self.next_corpse_id,
            former_organism_id: self.organisms[idx].id,
            modules,
            energy_value: energy,
            decay_timer: CORPSE_DECAY_TICKS,
            hardness: self.organisms[idx].defense().max(0),
            tags,
            node,
        };
        self.next_corpse_id += 1;
        let ci = self.corpses.len();
        self.corpses.push(corpse);
        self.nodes[node] = NodeOccupant::Corpse(ci);
        self.counters.deaths += 1;
    }

    fn decay_corpses(&mut self) {
        let harden_floor = self.tick % 5 == 0;
        for (ci, c) in self.corpses.iter_mut().enumerate() {
            if c.decay_timer == 0 {
                continue;
            }
            // Hardened corpses decay more slowly: only soft ones decay every tick.
            let decays = c.hardness <= 0 || self.tick % 2 == 0;
            if decays {
                c.decay_timer -= 1;
            }
            // Energy bleeds off but never goes negative.
            if harden_floor {
                c.energy_value = (c.energy_value - 1).max(0);
            }
            if c.decay_timer == 0 && self.nodes[c.node] == NodeOccupant::Corpse(ci) {
                self.nodes[c.node] = NodeOccupant::Empty;
            }
        }
    }
}

/// A fresh, empty phenotype with the given (evolvable) decoder strength.
pub fn new_phenotype(decoder_strength: u32) -> Phenotype {
    Phenotype {
        active_modules: Vec::new(),
        membrane_tokens: Vec::new(),
        call_preferences: Vec::new(),
        max_calls_per_tick: 2,
        decoder_strength,
        defense_level: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn death_creates_corpse_at_same_node() {
        let mut w = World::new(1, 4, 3);
        w.spawn_random_at(0, None, None);
        w.organisms[0].energy = -1;
        w.kill(0);
        assert!(!w.organisms[0].alive);
        assert!(matches!(w.nodes[0], NodeOccupant::Corpse(0)));
        assert_eq!(w.corpses[0].former_organism_id, 1);
        assert!(w.corpses[0].energy_value >= CORPSE_BASE_ENERGY);
    }

    #[test]
    fn corpse_energy_never_goes_negative() {
        let mut w = World::new(2, 3, 2);
        w.spawn_random_at(0, None, None);
        w.kill(0);
        for _ in 0..(CORPSE_DECAY_TICKS * 4) {
            w.tick += 1;
            w.decay_corpses();
        }
        assert!(w.corpses.iter().all(|c| c.energy_value >= 0));
    }

    #[test]
    fn reproduction_creates_mutated_child_archive() {
        let mut w = World::new(3, 5, 4);
        w.randomize_edges();
        w.spawn_random_at(0, None, None);
        w.nodes[1] = NodeOccupant::Empty;
        w.add_edge(0, 1);
        w.organisms[0].energy = 300;
        w.organisms[0].archive.mutation_rate = 1.0;
        let before = w.organisms[0].archive.dormant_modules.clone();
        w.try_reproduce(0);
        assert_eq!(w.counters.births, 1);
        assert_ne!(w.organisms[1].archive.dormant_modules, before);
    }

    #[test]
    fn maintenance_applies_resource_costs() {
        let mut w = World::new(4, 3, 2);
        w.spawn_random_at(0, None, None);
        w.resources[0] = 0; // remove the passive-feeding offset
        let e = w.organisms[0].energy;
        w.pay_maintenance(0);
        assert!(w.organisms[0].energy < e);
    }

    #[test]
    fn predation_transfers_energy_and_can_kill() {
        let mut w = World::new(5, 3, 2);
        w.spawn_random_at(0, None, None);
        w.spawn_random_at(1, None, None);
        w.add_edge(0, 1);
        w.organisms[1].energy = 3;
        w.organisms[1].phenotype.defense_level = 0;
        let attacker_before = w.organisms[0].energy;
        w.bite(0, 1);
        assert!(!w.organisms[1].alive); // low-energy victim dies -> corpse
        assert!(w.organisms[0].energy >= attacker_before - ATTACK_COST); // gained some steal
        assert!(matches!(w.nodes[1], NodeOccupant::Corpse(_)));
    }

    #[test]
    fn infection_inserts_module_into_target_archive() {
        let mut w = World::new(6, 3, 2);
        w.spawn_random_at(0, None, None);
        w.spawn_random_at(1, None, None);
        w.add_edge(0, 1);
        w.organisms[1].phenotype.defense_level = 0;
        // Force determinism-friendly large archive headroom.
        let before = w.organisms[1].archive.dormant_modules.len();
        // Provider must have active modules to inject.
        assert!(!w.organisms[0].phenotype.active_modules.is_empty());
        for _ in 0..50 {
            w.infect(0, 1);
            if w.organisms[1].archive.dormant_modules.len() > before {
                break;
            }
        }
        assert!(w.organisms[1].archive.dormant_modules.len() > before);
        assert!(w.counters.infections >= 1);
    }

    #[test]
    fn same_seed_reproducible() {
        fn run() -> (usize, usize, u64, u64, u64, u64) {
            let mut w = World::new(9, 60, 4);
            w.randomize_edges();
            w.seed_random_organisms(25);
            for _ in 0..60 {
                w.step();
            }
            let living = w.organisms.iter().filter(|o| o.alive).count();
            (
                living,
                w.corpses.len(),
                w.counters.births,
                w.counters.deaths,
                w.counters.hgt,
                w.counters.calls,
            )
        }
        assert_eq!(run(), run());
    }

    #[test]
    fn max_degree_preserved_under_dynamics() {
        let mut w = World::new(10, 50, 4);
        w.randomize_edges();
        w.seed_random_organisms(20);
        for _ in 0..80 {
            w.step();
            assert!(w.edges.iter().all(|e| e.len() <= w.max_degree));
        }
    }
}
