use core::arch::x86_64::CpuidResult;
use std::fmt;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::vec::Vec;
use super::{cpuid, is_empty_leaf};

pub trait DisplayLeaf {
    fn scan_sub_leaves(&self, leaf: u32) -> Vec<CpuidResult>;
    fn display_leaf(&self, leaf: &[CpuidResult], f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error>;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StartLeaf {}

impl DisplayLeaf for StartLeaf {
    fn scan_sub_leaves(&self, leaf: u32) -> Vec<CpuidResult> {
        vec![cpuid(leaf,0)]
    }
    fn display_leaf(&self, leaf: &[CpuidResult], f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error>
    {
        let CpuidResult{eax:max_leaf, ebx, ecx, edx} = leaf[0];

        let text = String::from_utf8([ebx, edx, ecx].iter().map(|val| val.to_le_bytes()).flatten().collect()).unwrap();

        write!(f, "'{}' max leaf:{}",text, max_leaf)
    }

}

#[derive(Debug, Serialize, Deserialize)]
pub struct StringLeaf {}

impl DisplayLeaf for StringLeaf {
    fn scan_sub_leaves(&self, leaf: u32) -> Vec<CpuidResult> {
        let cpuid = cpuid(leaf,0);
        if ! is_empty_leaf(&cpuid) {
            vec![cpuid]
        } else {
            vec![]
        }
    }
    fn display_leaf(&self, leaf: &[CpuidResult], f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error>
    {
        let CpuidResult{eax, ebx, ecx, edx} = leaf[0];

        let text = String::from_utf8([eax, ebx, ecx, edx].iter().map(|val| val.to_le_bytes()).flatten().collect()).unwrap();

        write!(f, "'{}'",text)
    }

}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum LeafType {
    Start(StartLeaf),
    String(StringLeaf),
}

impl DisplayLeaf for LeafType {
    fn scan_sub_leaves(&self, leaf: u32) -> Vec<CpuidResult> {
        match self {
            LeafType::Start(desc) => desc.scan_sub_leaves(leaf),
            LeafType::String(desc) => desc.scan_sub_leaves(leaf),
        }
    }
    fn display_leaf(&self, leaf: &[CpuidResult], f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        match self {
            LeafType::Start(desc) => desc.display_leaf(leaf, f),
            LeafType::String(desc) => desc.display_leaf(leaf, f),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LeafDesc {
    name: String,
    data_type: LeafType,
}

impl LeafDesc {
    pub fn new(name: String, data_type: LeafType) -> LeafDesc {
        LeafDesc{name, data_type}
    }

    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn data_type(&self) -> &LeafType {
        &self.data_type
    }

    pub fn bind_leaf(&self, leaf: u32) -> Option<BoundLeaf> {
        let sub_leaves = self.scan_sub_leaves(leaf);
        if sub_leaves.len() > 0 {
            Some(BoundLeaf{ desc: self,
            sub_leaves,
            })
        } else {
            None
        }
    }
}


pub struct BoundLeaf<'a> {
    pub desc: &'a LeafDesc,
    pub sub_leaves: Vec<CpuidResult>,
}

impl<'a> fmt::Display for BoundLeaf<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        self.desc.display_leaf(&self.sub_leaves, f)
    }
}

impl DisplayLeaf for LeafDesc {
    fn scan_sub_leaves(&self, leaf: u32) -> Vec<CpuidResult> {
        self.data_type.scan_sub_leaves(leaf)
    }
    fn display_leaf(&self, leaf: &[CpuidResult], f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "{}: ", self.name)?;
        self.data_type.display_leaf(leaf, f)
    }
}

