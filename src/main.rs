// (Buf) Uncomment these lines to have the output buffered, this can provide
// better performance but is not always intuitive behaviour.
// use std::io::BufWriter;

use cpuid::facts::{GenericFact, Facter};
use cpuid::layout::LeafDesc;
use cpuid::msr::MSRValue;
use cpuid::*;
use enum_dispatch::enum_dispatch;
use msr::MSRDesc;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::convert::TryFrom;
use structopt::StructOpt;

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
}

#[derive(StructOpt)]
struct Disp {
    #[structopt(short, long)]
    display_raw: bool,
}

impl Command for Disp {
    fn run(&self, config: &Definition) -> Result<(), Box<dyn std::error::Error>> {
        if self.display_raw {
            display_raw()
        } else {
            println!("CPUID:");
            for (leaf, desc) in &config.cpuids {
                if let Some(bound) = desc.bind_leaf(*leaf) {
                    println!("{:#010x}: {}", leaf, bound);
                }
            }
            println!("MSRS:");
            for msr in &config.msrs {
                match msr.into_value() {
                    Ok(value) => println!("{}", value),
                    Err(err) => println!("{} Error : {}", msr, err),
                }
            }
            Ok(())
        }
    }
}

#[derive(StructOpt)]
struct Facts {}

fn collect_facts(config: &Definition) -> Result<Vec<GenericFact<serde_yaml::Value>>, Box<dyn std::error::Error>> {
    let mut ret: Vec<GenericFact<serde_yaml::Value>> = config
        .cpuids
        .iter()
        .filter_map(|(leaf, desc)| desc.bind_leaf(*leaf))
        .flat_map(|bound| bound.get_facts().into_iter())
        .map(|mut fact| {
            fact.add_path("cpuid");
            fact
        })
        .collect();

    for msr in &config.msrs {
        if let Ok(value) = MSRValue::try_from(msr) {
            let mut facts = value.collect_facts();
            for fact in &mut facts {
                fact.add_path("msr");
            }
            ret.append(&mut facts);
        }
    }

    Ok(ret)
}

impl Command for Facts {
    fn run(&self, config: &Definition) -> Result<(), Box<dyn std::error::Error>> {
        println!("{}", serde_yaml::to_string(&collect_facts(config)?)?);
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct Definition {
    pub cpuids: BTreeMap<u32, LeafDesc>,
    pub msrs: Vec<MSRDesc>,
}

fn find_read_config() -> Result<Definition, Box<dyn std::error::Error>> {
    let file = std::fs::File::open("config.yaml")?;
    Ok(serde_yaml::from_reader(file)?)
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
