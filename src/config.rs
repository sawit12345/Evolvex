//! Configuration: CLI parsing and the global resource-accounting constants.
//!
//! Energy in Evolvex is *bounded*. The only primary production is the slow
//! regeneration of per-node environmental `resource`. Everything else
//! (calls, attacks, scavenging, trade) merely *moves* energy between
//! organisms, and every operation that creates/copies/calls/stores/moves
//! information costs something. This is what prevents infinite free
//! replication and forces a genuine ecology rather than a harvest faucet.

use clap::{Parser, ValueEnum};

// ----------------------------------------------------------------------------
// Metabolism / storage
// ----------------------------------------------------------------------------
pub const BASE_METABOLISM_COST: i64 = 1;
pub const ACTIVE_MODULE_COST: i64 = 1; // active (expressed) modules are expensive
pub const ARCHIVE_STORAGE_DIVISOR: i64 = 14; // dormant archive is *cheap*: ceil(len / divisor)

// ----------------------------------------------------------------------------
// Primary production (environment -> organism). Deliberately weak.
// ----------------------------------------------------------------------------
pub const RESOURCE_MAX: i64 = 40; // cap of energy a node can hold
pub const RESOURCE_REGEN: i64 = 4; // regenerated per node per tick (global energy influx bound)
pub const PASSIVE_ABSORB: i64 = 3; // sessile feeding during maintenance
pub const HARVEST_FLAT: i64 = 1; // tiny guaranteed trickle so minimal life can persist
pub const HARVEST_DRAW_MAX: i64 = 8; // active harvest draws from local node resource
/// Below this energy an organism switches to a homeostatic "hunger" drive and
/// feeds (harvest/scavenge) instead of taking a random fuzzy action. This is an
/// internal survival reflex, not an external fitness target.
pub const HUNGER_THRESHOLD: i64 = 24;

// ----------------------------------------------------------------------------
// Interaction costs / transfers (secondary production)
// ----------------------------------------------------------------------------
pub const CALL_COST: i64 = 1; // bandwidth paid by the caller, dissipated
pub const MAX_TAX: i64 = 8; // ceiling on what a callee can charge a caller (payload-driven)
pub const SERVICE_REPAIR_HEAL: i64 = 5; // energy a Repair service transfers provider -> caller
pub const TRADE_MAX: i64 = 5; // ceiling on a Trade dividend
pub const EDGE_REWIRE_COST: i64 = 3;
pub const COPY_MODULE_COST: i64 = 4; // self-initiated horizontal gene transfer
pub const REPRODUCTION_COST: i64 = 24;
pub const REPRODUCTION_THRESHOLD: i64 = 48;
pub const MUTATION_COST: i64 = 1; // optional: paid when a germline mutation event fires

// ----------------------------------------------------------------------------
// Combat / parasitism
// ----------------------------------------------------------------------------
pub const SCAVENGE_GAIN_MAX: i64 = 10;
pub const ATTACK_COST: i64 = 2;
pub const ATTACK_DAMAGE: i64 = 8;
pub const DEFENSE_REDUCTION: i64 = 2; // damage reduced by defense * this
pub const ATTACK_STEAL_PERCENT: i64 = 55; // predator keeps this % of damage dealt
pub const INFECTION_BASE_CHANCE: f64 = 0.75; // chance an exposed Copy call injects a module
pub const INFECTION_RESIST_PER_DEFENSE: f64 = 0.12; // each defense point resists infection

// ----------------------------------------------------------------------------
// Expression / addressing
// ----------------------------------------------------------------------------
pub const MAX_FUZZY_DISTANCE: u32 = 20; // calls above this Hamming distance simply miss
pub const MEMBRANE_EXPOSE_DIST: u32 = 22; // active module exposed only if near a membrane token
pub const MAX_ACTIVE_MODULES: usize = 8;
pub const MAX_ARCHIVE_MODULES: usize = 24;
pub const DEFENSE_DECAY: i64 = 1; // temporary defense bleeds off each tick (forces arms race)

// ----------------------------------------------------------------------------
// Necromass / life cycle
// ----------------------------------------------------------------------------
pub const CORPSE_DECAY_TICKS: u64 = 60;
pub const CORPSE_BASE_ENERGY: i64 = 8;
pub const STARTING_ENERGY: i64 = 55;
pub const SOMATIC_MUTATION_CHANCE: f64 = 0.03; // per-tick (loop step 3f) mutation chance

/// Experiment scenarios. Names intentionally use underscores to match the spec
/// CLI (`--scenario parasite_test`); kebab-case aliases are also accepted.
#[derive(Clone, Debug, ValueEnum, PartialEq, Eq)]
pub enum Scenario {
    #[value(name = "random")]
    Random,
    #[value(name = "parasite_test", alias = "parasite-test")]
    ParasiteTest,
    #[value(name = "necromass_test", alias = "necromass-test")]
    NecromassTest,
    #[value(name = "symbiosis_test", alias = "symbiosis-test")]
    SymbiosisTest,
}

#[derive(Parser, Debug, Clone)]
#[command(
    name = "evolvex",
    about = "Evolvex Biosphere: content-addressed artificial life under resource pressure"
)]
pub struct Config {
    #[arg(long, default_value_t = 42)]
    pub seed: u64,
    #[arg(long, default_value_t = 500)]
    pub organisms: usize,
    #[arg(long, default_value_t = 10_000)]
    pub ticks: u64,
    #[arg(long, default_value_t = 1000)]
    pub nodes: usize,
    #[arg(long, default_value_t = 8)]
    pub max_degree: usize,
    #[arg(long, default_value_t = 100)]
    pub log_every: u64,
    #[arg(long, value_enum, default_value_t = Scenario::Random)]
    pub scenario: Scenario,
    /// Optional path to write per-log-interval metrics as CSV.
    #[arg(long)]
    pub csv: Option<String>,
}
