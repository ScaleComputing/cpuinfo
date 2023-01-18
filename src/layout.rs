//! Provide funcationality to parse and display different cpuid leaf types

use super::facts::{self, GenericFact};
use super::{
    bitfield::{self, Facter},
    is_empty_leaf, CpuidDB,
};
use core::arch::x86_64::CpuidResult;
use enum_dispatch::enum_dispatch;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::string;
use std::vec::Vec;

#[enum_dispatch]
pub trait DisplayLeaf {
    fn scan_sub_leaves<CPUIDFunc: CpuidDB>(&self, leaf: u32, cpuid: &CPUIDFunc)
        -> Vec<CpuidResult>;
    fn display_leaf(
        &self,
        leaf: &[CpuidResult],
        f: &mut fmt::Formatter<'_>,
    ) -> Result<(), fmt::Error>;
    fn get_facts<T: From<String> + From<u32> + From<bool>>(
        &self,
        leaves: &[CpuidResult],
    ) -> Vec<GenericFact<T>>;
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

        let bytes = vec![*ebx, *edx, *ecx]
            .into_iter()
            .flat_map(|val| Vec::from(val.to_le_bytes()).into_iter())
            .collect::<Vec<u8>>();
        ToString::to_string(&string::String::from_utf8_lossy(&bytes))
    }
}

impl DisplayLeaf for StartLeaf {
    fn scan_sub_leaves<CPUIDFunc: CpuidDB>(
        &self,
        leaf: u32,
        cpuid: &CPUIDFunc,
    ) -> Vec<CpuidResult> {
        vec![cpuid.get_cpuid(leaf, 0)]
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

    fn get_facts<T>(&self, leaves: &[CpuidResult]) -> Vec<GenericFact<T>>
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

/// A leaf that contains a string encoded in 32-bit registers
#[derive(Debug, Serialize, Deserialize)]
pub struct StringLeaf {}

impl StringLeaf {
    pub fn get_text(&self, leaf: &CpuidResult) -> String {
        let CpuidResult { eax, ebx, ecx, edx } = leaf;
        let text = vec![*eax, *ebx, *ecx, *edx]
            .into_iter()
            .flat_map(|val| Vec::from(val.to_le_bytes()).into_iter())
            .collect::<Vec<u8>>();

        ToString::to_string(&String::from_utf8_lossy(&text))
    }
}

impl DisplayLeaf for StringLeaf {
    fn scan_sub_leaves<CPUIDFunc: CpuidDB>(
        &self,
        leaf: u32,
        cpuid: &CPUIDFunc,
    ) -> Vec<CpuidResult> {
        let cpuid = cpuid.get_cpuid(leaf, 0);
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

    fn get_facts<T>(&self, leaves: &[CpuidResult]) -> Vec<GenericFact<T>>
    where
        T: From<String>,
    {
        let text = self.get_text(&leaves[0]);
        vec![GenericFact::new("value".into(), text.into())]
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
    fn scan_sub_leaves<CPUIDFunc: CpuidDB>(
        &self,
        leaf: u32,
        cpuid: &CPUIDFunc,
    ) -> Vec<CpuidResult> {
        let cpuid = cpuid.get_cpuid(leaf, 0);
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
    fn get_facts<T>(&self, leaves: &[CpuidResult]) -> Vec<GenericFact<T>>
    where
        T: From<bool> + From<u32>,
    {
        let CpuidResult { eax, ebx, ecx, edx } = leaves[0];
        [
            ("eax", eax, &self.eax),
            ("ebx", ebx, &self.ebx),
            ("ecx", ecx, &self.ecx),
            ("edx", edx, &self.edx),
        ]
        .iter()
        .flat_map(|i| i.2.iter().map(move |j| (i.0, i.1.into(), j)))
        .map(|q| {
            let mut fact = bitfield::BoundField::from_register_and_field(q.1, q.2).collect_fact();
            fact.add_path(q.0);
            fact
        })
        .collect::<Vec<GenericFact<T>>>()
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

    pub fn bind_leaf<CPUIDFunc: CpuidDB>(&self, leaf: u32, cpuid: &CPUIDFunc) -> Option<BoundLeaf> {
        let sub_leaves = self.scan_sub_leaves(leaf, cpuid);
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

impl DisplayLeaf for LeafDesc {
    fn scan_sub_leaves<CPUIDFunc: CpuidDB>(
        &self,
        leaf: u32,
        cpuid: &CPUIDFunc,
    ) -> Vec<CpuidResult> {
        self.data_type.scan_sub_leaves(leaf, cpuid)
    }
    fn display_leaf(
        &self,
        leaf: &[CpuidResult],
        f: &mut fmt::Formatter<'_>,
    ) -> Result<(), fmt::Error> {
        write!(f, "{}: ", self.name)?;
        self.data_type.display_leaf(leaf, f)
    }
    fn get_facts<T>(&self, leaves: &[CpuidResult]) -> Vec<GenericFact<T>>
    where
        T: From<u32> + From<String> + From<bool>,
    {
        self.data_type.get_facts(leaves)
    }
}

pub struct BoundLeaf<'a> {
    pub desc: &'a LeafDesc,
    pub sub_leaves: Vec<CpuidResult>,
}

impl<'a> BoundLeaf<'a> {
    pub fn get_facts<T: From<u32> + From<bool> + From<String>>(&self) -> Vec<GenericFact<T>> {
        let mut facts = self.desc.get_facts(&self.sub_leaves);
        facts.iter_mut().for_each(|i| {
            i.add_path(&self.desc.name);
        });
        facts
    }
}

impl<'a, T: From<u32> + From<bool> + From<String>> facts::Facter<GenericFact<T>> for BoundLeaf<'a> {
    fn collect_facts(&self) -> Vec<GenericFact<T>> {
        self.get_facts()
    }
}

impl<'a> fmt::Display for BoundLeaf<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        self.desc.display_leaf(&self.sub_leaves, f)
    }
}
