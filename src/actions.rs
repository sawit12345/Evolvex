use crate::module::{ModuleKind, ModuleRef};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action { Execute(ModuleRef), Harvest, Noop }

pub fn kind_name(k: ModuleKind) -> &'static str {
    match k { ModuleKind::Harvest=>"Harvest", ModuleKind::Attack=>"Attack", ModuleKind::Defend=>"Defend", ModuleKind::Copy=>"Copy", ModuleKind::Decode=>"Decode", ModuleKind::Trade=>"Trade", ModuleKind::Repair=>"Repair", ModuleKind::MoveEdge=>"MoveEdge", ModuleKind::Auth=>"Auth", ModuleKind::Scavenge=>"Scavenge", ModuleKind::Reproduce=>"Reproduce", ModuleKind::Noop=>"Noop" }
}
