use core::arch::x86_64::{CpuidResult, __cpuid_count};

pub mod bitfield;
pub mod facts;
pub mod layout;
pub mod msr;

#[derive(Debug)]
pub enum CpuidError {
    NoCPUID,
    LeafOutOfRange(u32, CpuidFunction),
}

impl std::fmt::Display for CpuidError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CpuidError::NoCPUID => write!(f, "No CPUID Present on hardware"),
            CpuidError::LeafOutOfRange(leaf, func) => {
                write!(f, "Leaf {:#x} not present in function {:?}", leaf, func)
            }
        }
    }
}

impl std::error::Error for CpuidError {}

fn cpuid(leaf: u32, sub_leaf: u32) -> CpuidResult {
    unsafe { __cpuid_count(leaf, sub_leaf) }
}

#[derive(Debug, Clone)]
pub enum CpuidFunction {
    Basic,
    Hypervisor,
    Extended,
}

impl CpuidFunction {
    pub fn start_eax(&self) -> u32 {
        match self {
            CpuidFunction::Basic => 0,
            CpuidFunction::Hypervisor => 0x40000000,
            CpuidFunction::Extended => 0x80000000,
        }
    }
    pub fn is_valid_leaf(&self, leaf: u32) -> bool {
        match self {
            CpuidFunction::Basic => leaf < 0x40000000,
            CpuidFunction::Hypervisor => (0x40000000..0x50000000).contains(&leaf),
            CpuidFunction::Extended => leaf >= 0x80000000,
        }
    }
}

#[derive(Debug, Hash, Clone)]
pub struct LeafAddr {
    pub leaf: u32,
    pub sub_leaf: u32,
}

#[derive(Debug, Clone)]
pub struct CpuidIterator {
    leaf: u32,
    sub_leaf: u32,
    last: u32,
    last_sub_leaf: Option<CpuidResult>,
}

impl CpuidIterator {
    pub fn new(func: CpuidFunction) -> Result<CpuidIterator, CpuidError> {
        CpuidIterator::at_leaf(func.start_eax(), func)
    }
    pub fn at_leaf(leaf: u32, func: CpuidFunction) -> Result<CpuidIterator, CpuidError> {
        CpuidIterator::at_sub_leaf(leaf, 0, func)
    }

    pub fn at_sub_leaf(
        leaf: u32,
        sub_leaf: u32,
        func: CpuidFunction,
    ) -> Result<CpuidIterator, CpuidError> {
        let range_info_function = func.start_eax();

        if func.is_valid_leaf(leaf) {
            Ok(CpuidIterator {
                leaf,
                sub_leaf,
                last: cpuid(range_info_function, 0).eax,
                last_sub_leaf: None,
            })
        } else {
            Err(CpuidError::LeafOutOfRange(leaf, func))
        }
    }
}

fn is_empty_leaf(result: &CpuidResult) -> bool {
    let CpuidResult {
        eax,
        ebx,
        ecx,
        edx: _,
    } = result;
    // See
    *eax == 0 && *ebx == 0 && *ecx & 0x0000FF00 == 0
}

impl Iterator for CpuidIterator {
    type Item = (LeafAddr, CpuidResult);
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.leaf > self.last {
                break None;
            }
            let current = cpuid(self.leaf, self.sub_leaf);
            if is_empty_leaf(&current) || self.last_sub_leaf.take() == Some(current) {
                self.leaf += 1;
                self.sub_leaf = 0;
            } else {
                let sub_leaf = self.sub_leaf;
                self.sub_leaf += 1;
                self.last_sub_leaf.replace(current);
                break Some((
                    LeafAddr {
                        leaf: self.leaf,
                        sub_leaf,
                    },
                    current,
                ));
            }
        }
    }
}
