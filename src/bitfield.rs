//! Provide a means to specify a bit field when working with CPU ID and feature registers
//!

use super::facts::GenericFact;
use enum_dispatch::enum_dispatch;
use serde::{Deserialize, Serialize};
use std::convert::TryInto;
use std::fmt;
use std::ops;

pub type Register = u128;

/// A type is Bindable if it can be "bound" to a register
pub trait Bindable {
    /// The value type that results from a bind
    type Rep;
    /// A function to extract the value from the register
    fn value(&self, reg_val: Register) -> Option<Self::Rep>;
    /// Retreive the name of the bindable
    fn name(&self) -> &String;
}

#[enum_dispatch()]
pub trait Facter<T: From<u32> + From<bool>> {
    fn collect_fact(&self) -> GenericFact<T>;
}

///Wraps a bit flag, usually representing if a feature is present or not
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Flag {
    pub name: String,
    pub bit: u8,
}

impl Bindable for Flag {
    type Rep = bool;
    fn value(&self, reg_val: Register) -> Option<Self::Rep> {
        let flag = 1u128.checked_shl(self.bit.into())?;
        Some((reg_val & flag) != 0)
    }
    fn name(&self) -> &String {
        &self.name
    }
}

///Wraps an integer value from a bit field
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Int {
    pub name: String,
    pub bounds: ops::Range<u8>,
}

impl Bindable for Int {
    type Rep = u32;
    fn value(&self, reg_val: Register) -> Option<Self::Rep> {
        let shift = self.bounds.start;
        let mut mask = 0u128;

        for _bit in self.bounds.clone() {
            mask <<= 1;
            mask |= 1;
        }
        ((reg_val >> shift) & mask).try_into().ok()
    }
    fn name(&self) -> &String {
        &self.name
    }
}

/// Wraps an X86Model representation
/// These can have a number of weird conditions and are always going to be a part of a bit field
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct X86Model {
    pub name: String,
}

const MODEL_START_BIT: u8 = 4;
const EXTENDED_MODEL_START_BIT: u8 = 16;
const FAMILY_START_BIT: u8 = 8;
impl Bindable for X86Model {
    type Rep = u32;
    fn value(&self, reg_val: Register) -> Option<Self::Rep> {
        let reg32 = reg_val as u32;
        let nibble_mask = 0xF;
        let model = (reg32 >> MODEL_START_BIT) & nibble_mask;
        let famil_id = (reg32 >> FAMILY_START_BIT) & nibble_mask;

        match famil_id {
            6 | 0xF => {
                let extended_model = (reg32 >> EXTENDED_MODEL_START_BIT) & nibble_mask;
                Some((extended_model << 4) | model)
            }
            _ => Some(model),
        }
    }
    fn name(&self) -> &String {
        &self.name
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct X86Family {
    pub name: String,
}

const EXTENDED_FAMILY_START_BIT: u8 = 20;
impl Bindable for X86Family {
    type Rep = u32;
    fn value(&self, reg_val: Register) -> Option<Self::Rep> {
        let reg32 = reg_val as u32;
        const FAMILY_MASK: u32 = 0xF;
        const EXT_FAMILY_MASK: u32 = 0xFF;
        let family = (reg32 >> FAMILY_START_BIT) & FAMILY_MASK;
        let extended_family = (reg32 >> EXTENDED_FAMILY_START_BIT) & EXT_FAMILY_MASK;

        match family {
            0xF => Some(extended_family + family),
            _ => Some(family),
        }
    }
    fn name(&self) -> &String {
        &self.name
    }
}

pub struct Bound<'a, T: Bindable> {
    reg_val: Register,
    bits: &'a T,
}

impl<'a, T: Bindable> Bound<'a, T> {
    pub fn new(reg_val: Register, bits: &'a T) -> Self {
        Self { reg_val, bits }
    }
    pub fn name(&self) -> &String {
        self.bits.name()
    }
}

impl fmt::Display for Bound<'_, Flag> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "{} = {:>10}",
            self.bits.name,
            if let Some(true) = self.bits.value(self.reg_val) {
                "true"
            } else {
                "false"
            }
        )
    }
}

impl fmt::Display for Bound<'_, Int> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "{} = {:>10x}",
            self.bits.name,
            self.bits.value(self.reg_val).unwrap_or(0)
        )
    }
}

impl<B, R, T: From<u32> + From<bool>> Facter<T> for Bound<'_, B>
where
    R: Default + Into<T>,
    B: Bindable<Rep = R>,
{
    fn collect_fact(&self) -> GenericFact<T> {
        GenericFact::new(
            self.bits.name().clone(),
            self.bits.value(self.reg_val).unwrap_or_default().into(),
        )
    }
}

impl fmt::Display for Bound<'_, X86Model> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "{} = {:>10}",
            self.bits.name,
            self.bits.value(self.reg_val).unwrap_or(0)
        )
    }
}

impl fmt::Display for Bound<'_, X86Family> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "{} = {:>10}",
            self.bits.name,
            self.bits.value(self.reg_val).unwrap_or(0)
        )
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum Field {
    Int(Int),
    Flag(Flag),
    X86Model(X86Model),
    X86Family(X86Family),
}

pub enum BoundField<'a> {
    Int(Bound<'a, Int>),
    Flag(Bound<'a, Flag>),
    X86Model(Bound<'a, X86Model>),
    X86Family(Bound<'a, X86Family>),
}

impl<'a> BoundField<'a> {
    pub fn from_register_and_field(reg_val: Register, field: &'a Field) -> Self {
        match field {
            Field::Int(bits) => Self::Int(Bound { reg_val, bits }),
            Field::Flag(bits) => Self::Flag(Bound { reg_val, bits }),
            Field::X86Model(bits) => Self::X86Model(Bound { reg_val, bits }),
            Field::X86Family(bits) => Self::X86Family(Bound { reg_val, bits }),
        }
    }
}

impl fmt::Display for BoundField<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            Self::Int(bound) => bound.fmt(f),
            Self::Flag(bound) => bound.fmt(f),
            Self::X86Model(bound) => bound.fmt(f),
            Self::X86Family(bound) => bound.fmt(f),
        }
    }
}

impl<T: From<bool> + From<u32>> Facter<T> for BoundField<'_> {
    fn collect_fact(&self) -> GenericFact<T> {
        match self {
            Self::Int(bound) => bound.collect_fact(),
            Self::Flag(bound) => bound.collect_fact(),
            Self::X86Model(bound) => bound.collect_fact(),
            Self::X86Family(bound) => bound.collect_fact(),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::bitfield::Bindable;

    #[test]
    fn x86_model_test() {
        let field_definition = super::X86Model {
            name: "model".to_string(),
        };
        let regular_model: super::Register = 0x0AF50341;
        assert_eq!(field_definition.value(regular_model).unwrap(), 0x4);
        let extended_family_model: super::Register = 0x0AF50641;
        assert_eq!(field_definition.value(extended_family_model).unwrap(), 0x54);
        let extended_family_model: super::Register = 0x0AF50F41;
        assert_eq!(field_definition.value(extended_family_model).unwrap(), 0x54);
    }
    #[test]
    fn x86_family_test() {
        let field_definition = super::X86Family {
            name: "model".to_string(),
        };
        let regular_model: super::Register = 0x0AE50341;
        assert_eq!(field_definition.value(regular_model).unwrap(), 0x3);
        let extended_family_model: super::Register = 0x0AE50F41;
        assert_eq!(
            field_definition.value(extended_family_model).unwrap(),
            0xAE + 0xF
        );
    }
}
