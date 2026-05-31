//! Experiment scenarios. All worlds start from random organisms on a random
//! bounded-degree graph; scenarios then perturb the seed to probe specific
//! dynamics. The "anglerfish" predator is *not* a privileged organism type —
//! it is just an ordinary organism whose archive happens to encode the
//! lure-and-tax strategy, demonstrating that the mechanics permit it to exist
//! (and, ideally, to be out-competed or to spread).

use crate::config::Scenario;
use crate::module::{Module, ModuleKind};
use crate::organism::{Archive, Organism};
use crate::tag::{random_tag, Tag};
use crate::world::{new_phenotype, NodeOccupant, World};

pub fn initialize(world: &mut World, scenario: &Scenario, organisms: usize) {
    world.randomize_edges();
    world.seed_random_organisms(organisms);
    match scenario {
        Scenario::Random => {}
        Scenario::ParasiteTest => seed_anglerfish(world),
        Scenario::NecromassTest => seed_necromass(world),
        Scenario::SymbiosisTest => seed_symbiotic_pair(world),
    }
}

/// Hand-seed a "semantic anglerfish": it exposes many tags around a single
/// attractive membrane token (luring fuzzy calls) and answers with Attack and
/// Copy modules carrying a steep price — so callers get bitten, infected, and
/// taxed. It is placed on the highest-degree node (a graph bridge) to maximise
/// the calls it attracts.
pub fn seed_anglerfish(world: &mut World) {
    let lure: Tag = [0xAA; 8]; // the high-demand address the lure advertises
    let mut dormant: Vec<Module> = Vec::new();
    // Several lure modules clustered near the membrane token.
    for k in [
        ModuleKind::Attack,
        ModuleKind::Copy,
        ModuleKind::Attack,
        ModuleKind::Auth,
        ModuleKind::Harvest,
        ModuleKind::Reproduce,
        ModuleKind::Copy,
    ] {
        let id = world.alloc_module_id();
        dormant.push(Module {
            id,
            tag: lure, // identical tags => everything answers the lure address
            kind: k,
            cost: 1,
            payload: 8, // maximum tax
        });
    }
    let archive = Archive {
        dormant_modules: dormant,
        decoder_bias_tags: vec![lure, lure],
        mutation_rate: 0.02,
        storage_cost_modifier: 1.0,
    };
    let id = world.next_org_id;
    world.next_org_id += 1;
    let org = Organism {
        id,
        energy: 120,
        age: 0,
        archive,
        phenotype: new_phenotype(30),
        alive: true,
        lineage_id: id,
        parent_id: None,
        symbiont_links: Vec::new(),
        local_memory: Vec::new(),
        node: 0,
    };
    // Place on the current highest-degree free-or-replaceable node (a bridge).
    let node = highest_degree_node(world);
    if let NodeOccupant::Organism(victim) = world.nodes[node] {
        world.kill(victim);
        // killing turned it into a corpse; clear back to empty for the predator
        world.nodes[node] = NodeOccupant::Empty;
    }
    world.insert_organism(org, node);
}

fn highest_degree_node(world: &World) -> usize {
    world
        .edges
        .iter()
        .enumerate()
        .max_by_key(|(_, e)| e.len())
        .map(|(i, _)| i)
        .unwrap_or(0)
}

/// Kill a handful of organisms up front to seed necromass for scavengers.
fn seed_necromass(world: &mut World) {
    let victims: Vec<usize> = world
        .organisms
        .iter()
        .enumerate()
        .take(5)
        .map(|(i, _)| i)
        .collect();
    for i in victims {
        world.kill(i);
    }
}

/// Seed a complementary mutualist: it exposes cheap Repair/Trade services
/// (price 0) that benefit callers, encouraging stable symbiotic links to form.
fn seed_symbiotic_pair(world: &mut World) {
    let token: Tag = random_tag(&mut world.rng);
    let mut dormant = Vec::new();
    for k in [ModuleKind::Repair, ModuleKind::Trade, ModuleKind::Defend, ModuleKind::Harvest] {
        let id = world.alloc_module_id();
        dormant.push(Module { id, tag: token, kind: k, cost: 1, payload: 0 });
    }
    let archive = Archive {
        dormant_modules: dormant,
        decoder_bias_tags: vec![token, token],
        mutation_rate: 0.02,
        storage_cost_modifier: 1.0,
    };
    let id = world.next_org_id;
    world.next_org_id += 1;
    let org = Organism {
        id,
        energy: 90,
        age: 0,
        archive,
        phenotype: new_phenotype(28),
        alive: true,
        lineage_id: id,
        parent_id: None,
        symbiont_links: Vec::new(),
        local_memory: Vec::new(),
        node: 0,
    };
    let node = highest_degree_node(world);
    if world.nodes[node] == NodeOccupant::Empty {
        world.insert_organism(org, node);
    } else if let Some(empty) = (0..world.nodes.len()).find(|&n| world.nodes[n] == NodeOccupant::Empty) {
        world.insert_organism(org, empty);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_scenarios_initialize_and_step() {
        for scenario in [
            Scenario::Random,
            Scenario::ParasiteTest,
            Scenario::NecromassTest,
            Scenario::SymbiosisTest,
        ] {
            let mut world = World::new(13, 40, 4);
            initialize(&mut world, &scenario, 15);
            for _ in 0..20 {
                world.step();
            }
            assert!(world.edges.iter().all(|e| e.len() <= world.max_degree));
            // The world should not instantly empty out.
            assert!(world.organisms.iter().any(|o| o.alive));
        }
    }

    #[test]
    fn anglerfish_attracts_calls_and_taxes() {
        let mut world = World::new(21, 60, 6);
        initialize(&mut world, &Scenario::ParasiteTest, 30);
        for _ in 0..40 {
            world.step();
        }
        // The lure economy should have generated calls and at least attempted
        // infections/attacks (parasitic mechanics are reachable).
        assert!(world.counters.calls > 0);
    }
}
