use super::{bitfield, cpuid, is_empty_leaf};
use core::arch::x86_64::CpuidResult;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::vec::Vec;

pub trait DisplayLeaf {
    fn scan_sub_leaves(&self, leaf: u32) -> Vec<CpuidResult>;
    fn display_leaf(
        &self,
        leaf: &[CpuidResult],
        f: &mut fmt::Formatter<'_>,
    ) -> Result<(), fmt::Error>;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StartLeaf {}

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
            ebx,
            ecx,
            edx,
        } = leaf[0];

        let text = String::from_utf8(
            vec![ebx, edx, ecx]
                .into_iter()
                .flat_map(|val| Vec::from(val.to_le_bytes()).into_iter())
                .collect(),
        )
        .unwrap();

        write!(f, "'{}' max leaf:{}", text, max_leaf)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StringLeaf {}

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
        let CpuidResult { eax, ebx, ecx, edx } = leaf[0];
        let registers = vec![eax, ebx, ecx, edx];

        let text = String::from_utf8(
            registers
                .into_iter()
                .flat_map(|val| Vec::from(val.to_le_bytes()).into_iter())
                .collect(),
        )
        .unwrap();

        write!(f, "'{}'", text)
    }
}

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

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum LeafType {
    Start(StartLeaf),
    String(StringLeaf),
    BitField(BitFieldLeaf),
}

impl DisplayLeaf for LeafType {
    fn scan_sub_leaves(&self, leaf: u32) -> Vec<CpuidResult> {
        match self {
            LeafType::Start(desc) => desc.scan_sub_leaves(leaf),
            LeafType::String(desc) => desc.scan_sub_leaves(leaf),
            LeafType::BitField(desc) => desc.scan_sub_leaves(leaf),
        }
    }
    fn display_leaf(
        &self,
        leaf: &[CpuidResult],
        f: &mut fmt::Formatter<'_>,
    ) -> Result<(), fmt::Error> {
        match self {
            LeafType::Start(desc) => desc.display_leaf(leaf, f),
            LeafType::String(desc) => desc.display_leaf(leaf, f),
            LeafType::BitField(desc) => desc.display_leaf(leaf, f),
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
