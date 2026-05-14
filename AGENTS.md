You are an expert Rust systems programmer and artificial life researcher.

Build a prototype of an artificial life simulator called "Evolved Biosphere".

Use Rust 2021 edition.

The simulator should model a digital biosphere based on the principle:

"Content-addressed interaction under resource pressure."

This is not a physics simulation. There is no 2D or 3D grid, no gravity, no fluid dynamics, no bodies in Euclidean space. The world is a graph of digital organisms. Organisms interact locally through graph edges. They call each other's modules using fuzzy content-addressed tags. They compete, cooperate, parasitize, scavenge dead code, copy useful modules, mutate archives, and reproduce under strict resource accounting.

The goal is not to create intelligence. The goal is to create an evolvable digital ecology where novelty can arise through:
1. coevolution,
2. fuzzy addressing,
3. graph locality,
4. digital necromass,
5. horizontal gene transfer,
6. genotype/phenotype separation,
7. API-call symbiosis,
8. resource pressure.

Produce a working Rust CLI program.

The program must compile and run with:

cargo run --release

Use deterministic randomness with a seed so experiments can be repeated.

Use only reasonable crates:
- rand
- rand_chacha
- serde
- serde_json
- clap
- petgraph or your own simple graph implementation
- csv optional

Avoid complex dependencies. Prefer clear code.

============================================================
HIGH-LEVEL WORLD MODEL
============================================================

The world contains:

1. Organisms
2. Corpses / necromass
3. A graph topology
4. Resource accounting
5. Fuzzy content-addressed calls
6. Archives and decoders
7. Mutation and reproduction
8. Logging and metrics

There is no global well-mixed soup. Every organism has a limited number of neighbors. Organisms can only interact with graph neighbors, local corpses, or modules they already carry.

============================================================
CORE DATA TYPES
============================================================

Implement the following conceptual structures.

1. Tag

A Tag is a short bitstring or fixed-size byte array used for fuzzy addressing.

Example:
- [u8; 8], representing 64 bits.

Implement:

fn tag_distance(a: &Tag, b: &Tag) -> u32

Use Hamming distance.

Implement:

fn mutate_tag(tag: &mut Tag, mutation_rate: f64, rng: &mut Rng)

Each bit has some chance of flipping.

2. Module

A Module is an inert code-like object. It has:

- id
- tag
- kind
- cost
- payload

The module does not need to run arbitrary code. For safety and simplicity, define a small enum of possible module behaviors.

Example module kinds:

enum ModuleKind {
    Harvest,
    Attack,
    Defend,
    Copy,
    Decode,
    Trade,
    Repair,
    MoveEdge,
    Auth,
    Scavenge,
    Reproduce,
    Noop,
}

A module's behavior is interpreted by the simulator.

This is important: do not execute untrusted code. The organisms are not real native code. They are interpreted symbolic structures.

3. Archive

The Archive is the genotype.

It is cheap to store. It contains dormant modules and tags.

Fields:

- dormant_modules: Vec<Module>
- decoder_bias_tags: Vec<Tag>
- mutation_rate
- compression_level or storage_cost_modifier

Most archive modules are not active. They are cheap to carry.

4. Phenotype

The Phenotype is the active machine.

Fields:

- active_modules: Vec<Module>
- membrane_tokens: Vec<Tag>
- call_preferences: Vec<Tag>
- max_calls_per_tick
- decoder_strength
- defense_level

Active modules cost energy each tick. Dormant archive modules cost much less.

5. Organism

Fields:

- id
- energy: i64
- age: u64
- archive: Archive
- phenotype: Phenotype
- alive: bool
- lineage_id
- parent_id optional
- symbiont_links: Vec<OrganismId>
- local_memory: Vec<Tag>

6. Corpse

When an organism dies, it must not vanish.

It becomes necromass.

Fields:

- id
- former_organism_id
- modules: Vec<Module>
- energy_value
- decay_timer
- hardness
- tags

Corpses are inert. They cannot act, but living organisms can:
- scavenge them for energy,
- copy modules from them,
- use them as barriers or filters,
- mutate around them.

7. World Graph

Use graph locality.

Nodes may contain:
- alive organism,
- corpse,
- empty site,
- colony structure later.

Edges represent possible interaction.

Organisms may only call/interact with neighboring nodes.

Implement a maximum degree per node, e.g. 8.

Implement edge rewiring:
- organisms with MoveEdge modules can drop one edge and form another local/random edge, paying energy.

============================================================
SIMULATION LOOP
============================================================

Each tick:

