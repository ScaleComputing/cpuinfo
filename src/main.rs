// (Buf) Uncomment these lines to have the output buffered, this can provide
// better performance but is not always intuitive behaviour.
// use std::io::BufWriter;

use cpuinfo::facts::{FactSet, Facter, GenericFact};
use cpuinfo::layout::LeafDesc;
use cpuinfo::msr::MSRValue;
use cpuinfo::*;
use enum_dispatch::enum_dispatch;
use msr::MSRDesc;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::error::Error;
use std::fmt;
use structopt::StructOpt;

type YAMLFact = GenericFact<serde_yaml::Value>;
type YAMLFactSet = FactSet<serde_yaml::Value>;

#[enum_dispatch()]
trait Command {
    fn run(&self, config: &Definition) -> Result<(), Box<dyn std::error::Error>>;
}

// Our CLI arguments. (help and version are automatically generated)
// Documentation on how to use:
// https://docs.rs/structopt/0.2.10/structopt/index.html#how-to-derivestructopt
#[enum_dispatch(Command)]
#[derive(StructOpt)]
enum CommandOpts {
    Disp(Disp),
    Facts(Facts),
    Diff(Diff),
}

#[derive(StructOpt)]
struct Disp {
    #[structopt(short = "r", long)]
    display_raw: bool,
    #[cfg(all(target_os = "linux", feature = "kvm"))]
    #[structopt(long)]
    skip_kvm: bool,
    #[structopt(long)]
    skip_msr: bool,
}

impl Command for Disp {
    fn run(&self, config: &Definition) -> Result<(), Box<dyn std::error::Error>> {
        if self.display_raw {
            display_raw()
        } else {
            println!("CPUID:");
            let cpuid_db = cpuinfo::RunningCpuidDB::new();
            for (leaf, desc) in &config.cpuids {
                if let Some(bound) = desc.bind_leaf(*leaf, &cpuid_db) {
                    println!("{:#010x}: {}", leaf, bound);
                }
            }

            #[cfg(all(target_os = "linux", feature = "kvm"))]
            if !self.skip_kvm {
                use cpuinfo::kvm::KvmInfo;
                use kvm_ioctls::Kvm;
                println!("KVM-CPUID:");
                if let Err(e) = {
                    let kvm = Kvm::new()?;
                    let kvm_info = KvmInfo::new(&kvm)?;
                    for (leaf, desc) in &config.cpuids {
                        if let Some(bound) = desc.bind_leaf(*leaf, &kvm_info) {
                            println!("{:#010x}: {}", leaf, bound);
                        }
                    }
                    Ok::<(), kvm_ioctls::Error>(())
                } {
                    println!("Error Processing KVM-CPUID: {}", e);
                }
            }

            if MSRDesc::is_availible() && self.skip_msr {
                println!("MSRS:");
                for msr in &config.msrs {
                    match msr.into_value() {
                        Ok(value) => println!("{}", value),
                        Err(err) => println!("{} Error : {}", msr, err),
                    }
                }
            }
            Ok(())
        }
    }
}

#[derive(StructOpt)]
struct Facts {
    #[cfg(all(target_os = "linux", feature = "kvm"))]
    #[structopt(short, long)]
    use_kvm: bool,
}

fn collect_facts(
    config: &Definition,
    cpuid_selected: CpuidType,
) -> Result<Vec<YAMLFact>, Box<dyn std::error::Error>> {
    let mut ret: Vec<YAMLFact> = config
        .cpuids
        .iter()
        .filter_map(|(leaf, desc)| desc.bind_leaf(*leaf, &cpuid_selected))
        .flat_map(|bound| bound.get_facts().into_iter())
        .map(|mut fact| {
            fact.add_path("cpuid");
            fact
        })
        .collect();

    let use_kvm = match cpuid_selected {
        CpuidType::Func(_) => false,
        #[cfg(all(target_os = "linux", feature = "kvm"))]
        CpuidType::KvmInfo(_) => true,
    };

    if MSRDesc::is_availible() && !use_kvm {
        for msr in &config.msrs {
            if let Ok(value) = MSRValue::try_from(msr) {
                let mut facts = value.collect_facts();
                for fact in &mut facts {
                    fact.add_path("msr");
                }
                ret.append(&mut facts);
            }
        }
    }

    Ok(ret)
}

