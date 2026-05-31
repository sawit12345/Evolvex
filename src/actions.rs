//! The decision layer: how an organism picks an action each turn, and how a
//! fuzzy content-addressed call is dispatched.
//!
//! THE KEY SEMANTIC FIX vs. the old code: a call now has a *provider* and a
//! *caller* with distinct roles. When caller A calls provider B's exposed
//! module M:
//!   * A pays bandwidth (CALL_COST) and a price (M.payload, the tax B charges).
//!   * The module's effect then runs, and for parasitic kinds the *provider*
//!     acts on the caller:
//!       - Copy   -> B injects (infects) a module into A   (HGT / contagion)
//!       - Attack -> B bites A                              (the "anglerfish")
//!     while service kinds (Repair, Trade, Defend, ...) benefit A and earn B
//!     its tax. This makes mutualism, parasitism and the apex-predator strategy
//!     emergent rather than impossible.

use rand::seq::SliceRandom;
use rand::Rng;

use crate::config::*;
use crate::module::{fuzzy_select_module, Module, ModuleKind, ModuleOwner, ModuleRef};
use crate::tag::{jitter_tag, random_tag, Tag};
use crate::world::World;

/// The action an organism takes on a turn. Most behaviour routes through a
/// fuzzy `Call`; `Harvest` is the weak primary-production fallback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Call(ModuleRef),
    Harvest,
    Scavenge,
    Idle,
}

impl World {
    /// Build the desired call tag: usually a jittered favourite (so calls are
    /// *fuzzy*, not exact), sometimes deliberately aimed at a neighbour's
    /// exposed API, sometimes random exploration.
    pub fn desired_tag(&mut self, idx: usize) -> Tag {
        let roll: f64 = self.rng.gen();
        if roll < 0.3 {
            // Aim at a random neighbour's exposed module (seek a service).
            let neigh = self.neighbor_orgs(idx);
            if let Some(&oi) = neigh.choose(&mut self.rng) {
                let exposed = self.organisms[oi].exposed_modules();
                if let Some((_, tag)) = exposed.choose(&mut self.rng) {
                    return jitter_tag(tag, self.rng.gen_range(0..=2), &mut self.rng);
                }
            }
        }
        if roll < 0.85 {
            // Aim at a remembered / preferred tag, jittered.
            let prefs = &self.organisms[idx].phenotype.call_preferences;
            if !prefs.is_empty() {
                let t = prefs[self.rng.gen_range(0..prefs.len())];
                return jitter_tag(&t, self.rng.gen_range(0..=2), &mut self.rng);
            }
        }
        random_tag(&mut self.rng)
    }

    /// Candidate modules for a call: all of self's active modules plus only the
    /// *exposed* active modules of living neighbours. Corpses are inert and are
    /// never callable (they are only reachable via Scavenge/Copy).
    pub fn call_candidates(&self, idx: usize) -> Vec<(ModuleOwner, usize, Tag)> {
        let mut out = Vec::new();
        for (mi, m) in self.organisms[idx].phenotype.active_modules.iter().enumerate() {
            out.push((ModuleOwner::SelfOrg, mi, m.tag));
        }
        for oi in self.neighbor_orgs(idx) {
            for (mi, tag) in self.organisms[oi].exposed_modules() {
                out.push((ModuleOwner::Neighbor(oi), mi, tag));
            }
        }
        out
    }

    /// Pick this turn's action via fuzzy content-addressing.
    ///
    /// A hungry organism (low energy) overrides exploration with a survival
    /// reflex: scavenge a rich neighbouring corpse if one exists, else harvest.
    /// This is internal homeostasis, not an external objective — well-fed
    /// organisms still do the full range of (often risky) interactions, which
    /// is where diversity, predation and symbiosis come from.
    pub fn choose_action(&mut self, idx: usize) -> Action {
        if self.organisms[idx].energy < HUNGER_THRESHOLD {
            if !self.neighbor_corpses(idx).is_empty() && self.rng.gen_bool(0.5) {
                // Route through a synthetic self "scavenge" by direct call effect.
                return Action::Scavenge;
            }
            return Action::Harvest;
        }
        let desired = self.desired_tag(idx);
        let candidates = self.call_candidates(idx);
        match fuzzy_select_module(&desired, &candidates, MAX_FUZZY_DISTANCE, &mut self.rng) {
            Some(r) => Action::Call(r),
            None => Action::Harvest, // no neighbour answered; fall back to weak harvest
        }
    }

