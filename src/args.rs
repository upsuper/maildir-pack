use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[clap(name = "maildir-pack")]
#[clap(author, version, about)]
pub struct Args {
    /// Path to the maildir.
    pub maildir: PathBuf,
    /// The directory we put packed archives in, which is maildir/packed.
    #[clap(skip)]
    pub packed_dir: PathBuf,
    /// Suppress any progress output if set.
    #[clap(short, long)]
    pub quiet: bool,
}

impl Args {
    pub fn parse_args() -> Self {
        let mut result: Self = Self::parse();
        result.packed_dir = result.maildir.join("packed");
        result
    }
}
