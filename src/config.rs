use clap::{Parser, ValueEnum};

pub const BASE_METABOLISM_COST: i64 = 1;
pub const ACTIVE_MODULE_COST: i64 = 1;
pub const ARCHIVE_MODULE_STORAGE_COST: i64 = 1;
pub const CALL_COST: i64 = 2;
pub const SERVICE_REWARD: i64 = 1;
pub const EDGE_REWIRE_COST: i64 = 5;
pub const COPY_MODULE_COST: i64 = 6;
pub const REPRODUCTION_COST: i64 = 40;
pub const REPRODUCTION_THRESHOLD: i64 = 90;
pub const HARVEST_GAIN: i64 = 8;
pub const SCAVENGE_GAIN: i64 = 12;
pub const ATTACK_COST: i64 = 3;
pub const ATTACK_DAMAGE: i64 = 8;
pub const DEFENSE_REDUCTION: i64 = 6;
pub const MAX_FUZZY_DISTANCE: u32 = 24;
pub const CORPSE_DECAY_TICKS: u64 = 80;
pub const STARTING_ENERGY: i64 = 100;

#[derive(Clone, Debug, ValueEnum)]
pub enum Scenario { Random, ParasiteTest, NecromassTest, SymbiosisTest }

#[derive(Parser, Debug, Clone)]
#[command(name = "evolvex", about = "Evolvex Biosphere: content-addressed artificial life under resource pressure")]
pub struct Config {
    #[arg(long, default_value_t = 42)] pub seed: u64,
    #[arg(long, default_value_t = 500)] pub organisms: usize,
    #[arg(long, default_value_t = 10_000)] pub ticks: u64,
    #[arg(long, default_value_t = 1000)] pub nodes: usize,
    #[arg(long, default_value_t = 8)] pub max_degree: usize,
    #[arg(long, default_value_t = 100)] pub log_every: u64,
    #[arg(long, value_enum, default_value_t = Scenario::Random)] pub scenario: Scenario,
}
