// (Buf) Uncomment these lines to have the output buffered, this can provide
// better performance but is not always intuitive behaviour.
// use std::io::BufWriter;

use clap::{self, Args, Parser, Subcommand, ValueEnum};
use core_affinity::CoreId;
use cpuinfo::facts::{FactSet, Facter, GenericFact};
use cpuinfo::layout::LeafDesc;
use cpuinfo::msr::MsrStore;
use cpuinfo::*;
use enum_dispatch::enum_dispatch;
use msr::MSRDesc;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

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
#[derive(Clone, Subcommand)]
enum CommandOpts {
    Disp(Disp),
    Facts(Facts),
    Diff(Diff),
}

#[derive(Clone, Args)]
struct Disp {
    #[arg(short, long, default_value = "0")]
    cpu: usize,
    #[arg(short, long)]
    raw: bool,
    #[arg(long)]
    skip_cpu: bool,
    #[cfg(all(target_os = "linux", feature = "kvm"))]
    #[arg(long)]
    skip_kvm: bool,
    #[cfg(feature = "use_msr")]
    #[arg(long)]
    skip_msr: bool,
}

impl Command for Disp {
    fn run(&self, config: &Definition) -> Result<(), Box<dyn std::error::Error>> {
        if !core_affinity::set_for_current(CoreId { id: self.cpu }) {
            panic!("Unable to pin to core {}", self.cpu);
        }
        if self.raw {
            display_raw()
        } else {
            if !self.skip_cpu {
                println!("CPUID:");
                let cpuid_db = cpuinfo::RunningCpuidDB::new();
                for (leaf, desc) in &config.cpuids {
                    if let Some(bound) = desc.bind_leaf(*leaf, &cpuid_db) {
                        println!("{:#010x}: {}", leaf, bound);
                    }
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

            #[cfg(feature = "use_msr")]
            if !self.skip_msr {
                #[cfg(target_os = "linux")]
                {
                    match msr::linux::LinuxMsrStore::new(self.cpu) {
                        Ok(linux_store) => {
                            println!("MSRS:");
                            for msr in &config.msrs {
                                match linux_store.get_value(msr) {
                                    Ok(value) => println!("{}", value),
                                    Err(err) => println!("{} Error : {}", msr, err),
                                }
                            }
                        }
                        Err(e) => println!("Error checking all msrs: {}", e),
                    }
                }
                #[cfg(all(target_os = "linux", feature = "kvm"))]
                if !self.skip_kvm {
                    use cpuinfo::kvm::KvmMsrInfo;
                    use kvm_ioctls::Kvm;
                    println!("KVM-MSR:");
                    if let Err(e) = {
                        let kvm = Kvm::new()?;
                        let kvm_msr = KvmMsrInfo::new(&kvm)?;
                        for msr in &config.msrs {
                            match kvm_msr.get_value(msr) {
                                Ok(value) => println!("{}", value),
                                Err(err) => println!("{} Error : {}", msr, err),
                            }
                        }
                        Ok::<_, Box<dyn Error>>(())
                    } {
                        println!("Error Processing KVM-MSR: {}", e);
                    }
                }
            }
            Ok(())
        }
    }
}

#[derive(Clone, PartialEq, Eq, ValueEnum)]
enum FactsOutput {
    Yaml,
    Json,
}

#[derive(Clone, Args)]
struct Facts {
    #[arg(short, long, default_value = "0")]
    cpu: usize,
    #[cfg(all(target_os = "linux", feature = "kvm"))]
    #[arg(short, long)]
    use_kvm: bool,
    #[arg(short, long, value_enum, default_value = "yaml")]
    out_type: FactsOutput,
}

fn collect_facts(
    config: &Definition,
    cpuid_selected: CpuidType,
    msr_store: Box<dyn MsrStore>,
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

    if !msr_store.is_empty() {
        for msr in &config.msrs {
            if let Ok(value) = msr_store.get_value(msr) {
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
        if !core_affinity::set_for_current(CoreId { id: self.cpu }) {
            panic!("Unable to pin to core {}", self.cpu);
        }
        let (cpuid_source, msr_source): (_, Box<dyn MsrStore>) = {
            #[cfg(all(target_os = "linux", feature = "kvm"))]
            {
                if self.use_kvm {
                    use cpuinfo::kvm::KvmInfo;
                    use kvm::KvmMsrInfo;
                    use kvm_ioctls::Kvm;
                    let kvm = Kvm::new()?;
                    (
                        KvmInfo::new(&kvm)?.into(),
                        Box::new(KvmMsrInfo::new(&kvm)?) as Box<dyn MsrStore>,
                    )
                } else {
                    let msr = {
                        #[cfg(feature = "use_msr")]
                        {
                            match msr::linux::LinuxMsrStore::new(self.cpu) {
                                Ok(store) => Box::new(store) as Box<dyn MsrStore>,
                                Err(e) => {
                                    eprintln!("Error accessing MSRs: {}", e);
                                    Box::new(msr::EmptyMSR {})
                                }
                            }
                        }
                        #[cfg(not(feature = "use_msr"))]
                        {
                            Box::new(msr::EmptyMSR {})
                        }
                    };
                    (CpuidType::func(), msr)
                }
            }
            #[cfg(all(target_os = "linux", not(feature = "kvm"), feature = "use_msr"))]
            {
                (
                    CpuidType::func(),
                    Box::new(msr::linux::LinuxMsrStore::new()?) as Box<dyn MsrStore>,
                )
            }
            #[cfg(any(
                not(target_os = "linux"),
                all(not(feature = "kvm"), not(feature = "use_msr"))
            ))]
            {
                (
                    CpuidType::func(),
                    Box::new(msr::EmptyMSR {}) as Box<dyn MsrStore>,
                )
            }
        };
        let facts = collect_facts(config, cpuid_source, msr_source)?;
        println!(
            "{}",
            match self.out_type {
                FactsOutput::Yaml => serde_yaml::to_string(&facts)?,
                FactsOutput::Json => serde_json::to_string(&facts)?,
            }
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

#[derive(Clone, Args)]
struct Diff {
    from_file_name: String,
    to_file_name: String,
    #[arg(short, long)]
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

impl Definition {
    pub fn union(&mut self, b: Definition) {
        let Definition {
            mut cpuids,
            mut msrs,
        } = b;
        self.cpuids.append(&mut cpuids);
        self.msrs.append(&mut msrs);
    }
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

fn read_additional_configs<Paths, P>(
    def: &mut Definition,
    paths: Paths,
) -> Result<(), Box<dyn std::error::Error>>
where
    Paths: Iterator<Item = P>,
    P: AsRef<Path> + Sized,
{
    for path in paths {
        let file = std::fs::read(path)?;
        let definition = serde_yaml::from_slice(&file)?;
        def.union(definition);
    }
    Ok(())
}

#[derive(Clone, Parser)]
struct CmdLine {
    #[arg(short, long)]
    add_config: Vec<PathBuf>,
    #[command(subcommand)]
    command: CommandOpts,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = CmdLine::parse();

    let mut config = find_read_config()?;

    read_additional_configs(&mut config, args.add_config.iter())?;

    args.command.run(&config)
}