1. Increment world tick.
2. Shuffle living organisms deterministically.
3. For each living organism:
   a. pay maintenance cost:
      - base metabolism
      - active module cost
      - small archive storage cost
   b. if energy <= 0, die and become corpse.
   c. decode archive:
      - decoder chooses some dormant modules to activate based on fuzzy matching between decoder bias tags and module tags.
      - active phenotype can change over time.
   d. choose an action:
      - call neighbor module
      - harvest
      - attack
      - defend
      - trade
      - scavenge corpse
      - copy module from corpse or neighbor
      - rewire graph edge
      - reproduce
      - no-op
   e. resolve action with resource accounting.
   f. apply possible mutations.
4. Decay corpses.
5. Remove fully decayed corpses or convert them into inert environmental structure.
6. Log metrics.

============================================================
FUZZY CONTENT-ADDRESSED CALLS
============================================================

When organism A wants to call a function, it does not call exact module ID.

Instead it creates a desired Tag.

Among its own active modules and its neighbors' exposed active modules, find the nearest tag by Hamming distance.

If the distance is below a threshold, execute that module behavior.

If there are multiple near matches, choose weighted by closeness.

Important:
- calling a neighbor costs energy/bandwidth.
- exposing a module may earn energy.
- malicious modules may extract energy or copy themselves.
- approximate calls are allowed to hit unintended modules.

This is the core "digital chemistry".

Implement:

fn fuzzy_select_module(desired_tag, candidates, max_distance, rng) -> Option<ModuleRef>

Candidate modules come from:
- self phenotype,
- neighbor phenotypes,
- optionally corpse modules if scanning/scavenging.

============================================================
RESOURCE ACCOUNTING
============================================================

Everything costs something.

Implement global constants:

- BASE_METABOLISM_COST
- ACTIVE_MODULE_COST
- ARCHIVE_MODULE_STORAGE_COST
- CALL_COST
- EDGE_REWIRE_COST
- COPY_MODULE_COST
- REPRODUCTION_COST
- MUTATION_COST optional
- SCAVENGE_GAIN
- ATTACK_DAMAGE
- DEFENSE_REDUCTION

Organisms die at energy <= 0.

No operation should be free if it creates, copies, calls, stores, or moves information.

The simulator must conserve/track resource flow enough to avoid infinite free replication.

============================================================
ACTIONS
============================================================

Implement at least these actions.

1. Harvest

A basic action that gives a small amount of energy.

But avoid making it a fixed external objective. Harvest should be weak and only enough to sustain simple organisms. Richer energy should come from interaction.

2. Attack

Attack neighboring organism:
- costs attacker energy,
- removes victim energy,
- can be reduced by defense,
- if victim dies, corpse remains.

3. Defend

Increase temporary defense or strengthen membrane.

4. Trade / Service Call

One organism calls another's module.
The callee may receive energy.
The caller receives effect.

5. Scavenge

Eat local corpse:
- gain energy from corpse,
- maybe copy one module from corpse archive into own archive.

6. Copy / Horizontal Gene Transfer

Copy a module from:
- neighbor,
- corpse,
- symbiont.

Cost energy. Copied module goes into Archive, not immediately into Phenotype.

7. Decode

Move or express dormant archive modules into active phenotype.

The decoder uses fuzzy matching.

8. Rewire

Change graph connections.

This creates cheap geography without coordinates.

9. Reproduce

If energy exceeds a threshold:
- create child organism on nearby empty node or by splitting edge.
- copy archive with mutation.
- phenotype is regenerated by decoder.
- split energy with child.
- child keeps lineage information.

10. Symbiosis / API Link

Implement a simple early version:

If organism A repeatedly benefits from calling organism B, store B's id/tag in symbiont_links.

During reproduction, A may attempt to place child near B or copy a module from B.

Optional advanced version:
- two organisms can form a reproductive bundle if mutual calls produce net positive energy over time.

============================================================
DIGITAL NECROMASS
============================================================

Death must produce persistent corpse structures.

When organism dies:
- convert it to Corpse.
- preserve some archive modules and active modules.
- corpse has energy_value.
- corpse decays slowly.

Living organisms can:
- scavenge energy,
- copy modules,
- use corpse tags as environmental signals.

Implement corpse decay.

Optional:
- hardened corpses decay slower and can reduce attack/call access across edges.

============================================================
GENOTYPE / PHENOTYPE SPLIT
============================================================

This is crucial.

The archive is cheap dormant code.

The phenotype is expensive active code.

At birth:
- child receives mutated archive.
- phenotype starts with decoder-selected active modules.

Each tick:
- organism may re-decode some archive modules into phenotype.
- active modules have high metabolic cost.
- archive modules have tiny storage cost.

This allows cryptic variation.

============================================================
MUTATION
============================================================

Mutation operations:

