use crate::msr::{self, MSRValue, MsrStore};

use super::CpuidDB;
use core::arch::x86_64::CpuidResult;
use kvm_bindings::{kvm_msr_entry, Msrs, KVM_CPUID_FLAG_SIGNIFCANT_INDEX, KVM_MAX_CPUID_ENTRIES};
use std::error::Error;

/** Wrap information from kvm
 *
 * Other structures such as CPUInfo will then make it accessible like the cpuid function
 */
pub struct KvmInfo {
    cpuid_info: kvm_bindings::fam_wrappers::CpuId,
}

impl KvmInfo {
    pub fn new(kvm: &kvm_ioctls::Kvm) -> Result<Self, kvm_ioctls::Error> {
        let cpuid_info = kvm.get_supported_cpuid(KVM_MAX_CPUID_ENTRIES)?;
        Ok(Self { cpuid_info })
    }
}

impl CpuidDB for KvmInfo {
    fn get_cpuid(&self, leaf: u32, subleaf: u32) -> Option<CpuidResult> {
        self.cpuid_info.as_slice().iter().find_map(|entry| {
            if entry.function == leaf {
                if (subleaf == 0 && (entry.flags & KVM_CPUID_FLAG_SIGNIFCANT_INDEX) == 0)
                    || (subleaf == entry.index)
                {
                    Some(CpuidResult {
                        eax: entry.eax,
                        ebx: entry.ebx,
                        ecx: entry.ecx,
                        edx: entry.edx,
                    })
                } else {
                    None
                }
            } else {
                None
            }
        })
    }
}

pub struct KvmMsrInfo {
    msr_info: kvm_bindings::Msrs,
}

impl KvmMsrInfo {
    pub fn new(kvm: &kvm_ioctls::Kvm) -> Result<Self, Box<dyn Error>> {
        let msr_features = kvm.get_msr_feature_index_list()?;
        let mut msrs = Msrs::from_entries(
            &msr_features
                .as_slice()
                .iter()
                .map(|&index| kvm_msr_entry {
                    index,
                    ..Default::default()
                })
                .collect::<Vec<_>>(),
        )?;
        kvm.get_msrs(&mut msrs)?;
        Ok(KvmMsrInfo { msr_info: msrs })
    }
}

impl MsrStore for KvmMsrInfo {
    fn is_empty(&self) -> bool {
        false
    }
    fn get_value<'a>(
        &self,
        desc: &'a crate::msr::MSRDesc,
    ) -> std::result::Result<crate::msr::MSRValue<'a>, crate::msr::Error> {
        self.msr_info
            .as_slice()
            .iter()
            .find_map(|entry| {
                if entry.index == desc.address {
                    Some(MSRValue {
                        desc,
                        value: entry.data,
                    })
                } else {
                    None
                }
            })
            .ok_or_else(|| msr::Error::NotAvailible("/dev/kvm".to_string()))
    }
}
