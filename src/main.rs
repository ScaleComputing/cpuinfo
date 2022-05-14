// (Buf) Uncomment these lines to have the output buffered, this can provide
// better performance but is not always intuitive behaviour.
// use std::io::BufWriter;

use cpuid::layout::LeafDesc;
use cpuid::*;
use std::collections::BTreeMap;
use structopt::StructOpt;

// Our CLI arguments. (help and version are automatically generated)
// Documentation on how to use:
// https://docs.rs/structopt/0.2.10/structopt/index.html#how-to-derivestructopt
#[derive(StructOpt, Debug)]
struct Cli {
    #[structopt(short, long)]
    display_raw: bool,
}

fn find_read_config() -> Result<BTreeMap<u32, LeafDesc>, Box<dyn std::error::Error>> {
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
    let args = Cli::from_args();

    let config = find_read_config()?;

    if args.display_raw {
        display_raw()?;
    } else {
        for (leaf, desc) in config {
            if let Some(bound) = desc.bind_leaf(leaf) {
                println!("{:#010x}: {}", leaf, bound)
            }
        }
    }

    Ok(())
}
