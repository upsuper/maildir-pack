mod args;
mod classify;
mod collect;
mod datetime;
mod execute;
mod utils;
mod verify;

use crate::args::Args;
use anyhow::Result;
use std::fs;

fn main() -> Result<()> {
    let args = Args::parse_args();

    macro_rules! report {
        ($msg:expr) => {
            if !args.quiet {
                eprintln!($msg);
            }
        };
    }

    report!("Listing emails...");
    let list = collect::list_emails(&args)?;

    report!("Classifying emails...");
    let map = classify::classify_emails(list);

    report!("Archiving emails...");
    fs::create_dir_all(&args.packed_dir)?;
    execute::archive_emails(&args, map);

    Ok(())
}