    pub fn resolve_action(&mut self, idx: usize, action: Action) {
        match action {
            Action::Harvest => self.harvest(idx),
            Action::Scavenge => self.scavenge(idx),
            Action::Idle => {}
            Action::Call(r) => self.execute_call(idx, r),
        }
    }

    /// Dispatch a fuzzy call. `r.owner` is either the caller itself or a
    /// neighbour. (A corpse owner cannot occur here because corpses are not in
    /// the candidate set; the guard keeps the function total.)
    pub fn execute_call(&mut self, caller: usize, r: ModuleRef) {
        let provider = match r.owner {
            ModuleOwner::SelfOrg => None,
            ModuleOwner::Neighbor(b) => Some(b),
            ModuleOwner::Corpse(_) => return, // inert: never executed
        };

        // Fetch a clone of the targeted module.
        let module: Option<Module> = match r.owner {
            ModuleOwner::SelfOrg => self.organisms[caller]
                .phenotype
                .active_modules
                .get(r.module_index)
                .cloned(),
            ModuleOwner::Neighbor(b) => self.organisms[b]
                .phenotype
                .active_modules
                .get(r.module_index)
                .cloned(),
            ModuleOwner::Corpse(_) => None,
        };
        let Some(module) = module else {
            return;
        };

        let energy_before = self.organisms[caller].energy;

        // Activation cost is always paid by whoever runs the module.
        self.organisms[caller].energy -= module.cost.max(0);

        // Cross-organism call: pay bandwidth + the provider's price (the tax).
        if let Some(b) = provider {
            self.organisms[caller].energy -= CALL_COST;
            let tax = module.payload.clamp(0, MAX_TAX);
            self.organisms[caller].energy -= tax;
            self.organisms[b].energy += tax;
            self.counters.calls += 1;
        }

        self.apply_effect(caller, provider, module.kind);

        // Reinforcement / symbiosis: if calling a neighbour left us better off,
        // remember the partner and the productive tag.
        if let Some(b) = provider {
            if self.organisms[caller].alive && self.organisms[caller].energy > energy_before {
                let partner_id = self.organisms[b].id;
                self.organisms[caller].reinforce(partner_id, module.tag);
            }
        }
    }

