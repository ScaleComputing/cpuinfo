//! Provide funcationality to parse and display different cpuid leaf types

use super::facts::GenericFact;
use super::{bitfield, cpuid, is_empty_leaf};
use core::arch::x86_64::CpuidResult;
use enum_dispatch::enum_dispatch;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::string;
use std::vec::Vec;

#[enum_dispatch]
pub trait DisplayLeaf {
    fn scan_sub_leaves(&self, leaf: u32) -> Vec<CpuidResult>;
    fn display_leaf(
        &self,
        leaf: &[CpuidResult],
        f: &mut fmt::Formatter<'_>,
    ) -> Result<(), fmt::Error>;
}

///The first leaf found requires special processing
#[derive(Debug, Serialize, Deserialize)]
pub struct StartLeaf {}

impl StartLeaf {
    fn get_text(&self, leaf: &CpuidResult) -> String {
        let CpuidResult {
            eax: _,
            ebx,
            ecx,
            edx,
        } = leaf;

        let bytes = [ebx, edx, ecx]
            .iter()
            .flat_map(|val| val.to_le_bytes())
            .collect::<Vec<u8>>();
        ToString::to_string(&string::String::from_utf8_lossy(&bytes))
    }

    pub fn get_facts<T>(&self, leaves: &[CpuidResult]) -> Vec<GenericFact<T>>
    where
        T: From<u32> + From<String>,
    {
        let CpuidResult {
            eax: max_leaf,
            ebx: _,
            ecx: _,
            edx: _,
        } = leaves[0];
        let text = self.get_text(&leaves[0]);

        vec![
            GenericFact::new("max_leaves".into(), max_leaf.into()),
            GenericFact::new("type".into(), text.into()),
        ]
    }
}

impl DisplayLeaf for StartLeaf {
    fn scan_sub_leaves(&self, leaf: u32) -> Vec<CpuidResult> {
        vec![cpuid(leaf, 0)]
    }
    fn display_leaf(
        &self,
        leaf: &[CpuidResult],
        f: &mut fmt::Formatter<'_>,
    ) -> Result<(), fmt::Error> {
        let CpuidResult {
            eax: max_leaf,
            ebx: _,
            ecx: _,
            edx: _,
        } = leaf[0];

        let text = self.get_text(&leaf[0]);

        write!(f, "'{}' max leaf:{}", text, max_leaf)
    }
}

/// A leaf that contains a string encoded in 32-bit registers
#[derive(Debug, Serialize, Deserialize)]
pub struct StringLeaf {}

impl StringLeaf {
    pub fn get_text(&self, leaf: &CpuidResult) -> String {
        let CpuidResult { eax, ebx, ecx, edx } = leaf;
        let registers = [eax, ebx, ecx, edx];
        let text = registers
            .iter()
            .flat_map(|val| val.to_le_bytes())
            .collect::<Vec<u8>>();

        ToString::to_string(&String::from_utf8_lossy(&text))
    }
    pub fn get_facts<T>(&self, leaves: &[CpuidResult]) -> Vec<GenericFact<T>>
    where
        T: From<String>,
    {
        let text = self.get_text(&leaves[0]);
        vec![GenericFact::new("value".into(), text.into())]
    }
}

impl DisplayLeaf for StringLeaf {
    fn scan_sub_leaves(&self, leaf: u32) -> Vec<CpuidResult> {
        let cpuid = cpuid(leaf, 0);
        if !is_empty_leaf(&cpuid) {
            vec![cpuid]
        } else {
            vec![]
        }
    }
    fn display_leaf(
        &self,
        leaf: &[CpuidResult],
        f: &mut fmt::Formatter<'_>,
    ) -> Result<(), fmt::Error> {
        let text = self.get_text(&leaf[0]);
        write!(f, "'{}'", text)
    }
}

/// A leaf that contains a mix of non 32-bit integers and bit sized flags
#[derive(Debug, Serialize, Deserialize)]
pub struct BitFieldLeaf {
    eax: Vec<bitfield::Field>,
    ebx: Vec<bitfield::Field>,
    ecx: Vec<bitfield::Field>,
    edx: Vec<bitfield::Field>,
}

impl BitFieldLeaf {
    fn single_reg(
        name: &str,
        reg: u128,
        fields: &Vec<bitfield::Field>,
        f: &mut fmt::Formatter<'_>,
    ) -> Result<(), fmt::Error> {
        writeln!(f, " {}: {:#8x}", name, reg)?;
        for field in fields {
            writeln!(
                f,
                "  {}",
                bitfield::BoundField::from_register_and_field(reg, field)
            )?
        }
        Ok(())
    }
}

impl DisplayLeaf for BitFieldLeaf {
    fn scan_sub_leaves(&self, leaf: u32) -> Vec<CpuidResult> {
        let cpuid = cpuid(leaf, 0);
        if !is_empty_leaf(&cpuid) {
            vec![cpuid]
        } else {
            vec![]
        }
    }
    fn display_leaf(
        &self,
        leaf: &[CpuidResult],
        f: &mut fmt::Formatter<'_>,
    ) -> Result<(), fmt::Error> {
        let CpuidResult { eax, ebx, ecx, edx } = leaf[0];
        writeln!(f)?;

        Self::single_reg("eax", eax.into(), &self.eax, f)?;
        Self::single_reg("ebx", ebx.into(), &self.ebx, f)?;
        Self::single_reg("ecx", ecx.into(), &self.ecx, f)?;
        Self::single_reg("edx", edx.into(), &self.edx, f)?;
        Ok(())
    }
}

/// Enum to aid in serializing and deserializing leaf information
#[enum_dispatch(DisplayLeaf)]
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum LeafType {
    Start(StartLeaf),
    String(StringLeaf),
    BitField(BitFieldLeaf),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LeafDesc {
    name: String,
    data_type: LeafType,
}

impl LeafDesc {
    pub fn new(name: String, data_type: LeafType) -> LeafDesc {
        LeafDesc { name, data_type }
    }

    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn data_type(&self) -> &LeafType {
        &self.data_type
    }

    pub fn bind_leaf(&self, leaf: u32) -> Option<BoundLeaf> {
        let sub_leaves = self.scan_sub_leaves(leaf);
        if !sub_leaves.is_empty() {
            Some(BoundLeaf {
                desc: self,
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
    fn display_leaf(
        &self,
        leaf: &[CpuidResult],
        f: &mut fmt::Formatter<'_>,
    ) -> Result<(), fmt::Error> {
        write!(f, "{}: ", self.name)?;
        self.data_type.display_leaf(leaf, f)
    }
}
