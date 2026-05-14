use std::collections::HashMap;
use crate::{actions::kind_name, module::ModuleKind, world::World};

#[derive(Default, Clone, Debug)]
pub struct Counters { pub calls: u64, pub attacks: u64, pub scavenges: u64, pub births: u64, pub deaths: u64, pub hgt: u64 }

pub struct Metrics { pub living: usize, pub corpses: usize, pub total_living_energy: i64, pub total_corpse_energy: i64, pub avg_archive: f64, pub avg_pheno: f64, pub avg_degree: f64, pub lineage_diversity: usize, pub largest_lineage_share: f64, pub module_kind_entropy: f64, pub module_tag_entropy: f64, pub kind_counts: HashMap<ModuleKind, usize> }

pub fn collect(world: &World) -> Metrics {
    let orgs: Vec<_> = world.organisms.iter().filter(|o| o.alive).collect();
    let living = orgs.len();
    let corpses = world.corpses.iter().filter(|c| c.decay_timer > 0).count();
    let total_living_energy = orgs.iter().map(|o| o.energy).sum();
    let total_corpse_energy = world.corpses.iter().map(|c| c.energy_value).sum();
    let avg_archive = avg(orgs.iter().map(|o| o.archive.dormant_modules.len() as f64));
    let avg_pheno = avg(orgs.iter().map(|o| o.phenotype.active_modules.len() as f64));
    let avg_degree = avg(world.edges.iter().map(|e| e.len() as f64));
    let mut lineage = HashMap::new();
    let mut kind_counts = HashMap::new();
    let mut tag_buckets = HashMap::new();
    for o in &orgs {
        *lineage.entry(o.lineage_id).or_insert(0usize) += 1;
        for m in o.archive.dormant_modules.iter().chain(o.phenotype.active_modules.iter()) {
            *kind_counts.entry(m.kind).or_insert(0usize) += 1;
            *tag_buckets.entry(m.tag[0]).or_insert(0usize) += 1;
        }
    }
    let largest = lineage.values().copied().max().unwrap_or(0);
    Metrics { living, corpses, total_living_energy, total_corpse_energy, avg_archive, avg_pheno, avg_degree, lineage_diversity: lineage.len(), largest_lineage_share: if living==0 {0.0} else {largest as f64 / living as f64}, module_kind_entropy: entropy(kind_counts.values().copied()), module_tag_entropy: entropy(tag_buckets.values().copied()), kind_counts }
}

fn avg<I: Iterator<Item=f64>>(it: I) -> f64 { let (s,n)=it.fold((0.0,0usize), |(s,n), x| (s+x,n+1)); if n==0 {0.0} else {s/n as f64} }
fn entropy<I: Iterator<Item=usize>>(it: I) -> f64 { let xs: Vec<_> = it.collect(); let sum: usize = xs.iter().sum(); if sum==0 {return 0.0}; xs.into_iter().map(|c| { let p=c as f64/sum as f64; -p*p.log2() }).sum() }

pub fn print_log(world: &World) {
    let m = collect(world);
    println!("tick={} living={} corpses={} living_energy={} corpse_energy={} diversity={} avg_archive={:.1} avg_pheno={:.1} avg_degree={:.2} births={} deaths={} calls={} attacks={} scavenges={} hgt={} largest_lineage={:.3} kind_entropy={:.3} tag_entropy={:.3}", world.tick, m.living, m.corpses, m.total_living_energy, m.total_corpse_energy, m.lineage_diversity, m.avg_archive, m.avg_pheno, m.avg_degree, world.counters.births, world.counters.deaths, world.counters.calls, world.counters.attacks, world.counters.scavenges, world.counters.hgt, m.largest_lineage_share, m.module_kind_entropy, m.module_tag_entropy);
    let mut kinds: Vec<_> = m.kind_counts.iter().map(|(k,v)| (kind_name(*k), *v)).collect();
    kinds.sort_by_key(|x| x.0);
    println!("module_kinds={:?}", kinds);
}