impl Command for Facts {
    fn run(&self, config: &Definition) -> Result<(), Box<dyn std::error::Error>> {
        #[cfg(all(target_os = "linux", feature = "kvm"))]
        let kvm_option = {
            use cpuinfo::kvm::KvmInfo;
            use kvm_ioctls::Kvm;
            if self.use_kvm {
                println!("using kvm");
                let kvm = Kvm::new()?;
                Some(KvmInfo::new(&kvm)?)
            } else {
                None
            }
        };

        let cpuid_source = {
            #[cfg(all(target_os = "linux", feature = "kvm"))]
            {
                if let Some(kvm_info) = kvm_option {
                    kvm_info.into()
                } else {
                    CpuidType::func()
                }
            }
            #[cfg(any(not(target_os = "linux"), not(feature = "kvm")))]
            {
                CpuidType::func()
            }
        };
        println!(
            "{}",
            serde_yaml::to_string(&collect_facts(config, cpuid_source)?)?
        );
        Ok(())
    }
}

fn read_facts_from_file(fname: &str) -> Result<Vec<YAMLFact>, Box<dyn Error>> {
    let file = std::fs::File::open(fname)?;
    Ok(serde_yaml::from_reader(file)?)
}

#[derive(Serialize, Debug)]
struct DiffOutput {
    added: Vec<YAMLFact>,
    removed: Vec<YAMLFact>,
    changed: Vec<(YAMLFact, YAMLFact)>,
}

impl DiffOutput {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty()
    }
}

#[derive(Debug)]
struct DiffFoundError {
    inner: DiffOutput,
}

impl DiffFoundError {
    pub fn new(inner: DiffOutput) -> Self {
        Self { inner }
    }
}

impl fmt::Display for DiffFoundError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let as_yaml = serde_yaml::to_string(&self.inner).map_err(|_| std::fmt::Error {})?;
        write!(f, "{}", as_yaml)
    }
}

impl std::error::Error for DiffFoundError {}

#[derive(StructOpt)]
struct Diff {
    from_file_name: String,
    to_file_name: String,
    #[structopt(short, long)]
    verbose: bool,
}

impl Command for Diff {
    fn run(&self, _config: &Definition) -> Result<(), Box<dyn Error>> {
        let from: YAMLFactSet = read_facts_from_file(&self.from_file_name)?.into();
        let to: YAMLFactSet = read_facts_from_file(&self.to_file_name)?.into();

        let output = DiffOutput {
            added: from.added_facts(&to).map(Clone::clone).collect(),
            removed: from.removed_facts(&to).map(Clone::clone).collect(),
            changed: from
                .changed_facts(&to)
                .map(|v| (v.0.clone(), v.1.clone()))
                .collect(),
        };

        if output.is_empty() {
            if self.verbose {
                println!("{}", serde_yaml::to_string(&output)?);
            }
            Ok(())
        } else {
            println!("{}", serde_yaml::to_string(&output)?);
            Err(DiffFoundError::new(output).into())
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct Definition {
    pub cpuids: BTreeMap<u32, LeafDesc>,
    pub msrs: Vec<MSRDesc>,
}

fn find_read_config() -> Result<Definition, Box<dyn std::error::Error>> {
    let file = include_str!("config.yaml");
    Ok(serde_yaml::from_str(file)?)
}

fn display_raw() -> Result<(), Box<dyn std::error::Error>> {
    let iter = CpuidIterator::new(CpuidFunction::Basic)
        .expect("problems with cpuid iterator")
        .chain(
            CpuidIterator::new(CpuidFunction::Hypervisor)
                .expect("problems with hyperfisor cpuid iterator"),
        )
        .chain(
            CpuidIterator::new(CpuidFunction::Extended)
                .expect("problems with extended cpuid iterator"),
        );
    for (LeafAddr { leaf, sub_leaf }, result) in iter {
        println!(
            "({:#010x},{:#010x}) {:#010x} {:#010x} {:#010x} {:#010x}",
            leaf, sub_leaf, result.eax, result.ebx, result.ecx, result.edx
        );
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = CommandOpts::from_args();

    let config = find_read_config()?;

    args.run(&config)
}
