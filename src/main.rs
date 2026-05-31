//! CLI entry point. Wires up a world from the config, runs the tick loop,
//! prints periodic metrics, optionally writes CSV, and prints a final summary.

use std::fs::File;
use std::io::{BufWriter, Write};

use clap::Parser;

use evolvex::config::Config;
use evolvex::metrics::{collect, csv_header, csv_row, print_log};
use evolvex::scenarios;
use evolvex::world::World;

fn main() {
    let cfg = Config::parse();
    if cfg.organisms > cfg.nodes {
        eprintln!("--organisms ({}) must be <= --nodes ({})", cfg.organisms, cfg.nodes);
        std::process::exit(2);
    }

    let mut world = World::new(cfg.seed, cfg.nodes, cfg.max_degree.max(1));
    scenarios::initialize(&mut world, &cfg.scenario, cfg.organisms);

    // Optional CSV sink.
    let mut csv = cfg.csv.as_ref().map(|path| {
        let f = File::create(path).unwrap_or_else(|e| {
            eprintln!("cannot create CSV file {path}: {e}");
            std::process::exit(2);
        });
        let mut w = BufWriter::new(f);
        writeln!(w, "{}", csv_header()).ok();
        w
    });

    print_log(&world);
    if let Some(w) = csv.as_mut() {
        writeln!(w, "{}", csv_row(&world)).ok();
    }

    for _ in 0..cfg.ticks {
        world.step();
        if cfg.log_every > 0 && world.tick % cfg.log_every == 0 {
            print_log(&world);
            if let Some(w) = csv.as_mut() {
                writeln!(w, "{}", csv_row(&world)).ok();
            }
        }
        // An empty world cannot recover; stop early.
        if world.organisms.iter().all(|o| !o.alive) {
            println!("All organisms extinct at tick {}.", world.tick);
            break;
        }
    }

    if let Some(mut w) = csv {
        w.flush().ok();
    }

    let m = collect(&world);
    println!("\nSimulation complete.");
    println!("Final living organisms: {}", m.living);
    println!("Final corpses: {}", m.corpses);
    println!("Total births: {}", world.counters.births);
    println!("Total deaths: {}", world.counters.deaths);
    println!("Total calls: {}", world.counters.calls);
    println!("Total HGT events: {}", world.counters.hgt);
    println!("Total infections: {}", world.counters.infections);
    println!("Largest lineage share: {:.3}", m.largest_lineage_share);
    println!(
        "Module entropy: kind={:.3} tag={:.3}",
        m.module_kind_entropy, m.module_tag_entropy
    );
    if m.living == 0 {
        println!("Collapse: the biosphere went extinct.");
    } else if m.largest_lineage_share > 0.8 {
        println!(
            "Collapse warning: one lineage holds {:.1}% of living organisms (monoculture).",
            m.largest_lineage_share * 100.0
        );
    }
}