- bit flip in tag
- module kind mutation
- duplicate module
- delete module
- modify cost
- alter decoder bias tag
- alter mutation rate
- alter membrane token
- recombine/copy module from corpse or neighbor

Mutation should usually not create invalid organisms.

All genomes should remain executable.

If a module mutates into nonsense, represent it as Noop, not as a crash.

============================================================
APEX PREDATOR TEST
============================================================

The system should make it possible for a "semantic anglerfish" predator to evolve.

This predator type would:
- expose many high-demand fuzzy tags,
- attract calls,
- charge/tax callers,
- insert copied modules into callers,
- occupy graph bridge positions,
- farm corpses.

Do not hardcode this predator as a special organism.

But implement enough mechanics that such a strategy could arise from module combinations.

For debugging, optionally include a hand-seeded "anglerfish" organism in one experiment scenario.

============================================================
INITIALIZATION
============================================================

CLI options:

--seed <u64>
--organisms <usize>
--ticks <u64>
--nodes <usize>
--max-degree <usize>
--log-every <u64>
--scenario <random|parasite_test|necromass_test|symbiosis_test>

Initial random organisms:
- each has small archive with random modules.
- each has decoder bias tags.
- each has starting energy.
- graph is random but bounded degree.

Initial module distribution:
- Harvest common
- Noop common
- Attack uncommon
- Defend uncommon
- Copy uncommon
- Scavenge uncommon
- Decode common
- Reproduce uncommon
- Trade uncommon
- MoveEdge uncommon
- Auth rare

============================================================
METRICS
============================================================

Every N ticks, log:

- tick
- living organism count
- corpse count
- total living energy
- total corpse energy
- average archive size
- average phenotype size
- average graph degree
- lineage diversity
- module kind counts
- number of calls
- number of attacks
- number of scavenges
- number of births
- number of deaths
- number of HGT/copy events
- largest lineage share
- entropy of module tags
- entropy of module kinds

Print to stdout and optionally write CSV/JSON.

The goal is to see whether the world:
- collapses,
- reaches monoculture,
- maintains diversity,
- produces arms races,
- produces stable symbioses,
- evolves parasitic service hubs.

============================================================
PROJECT STRUCTURE
============================================================

Create a clean Rust project:

src/
  main.rs
  config.rs
  tag.rs
  module.rs
  organism.rs
  corpse.rs
  world.rs
  actions.rs
  mutation.rs
  metrics.rs
  scenarios.rs

Cargo.toml

Use idiomatic Rust.

Add comments explaining major concepts.

Include unit tests for:
- Hamming distance
- tag mutation
- fuzzy module selection
- death creates corpse
- reproduction mutates archive
- resource costs are applied
- graph locality prevents non-neighbor calls

============================================================
OUTPUT
============================================================

When run, the program should print something like:

tick=0 living=500 corpses=0 diversity=500 avg_archive=12.4 avg_pheno=3.1 births=0 deaths=0
tick=100 living=472 corpses=38 diversity=461 avg_archive=13.8 avg_pheno=3.4 births=22 deaths=50
...

At the end, print a summary:

Simulation complete.
Final living organisms:
Final corpses:
Total births:
Total deaths:
Total calls:
Total HGT events:
Largest lineage share:
Module entropy:
Possible collapse warning if one lineage exceeds 80%.

============================================================
IMPORTANT DESIGN RULES
============================================================

1. No arbitrary code execution.
Organisms are symbolic/interpreted modules.

2. No global access.
Organisms interact only through graph locality.

3. No free copying.
Every copy costs energy.

4. No vanishing dead.
Death creates necromass.

5. No exact-only addressing.
Use fuzzy Hamming-distance matching.

6. No brittle invalid genomes.
All mutations must produce valid structures.

7. No fixed external fitness function.
Fitness emerges from survival, reproduction, and interaction.

8. Keep the MVP simple but extensible.

============================================================
DELIVERABLES
============================================================

Please output:

1. Cargo.toml
2. All Rust source files
3. Instructions to run
4. Explanation of the architecture
5. Example commands:
   cargo run --release -- --seed 42 --organisms 500 --nodes 1000 --ticks 10000 --log-every 100
6. Suggested next improvements.

Start by implementing the simplest complete version that compiles and runs.

Do not leave TODOs in core logic.

If you simplify something, explain the simplification.

Do not over-engineer. Do not build a neural network. Do not build a full VM. Do not execute generated code. Build an interpreted ecology of symbolic modules first.

The first version must be crude but alive.

The miracle is not in fancy code.  
The miracle is in the loop:

local interaction
energy cost
fuzzy call
mutation
death leaves structure
copying from the dead
reproduction
repeat

That is the seed crystal.
