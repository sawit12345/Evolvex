//! Digital necromass. Death never makes an organism vanish; it leaves a
//! persistent `Corpse` that can be scavenged for energy, mined for modules
//! (horizontal gene transfer from the dead), or used as an environmental
//! signal. Harder corpses yield energy more slowly and decay more slowly.

use serde::{Deserialize, Serialize};

use crate::module::Module;
use crate::tag::Tag;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Corpse {
    pub id: u64,
    pub former_organism_id: u64,
    pub modules: Vec<Module>,
    pub energy_value: i64,
    pub decay_timer: u64,
    pub hardness: i64,
    pub tags: Vec<Tag>,
    pub node: usize,
}

impl Corpse {
    /// How much energy a single scavenge bite can extract. Hard corpses are
    /// stingy: each hardness point reduces the bite (min 1).
    pub fn bite_size(&self, max_bite: i64) -> i64 {
        let reduced = (max_bite - self.hardness).max(1);
        reduced.min(self.energy_value).max(0)
    }
}
