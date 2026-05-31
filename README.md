# Evolvex Biosphere

A prototype Rust CLI artificial-life simulator built on a single principle:

> **Content-addressed interaction under resource pressure.**

This is *not* a physics simulation — there is no grid, no space, no bodies. The
world is a bounded-degree **graph** of digital organisms that interact only with
their graph neighbours. They call each other's modules by **fuzzy** (Hamming
distance) content addresses, compete and cooperate, parasitize and infect each
other, scavenge the dead, copy useful genes horizontally, mutate their archives,
and reproduce under strict, bounded energy accounting. Fitness is never an
external objective; it emerges from survival, reproduction and interaction.

## Run

```bash
cargo run --release -- --seed 42 --organisms 500 --nodes 1000 --ticks 10000 --log-every 100
```

Small smoke run:

```bash
cargo run -- --seed 7 --organisms 20 --nodes 50 --ticks 200 --log-every 20
```

Scenarios and CSV output:

```bash
cargo run --release -- --scenario parasite_test --seed 42 --csv run.csv
cargo run --release -- --scenario symbiosis_test
cargo run --release -- --scenario necromass_test
```

Run the tests:

```bash
cargo test
```

### CLI options

| Flag | Default | Meaning |
|------|---------|---------|
| `--seed <u64>` | 42 | Deterministic RNG seed (runs are reproducible) |
| `--organisms <usize>` | 500 | Initial organisms (must be `<= --nodes`) |
| `--ticks <u64>` | 10000 | Simulation length |
| `--nodes <usize>` | 1000 | Graph nodes |
| `--max-degree <usize>` | 8 | Maximum edges per node |
| `--log-every <u64>` | 100 | Metrics print interval |
| `--scenario <name>` | random | `random` \| `parasite_test` \| `necromass_test` \| `symbiosis_test` |
| `--csv <path>` | — | Optional: write per-interval metrics as CSV |

## Architecture

```
src/
  main.rs        CLI driver, tick loop, summary
  config.rs      CLI + all resource-accounting constants
  tag.rs         64-bit content-address tags: Hamming distance, mutation, jitter
  module.rs      ModuleKind, Module (functional payload), fuzzy_select_module
  organism.rs    Archive (genotype) / Phenotype (phenotype), decode + exposure
  corpse.rs      Necromass (hardness-aware scavenging)
  world.rs       Graph + node resources + simulation loop + mechanics
  actions.rs     Decision layer + the fuzzy-call dispatch (provider/caller roles)
  mutation.rs    Germline + somatic mutation (always valid genomes)
  metrics.rs     Counters, metrics, entropy, CSV
  scenarios.rs   Seeding incl. the hand-built "anglerfish" predator
```

### How it works

* **Genotype / phenotype split.** The `Archive` holds cheap, mostly-dormant
  modules (the genotype). A *decoder* expresses a subset into the expensive
  active `Phenotype` by fuzzy-matching dormant module tags against evolvable
  decoder-bias tags. Active modules cost energy every tick; dormant ones cost
  almost nothing. This allows cryptic variation to ride along until expressed.

* **Bounded energy.** The *only* primary production is the slow regeneration of
  per-node environmental `resource`. Organisms feed weakly (passive absorption +
  harvest) from their node; everything else merely *moves* energy. Because total
  influx is capped by `nodes × regen`, there is no infinite free replication —
  the population self-regulates around a carrying capacity well below node count.

* **Fuzzy content-addressed calls** are the core "digital chemistry". An
  organism emits a desired tag and the nearest *exposed* module within a Hamming
  threshold answers. Crucially, a call has distinct **caller** and **provider**
  roles: the caller pays bandwidth plus a price the provider sets via the
  module's `payload` (a tax). Service kinds (Repair, Trade, Defend) benefit the
  caller and earn the provider its fee — **mutualism**. The parasitic kinds flip:
  a called **Copy** lets the provider *inject* a gene into the caller
  (contagious HGT), and a called **Attack** lets the provider *bite* the caller.
  Approximate calls can hit unintended modules, which is the engine of novelty.

* **Membranes** gate the public API: only active modules near a membrane token
  are exposed to neighbours' calls. An "anglerfish" exposes many lure tags and
  answers with steep-priced Attack/Copy modules — it is *not* a special type,
  just an archive that encodes that strategy (see `scenarios::seed_anglerfish`).

* **Digital necromass.** Death never deletes; it leaves a persistent `Corpse`
  with energy and modules. The living scavenge it for energy and copy genes from
  the dead. Harder corpses (high defenders) decay and yield more slowly.

* **Mutation** is germline (at reproduction) and somatic (a small per-tick
  chance). Every mutation keeps the genome valid — nonsense collapses to `Noop`,
  never a crash, and archive size stays bounded.

### What you should see

A typical 10k-tick run from 500 organisms settles into a living, turning-over
ecology of ~900 organisms with continuous predation arms races, ongoing HGT and
infection, and *rising* module/tag diversity even as lineages competitively
consolidate. Different seeds and scenarios produce qualitatively different
outcomes — high-diversity coexistence, predator booms, or drift toward
monoculture (the summary prints a collapse warning past 80% single-lineage
share). The miracle is the loop, not the code: local interaction → energy cost →
fuzzy call → mutation → death leaves structure → copying from the dead →
reproduction → repeat.

## next improvements:

* Explicit per-interval (delta) counters alongside cumulative totals.
* Service "contracts": let providers advertise and renegotiate prices, enabling
  richer market/parasite dynamics and reputation via `symbiont_links`.
* Spatial heterogeneity in node regeneration to create biomes and migration.
* Lineage phylogeny export and module-flow (HGT) network export for analysis.
* Property tests asserting global energy is conserved up to dissipation.
* Parameter sweeps over seeds/constants with summary statistics.
* Reproductive bundles: two organisms with net-positive mutual calls co-reproduce.
