
use serde::{Serialize, Deserialize};
use std::{collections::hash_map::HashMap, vec::Vec};

#[derive(Serialize, Deserialize, Debug, Hash, Eq, PartialEq)]
pub enum CpuidRegister {
    EAX,
    EBX,
    ECX,
    EDX,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CheckFeatureBitDescription {
    name: String,
    locations: Vec<(u32, u32, CpuidRegister)>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CheckFeatureBitValues {
    name: String,
    values: HashMap<(u32, u32, CpuidRegister), u32>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum CheckType {
    FeatureBits(CheckFeatureBitDescription),
}

#[derive(Serialize, Deserialize, Debug)]
pub enum CheckValues {
    FeatureBits(CheckFeatureBitValues),
}
