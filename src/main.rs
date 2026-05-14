use clap::Parser;
use evolvex::{config::Config, metrics::{collect, print_log}, scenarios, world::World};

fn main() {
    let cfg = Config::parse();
    if cfg.organisms > cfg.nodes { eprintln!("--organisms must be <= --nodes"); std::process::exit(2); }
    let mut world = World::new(cfg.seed, cfg.nodes, cfg.max_degree.max(1));
    scenarios::initialize(&mut world, &cfg.scenario, cfg.organisms);
    print_log(&world);
    for _ in 0..cfg.ticks { world.step(); if cfg.log_every > 0 && world.tick % cfg.log_every == 0 { print_log(&world); } }
    let m = collect(&world);
    println!("Simulation complete.");
    println!("Final living organisms: {}", m.living);
    println!("Final corpses: {}", m.corpses);
    println!("Total births: {}", world.counters.births);
    println!("Total deaths: {}", world.counters.deaths);
    println!("Total calls: {}", world.counters.calls);
    println!("Total HGT events: {}", world.counters.hgt);
    println!("Largest lineage share: {:.3}", m.largest_lineage_share);
    println!("Module entropy: kind={:.3} tag={:.3}", m.module_kind_entropy, m.module_tag_entropy);
    if m.largest_lineage_share > 0.8 && m.living > 0 { println!("Collapse warning: one lineage exceeds 80% of living organisms."); }
}