    /// Apply a module's kind-effect. `provider` is `Some(b)` for a neighbour's
    /// module (enabling parasitic provider->caller actions) and `None` for a
    /// self-activated module.
    fn apply_effect(&mut self, caller: usize, provider: Option<usize>, kind: ModuleKind) {
        match kind {
            ModuleKind::Harvest => self.harvest(caller),
            ModuleKind::Scavenge => self.scavenge(caller),
            ModuleKind::Decode => self.organisms[caller].decode(),
            ModuleKind::Reproduce => self.try_reproduce(caller),
            ModuleKind::MoveEdge => self.rewire(caller),
            ModuleKind::Noop => {}

            ModuleKind::Defend => {
                self.organisms[caller].phenotype.defense_level += 2;
            }
            ModuleKind::Repair => {
                // A repair *service* is funded by the provider; self-repair is
                // free of transfer but still bounded.
                if let Some(b) = provider {
                    let heal = SERVICE_REPAIR_HEAL.min(self.organisms[b].energy.max(0));
                    self.organisms[b].energy -= heal;
                    self.organisms[caller].energy += heal;
                } else {
                    self.organisms[caller].phenotype.defense_level += 1;
                }
            }
            ModuleKind::Trade => {
                if let Some(b) = provider {
                    self.trade_dividend(b, caller);
                } else if let Some(&p) = self.neighbor_orgs(caller).choose(&mut self.rng) {
                    // Self-initiated trade: smooth energy toward the poorer of
                    // the pair (cheap cooperation), and remember the partner.
                    self.trade_dividend(caller, p);
                    let pid = self.organisms[p].id;
                    let mtag = random_tag(&mut self.rng);
                    self.organisms[caller].reinforce(pid, mtag);
                }
            }
            ModuleKind::Auth => {
                // A toll-gate: the provider already collected its tax and
                // hardens itself; gives the caller nothing.
                if let Some(b) = provider {
                    self.organisms[b].phenotype.defense_level += 1;
                } else {
                    self.organisms[caller].phenotype.defense_level += 2;
                }
            }
            ModuleKind::Copy => match provider {
                // Called Copy => the provider injects a module into the caller
                // (infection / contagious horizontal gene transfer).
                Some(b) => self.infect(b, caller),
                // Self Copy => pull a module from a neighbour or corpse.
                None => self.copy_into_archive(caller),
            },
            ModuleKind::Attack => match provider {
                // Called Attack => the provider bites the caller (anglerfish).
                Some(b) => self.bite(b, caller),
                None => self.attack(caller),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::ModuleOwner;

    #[test]
    fn graph_locality_prevents_non_neighbor_calls() {
        use crate::module::{Module, ModuleKind};
        let mut w = World::new(1, 4, 3);
        w.spawn_random_at(0, None, None);
        w.spawn_random_at(2, None, None);
        // Give organism 1 a guaranteed-exposed module (tag == membrane token).
        let tag = [3u8; 8];
        w.organisms[1].phenotype.active_modules = vec![Module {
            id: 1,
            tag,
            kind: ModuleKind::Harvest,
            cost: 1,
            payload: 0,
        }];
        w.organisms[1].phenotype.membrane_tokens = vec![tag];

        // Not connected: organism 1 is not a candidate for organism 0, even
        // though it exposes a module — graph locality forbids the call.
        let c = w.call_candidates(0);
        assert!(!c.iter().any(|(o, _, _)| matches!(o, ModuleOwner::Neighbor(1))));
        // Connect them: now the exposed module becomes callable.
        w.add_edge(0, 2);
        let c = w.call_candidates(0);
        assert!(c.iter().any(|(o, _, _)| matches!(o, ModuleOwner::Neighbor(1))));
    }

    #[test]
    fn corpses_are_never_call_candidates() {
        let mut w = World::new(2, 3, 2);
        w.spawn_random_at(0, None, None);
        w.spawn_random_at(1, None, None);
        w.add_edge(0, 1);
        w.kill(1);
        let c = w.call_candidates(0);
        assert!(!c.iter().any(|(o, _, _)| matches!(o, ModuleOwner::Corpse(_))));
    }

    #[test]
    fn calling_neighbor_pays_tax_to_provider() {
        let mut w = World::new(3, 3, 2);
        w.spawn_random_at(0, None, None);
        w.spawn_random_at(1, None, None);
        w.add_edge(0, 1);
        // Give the provider a single, exposed, taxing Noop module so the call
        // has a deterministic, side-effect-free outcome except the tax.
        let tag = [9u8; 8];
        w.organisms[1].phenotype.active_modules = vec![Module {
            id: 999,
            tag,
            kind: ModuleKind::Noop,
            cost: 0,
            payload: 5,
        }];
        w.organisms[1].phenotype.membrane_tokens = vec![tag];
        let caller_before = w.organisms[0].energy;
        let provider_before = w.organisms[1].energy;
        let r = ModuleRef { owner: ModuleOwner::Neighbor(1), module_index: 0, distance: 0 };
        w.execute_call(0, r);
        // caller paid CALL_COST + tax(5); provider gained tax(5).
        assert_eq!(w.organisms[0].energy, caller_before - CALL_COST - 5);
        assert_eq!(w.organisms[1].energy, provider_before + 5);
    }
}
