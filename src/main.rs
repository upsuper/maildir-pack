extern crate chrono;
#[macro_use]
extern crate clap;
extern crate combine;
extern crate indicatif;
extern crate rayon;
extern crate sha2;
extern crate tar;
extern crate xz2;

mod args;
mod classify;
mod collect;
mod datetime;
mod execute;
mod utils;
mod verify;

use args::Args;
use std::fs;
use std::io;

fn main() -> io::Result<()> {
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
