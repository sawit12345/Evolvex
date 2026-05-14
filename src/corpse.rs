use serde::{Deserialize, Serialize};
use crate::{module::Module, tag::Tag};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Corpse { pub id: u64, pub former_organism_id: u64, pub modules: Vec<Module>, pub energy_value: i64, pub decay_timer: u64, pub hardness: i64, pub tags: Vec<Tag>, pub node: usize }
