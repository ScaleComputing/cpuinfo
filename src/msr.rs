//! Provide Read-Only access to Intel MSRs
//!

use super::bitfield::{self, Facter};
use super::facts::{self, GenericFact};
use serde::{Deserialize, Serialize};
use std::fs;
use std::vec::Vec;
use std::{convert, error, fmt, io};

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

pub trait MsrStore {
    fn is_empty(&self) -> bool;
    fn get_value<'a>(&self, desc: &'a MSRDesc) -> std::result::Result<MSRValue<'a>, Error>;
}

pub struct EmptyMSR {}

impl MsrStore for EmptyMSR {
    fn is_empty(&self) -> bool {
        true
    }
    fn get_value<'a>(&self, _desc: &'a MSRDesc) -> std::result::Result<MSRValue<'a>, Error> {
        Err(Error::NotAvailible)
    }
}

#[cfg(all(target_os = "linux", feature = "use_msr"))]
pub struct LinuxMsrStore {
    msr_device: fs::File,
}

#[cfg(all(target_os = "linux", feature = "use_msr"))]
impl LinuxMsrStore {
    pub fn new() -> Result<LinuxMsrStore> {
        Ok(LinuxMsrStore {
            msr_device: fs::OpenOptions::new()
                .read(true)
                .open("/dev/cpu/0/msr")
                .map_err(|e| match e.kind() {
                    io::ErrorKind::NotFound => Error::NotAvailible,
                    io::ErrorKind::PermissionDenied => Error::NotAvailible,
                    _ => Error::IOError(e),
                })?,
        })
    }
}

#[cfg(all(target_os = "linux", feature = "use_msr"))]
impl MsrStore for LinuxMsrStore {
    fn is_empty(&self) -> bool {
        false
    }
    fn get_value<'a>(&self, desc: &'a MSRDesc) -> std::result::Result<MSRValue<'a>, Error> {
        use std::os::unix::fs::FileExt;
        let mut msr_bytes = [u8::MIN; 8];
        self.msr_device
            .read_at(&mut msr_bytes, desc.address.into())?;
        Ok(MSRValue {
            desc,
            value: u64::from_le_bytes(msr_bytes),
        })
    }
}

/// Wraps a general description of an MSR
#[derive(Serialize, Deserialize, Debug)]
pub struct MSRDesc {
    pub name: String,
    pub address: u32,
    pub fields: Vec<bitfield::Field>,
}

impl fmt::Display for MSRDesc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {:#x}", self.name, self.address)
    }
}

pub struct MSRValue<'a> {
    pub desc: &'a MSRDesc,
    pub value: u64,
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
