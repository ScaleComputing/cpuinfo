use super::CpuidDB;
use core::arch::x86_64::CpuidResult;
use kvm_bindings::{KVM_CPUID_FLAG_SIGNIFCANT_INDEX, KVM_MAX_CPUID_ENTRIES};

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
    fn get_cpuid(&self, leaf: u32, subleaf: u32) -> CpuidResult {
        self.cpuid_info
            .as_slice()
            .iter()
            .find_map(|entry| {
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
            .unwrap_or(CpuidResult {
                eax: 0,
                ebx: 0,
                ecx: 0,
                edx: 0,
            })
    }
}
