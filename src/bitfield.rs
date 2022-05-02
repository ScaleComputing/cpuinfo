//! Provide a means to specify a bit field when working with CPU ID and feature registers
//!

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

///Wraps a bit flag, usually representing if a feature is present or not
#[derive(Serialize, Deserialize, Debug)]
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
#[derive(Serialize, Deserialize, Debug)]
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

impl<'a> fmt::Display for Bound<'a, Flag> {
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

impl<'a> fmt::Display for Bound<'a, Int> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "{} = {:>10x}",
            self.bits.name,
            self.bits.value(self.reg_val).unwrap_or(0)
        )
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Field {
    Int(Int),
    Flag(Flag),
}

pub enum BoundField<'a> {
    Int(Bound<'a, Int>),
    Flag(Bound<'a, Flag>),
}

impl<'a> BoundField<'a> {
    pub fn from_register_and_field(reg_val: Register, field: &'a Field) -> Self {
        match field {
            Field::Int(bits) => Self::Int(Bound { reg_val, bits }),
            Field::Flag(bits) => Self::Flag(Bound { reg_val, bits }),
        }
    }
}

impl<'a> fmt::Display for BoundField<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            Self::Int(bound) => bound.fmt(f),
            Self::Flag(bound) => bound.fmt(f),
        }
    }
}
