//! Provide Read-Only access to Intel MSRs
//!

use super::bitfield::{self, Facter};
use super::facts::{self, GenericFact};
use serde::{Deserialize, Serialize};
use std::vec::Vec;
use std::{convert, error, fmt, io};

#[derive(Debug)]
pub enum Error {
    NotAvailible(String),
    IOError(io::Error),
}

pub type Result<V> = std::result::Result<V, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotAvailible(name) => write!(f, "MSR Feature not availible file: {}", name),
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
        Err(Error::NotAvailible("".to_string()))
    }
}

#[cfg(all(target_os = "linux", feature = "use_msr"))]
pub mod linux {
    use super::*;
    use std::fs;
    use std::io;

    pub struct LinuxMsrStore {
        msr_device: fs::File,
    }

    impl LinuxMsrStore {
        pub fn new(cpu: usize) -> Result<LinuxMsrStore> {
            let file_name = format!("/dev/cpu/{}/msr", cpu);
            Ok(LinuxMsrStore {
                msr_device: fs::OpenOptions::new()
                    .read(true)
                    .open(file_name.clone())
                    .map_err(|e| match e.kind() {
                        io::ErrorKind::NotFound => Error::NotAvailible(file_name),
                        io::ErrorKind::PermissionDenied => Error::NotAvailible(file_name),
                        _ => Error::IOError(e),
                    })?,
            })
        }
    }

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
}

/// Wraps a general description of an MSR
#[derive(Serialize, Deserialize, Debug, Clone)]
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

impl<T: From<u32> + From<bool> + From<String>> facts::Facter<GenericFact<T>> for MSRValue<'_> {
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

impl fmt::Display for MSRValue<'_> {
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
