extern crate chrono;
#[macro_use]
extern crate clap;
extern crate indicatif;
extern crate rayon;
extern crate sha2;
extern crate tar;
extern crate xz2;

mod args;
mod classify;
mod collect;
mod execute;
mod verify;

use args::Args;
use std::fs;
use std::io;

fn main() {
    do_main(&Args::parse_args()).unwrap();
}

fn do_main(args: &Args) -> io::Result<()> {
    eprintln!("Listing emails...");
    let list = collect::list_emails(&args.maildir)?;

    eprintln!("Classifying emails...");
    let map = classify::classify_emails(list);

    eprintln!("Archiving emails...");
    fs::create_dir_all(&args.packed_dir)?;
    execute::archive_emails(args, map);

    Ok(())
}
