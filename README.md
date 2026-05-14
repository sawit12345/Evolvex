# Evolvex Biosphere

Prototype Rust CLI artificial life simulator based on content-addressed interaction under resource pressure.

Run:

```bash
cargo run --release -- --seed 42 --organisms 500 --nodes 1000 --ticks 10000 --log-every 100
```

Small smoke run:

```bash
cargo run -- --seed 7 --organisms 20 --nodes 50 --ticks 100 --log-every 10
```

Architecture: organisms occupy nodes in a bounded-degree graph, carry cheap dormant archives and costly active phenotypes, make fuzzy Hamming-distance calls to local modules, pay energy costs for maintenance/calls/copying/rewiring/reproduction, leave persistent corpses on death, and can scavenge or copy modules from local necromass.

Simplifications in this first version: module behavior is a small interpreted enum, graph topology is a simple adjacency list, symbiosis is tracked as beneficial trade links, and metrics are printed to stdout rather than CSV/JSON.

Suggested next improvements: richer call-benefit accounting, explicit service contracts, stronger parasite/anglerfish scenario instrumentation, CSV output, property tests for resource flow, and parameter sweeps over seeds.
