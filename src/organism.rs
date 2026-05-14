use serde::{Deserialize, Serialize};
use crate::{module::Module, tag::Tag};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Archive { pub dormant_modules: Vec<Module>, pub decoder_bias_tags: Vec<Tag>, pub mutation_rate: f64, pub storage_cost_modifier: f64 }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Phenotype { pub active_modules: Vec<Module>, pub membrane_tokens: Vec<Tag>, pub call_preferences: Vec<Tag>, pub max_calls_per_tick: usize, pub decoder_strength: u32, pub defense_level: i64 }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Organism {
    pub id: u64, pub energy: i64, pub age: u64, pub archive: Archive, pub phenotype: Phenotype,
    pub alive: bool, pub lineage_id: u64, pub parent_id: Option<u64>, pub symbiont_links: Vec<u64>, pub local_memory: Vec<Tag>, pub node: usize,
}

impl Organism {
    pub fn defense(&self) -> i64 { self.phenotype.defense_level + self.phenotype.active_modules.iter().filter(|m| matches!(m.kind, crate::module::ModuleKind::Defend)).count() as i64 }
}
