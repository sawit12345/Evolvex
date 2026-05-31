//! Organisms and the genotype/phenotype split.
//!
//! * `Archive` is the genotype: cheap, mostly-dormant modules. It is what gets
//!   copied (with mutation) into offspring.
//! * `Phenotype` is the expensive active machine, regenerated from the archive
//!   by the *decoder* via fuzzy matching. This separation allows cryptic
//!   variation: dormant modules ride along cheaply until a decoder shift
//!   expresses them.

use serde::{Deserialize, Serialize};

use crate::config::{MAX_ACTIVE_MODULES, MEMBRANE_EXPOSE_DIST};
use crate::module::{Module, ModuleKind};
use crate::tag::{tag_distance, Tag};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Archive {
    pub dormant_modules: Vec<Module>,
    pub decoder_bias_tags: Vec<Tag>,
    pub mutation_rate: f64,
    pub storage_cost_modifier: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Phenotype {
    pub active_modules: Vec<Module>,
    /// The organism's public API surface: only active modules near a membrane
    /// token are *exposed* to neighbours' calls. Everything else is private.
    pub membrane_tokens: Vec<Tag>,
    /// Tags this organism likes to aim calls at (seeded from memory + own tags).
    pub call_preferences: Vec<Tag>,
    pub max_calls_per_tick: usize,
    /// Fuzzy threshold controlling how much of the archive is expressed.
    /// Higher strength => more (and costlier) modules expressed. Evolvable.
    pub decoder_strength: u32,
    /// Temporary defense; bleeds off each tick so defense must be maintained.
    pub defense_level: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Organism {
    pub id: u64,
    pub energy: i64,
    pub age: u64,
    pub archive: Archive,
    pub phenotype: Phenotype,
    pub alive: bool,
    pub lineage_id: u64,
    pub parent_id: Option<u64>,
    /// Neighbours whose calls have repeatedly *paid off* for this organism.
    pub symbiont_links: Vec<u64>,
    /// Short memory of call tags that recently yielded net-positive energy;
    /// biases future call targeting (cheap reinforcement, not a neural net).
    pub local_memory: Vec<Tag>,
    pub node: usize,
}

impl Organism {
    /// Effective defense = transient level + structural Defend/Auth modules.
    pub fn defense(&self) -> i64 {
        let structural = self
            .phenotype
            .active_modules
            .iter()
            .filter(|m| matches!(m.kind, ModuleKind::Defend | ModuleKind::Auth))
            .count() as i64;
        self.phenotype.defense_level + structural
    }

    /// Re-express the phenotype from the archive. Dormant modules whose tag is
    /// within `decoder_strength` of any decoder-bias tag get activated, nearest
    /// first, capped at `MAX_ACTIVE_MODULES`.
    pub fn decode(&mut self) {
        let strength = self.phenotype.decoder_strength;
        let mut scored: Vec<(u32, Module)> = self
            .archive
            .dormant_modules
            .iter()
            .map(|m| {
                let d = self
                    .archive
                    .decoder_bias_tags
                    .iter()
                    .map(|t| tag_distance(t, &m.tag))
                    .min()
                    .unwrap_or(64);
                (d, m.clone())
            })
            .filter(|(d, _)| *d <= strength)
            .collect();
        scored.sort_by_key(|(d, _)| *d);
        scored.truncate(MAX_ACTIVE_MODULES);
        let mut active: Vec<Module> = scored.into_iter().map(|(_, m)| m).collect();
        // Guarantee a minimal viable phenotype even if nothing matched.
        if active.is_empty() {
            active = self.archive.dormant_modules.iter().take(2).cloned().collect();
        }
        self.phenotype.active_modules = active;

        // Membrane = a few decoder-bias tags; defines what neighbours may call.
        self.phenotype.membrane_tokens =
            self.archive.decoder_bias_tags.iter().take(3).copied().collect();

        // Call preferences: remembered rewarding tags first, then own tags.
        let mut prefs: Vec<Tag> = self.local_memory.clone();
        prefs.extend(self.phenotype.active_modules.iter().map(|m| m.tag));
        prefs.truncate(8);
        self.phenotype.call_preferences = prefs;
    }

    /// Active modules visible to neighbours (those near a membrane token),
    /// returned as `(active_index, tag)`. This is the organism's callable API.
    pub fn exposed_modules(&self) -> Vec<(usize, Tag)> {
        self.phenotype
            .active_modules
            .iter()
            .enumerate()
            .filter(|(_, m)| {
                self.phenotype
                    .membrane_tokens
                    .iter()
                    .any(|t| tag_distance(t, &m.tag) <= MEMBRANE_EXPOSE_DIST)
            })
            .map(|(i, m)| (i, m.tag))
            .collect()
    }

    /// Record that calling `partner` paid off, reinforcing the relationship.
    pub fn reinforce(&mut self, partner_id: u64, tag: Tag) {
        if !self.symbiont_links.contains(&partner_id) {
            self.symbiont_links.push(partner_id);
            if self.symbiont_links.len() > 8 {
                self.symbiont_links.remove(0);
            }
        }
        self.local_memory.push(tag);
        if self.local_memory.len() > 6 {
            self.local_memory.remove(0);
        }
    }
}
