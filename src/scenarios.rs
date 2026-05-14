use crate::{config::Scenario, module::{Module, ModuleKind}, tag::random_tag, world::World};

pub fn initialize(world: &mut World, scenario: &Scenario, organisms: usize) {
    world.randomize_edges(); world.seed_random_organisms(organisms);
    match scenario { Scenario::Random => {}, Scenario::ParasiteTest => seed_kind(world, ModuleKind::Trade), Scenario::NecromassTest => seed_necromass(world), Scenario::SymbiosisTest => seed_kind(world, ModuleKind::Copy) }
}

fn seed_kind(world: &mut World, kind: ModuleKind) {
    if let Some(o) = world.organisms.get_mut(0) {
        let tag = random_tag(&mut world.rng);
        o.archive.dormant_modules.push(Module { id: world.next_module_id, tag, kind, cost: 1, payload: 0 }); world.next_module_id += 1;
        o.archive.decoder_bias_tags.push(tag);
    }
}

fn seed_necromass(world: &mut World) {
    let victims: Vec<_> = world.organisms.iter().enumerate().take(3).map(|(i,_)| i).collect();
    for i in victims { world.kill(i); }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_scenarios_initialize_and_step() {
        for scenario in [Scenario::Random, Scenario::ParasiteTest, Scenario::NecromassTest, Scenario::SymbiosisTest] {
            let mut world = World::new(13, 20, 4);
            initialize(&mut world, &scenario, 8);
            for _ in 0..5 { world.step(); }
            assert!(world.organisms.len() >= 5);
            assert!(world.edges.iter().all(|e| e.len() <= world.max_degree));
        }
    }
}
