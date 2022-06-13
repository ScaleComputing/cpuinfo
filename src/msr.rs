//! Provide Read-Only access to Intel MSRs
//!

use super::bitfield::{self, Facter};
use super::facts::{self, GenericFact};
use serde::{Deserialize, Serialize};
use std::vec::Vec;
use std::{
    convert::{self, TryInto},
    error, fmt, io,
};

#[derive(Debug)]
pub enum Error {
    NotAvailible,
    IOError(io::Error),
}

pub type Result<V> = std::result::Result<V, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotAvailible => write!(f, "MSR Feature not availible"),
            Error::IOError(e) => write!(f, "IOError: {}", e),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        None
    }
}

impl convert::From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::IOError(e)
    }
}

/// Wraps a general description of an MSR
#[derive(Serialize, Deserialize, Debug)]
pub struct MSRDesc {
    pub name: String,
    pub address: u32,
    pub fields: Vec<bitfield::Field>,
}

impl MSRDesc {
    #[cfg(all(target_os = "linux", feature = "use_msr"))]
    pub fn get_value(&self) -> Result<u64> {
        use std::{
            fs,
            io::{Read, Seek},
        };

        let mut file = fs::OpenOptions::new()
            .read(true)
            .open("/dev/cpu/0/msr")
            .map_err(|e| match e.kind() {
                io::ErrorKind::NotFound => Error::NotAvailible,
                io::ErrorKind::PermissionDenied => Error::NotAvailible,
                _ => Error::IOError(e),
            })?;
        file.seek(io::SeekFrom::Start(self.address.into()))?;
        let mut msr_bytes = [u8::MIN; 8];
        file.read_exact(&mut msr_bytes)?;
        Ok(u64::from_le_bytes(msr_bytes))
    }
    #[cfg(all(target_os = "linux", feature = "use_msr"))]
    pub fn is_availible() -> bool {
        true
    }

    #[cfg(any(not(target_os = "linux"), not(feature = "use_msr")))]
    pub fn get_value(&self) -> Result<u64> {
        Err(Error::NotAvailible)
    }
    #[cfg(any(not(target_os = "linux"), not(feature = "use_msr")))]
    pub fn is_availible() -> bool {
        false
    }

    pub fn into_value(&self) -> Result<MSRValue> {
        self.try_into()
    }
}

impl<'a> fmt::Display for MSRDesc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {:#x}", self.name, self.address)
    }
}

pub struct MSRValue<'a> {
    pub desc: &'a MSRDesc,
    pub value: u64,
}

impl<'a> convert::TryFrom<&'a MSRDesc> for MSRValue<'a> {
    type Error = Error;
    fn try_from(desc: &'a MSRDesc) -> Result<MSRValue<'a>> {
        Ok(MSRValue {
            desc,
            value: desc.get_value()?,
        })
    }
}

impl<'a, T: From<u32> + From<bool> + From<String>> facts::Facter<GenericFact<T>> for MSRValue<'a> {
    fn collect_facts(&self) -> Vec<GenericFact<T>> {
        let value = self.value.into();
        self.desc
            .fields
            .iter()
            .map(|field| {
                let mut fact =
                    bitfield::BoundField::from_register_and_field(value, field).collect_fact();
                fact.add_path(&self.desc.name);
                fact
            })
            .collect()
    }
}

impl<'a> fmt::Display for MSRValue<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{} = {:#x}", self.desc, self.value)?;
        for field in &self.desc.fields {
            writeln!(
                f,
                "  {}",
                bitfield::BoundField::from_register_and_field(self.value.into(), field)
            )?
        }
        Ok(())
    }
}
