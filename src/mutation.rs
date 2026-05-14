use rand::Rng;
use crate::{module::{random_kind, random_module, ModuleKind}, organism::Archive, tag::mutate_tag};

pub fn mutate_archive(archive: &mut Archive, next_module_id: &mut u64, rng: &mut impl Rng) {
    let rate = archive.mutation_rate.clamp(0.0, 0.25);
    for m in &mut archive.dormant_modules {
        mutate_tag(&mut m.tag, rate, rng);
        if rng.gen_bool(rate * 0.2) { m.kind = random_kind(rng); }
        if rng.gen_bool(rate * 0.2) { m.cost = (m.cost + rng.gen_range(-1..=1)).clamp(1, 8); }
        if rng.gen_bool(rate * 0.05) { m.kind = ModuleKind::Noop; }
    }
    for t in &mut archive.decoder_bias_tags { mutate_tag(t, rate, rng); }
    if rng.gen_bool(rate * 0.5) && !archive.dormant_modules.is_empty() {
        let mut dup = archive.dormant_modules[rng.gen_range(0..archive.dormant_modules.len())].clone();
        dup.id = *next_module_id; *next_module_id += 1;
        mutate_tag(&mut dup.tag, rate.max(0.01), rng);
        archive.dormant_modules.push(dup);
    }
    if rng.gen_bool(rate * 0.25) && archive.dormant_modules.len() > 2 { archive.dormant_modules.remove(rng.gen_range(0..archive.dormant_modules.len())); }
    if rng.gen_bool(rate * 0.1) { archive.dormant_modules.push(random_module(*next_module_id, rng)); *next_module_id += 1; }
    if rng.gen_bool(rate * 0.2) { archive.mutation_rate = (archive.mutation_rate + rng.gen_range(-0.01..=0.01)).clamp(0.001, 0.2); }
}
