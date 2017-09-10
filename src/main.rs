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
use std::path::Path;

fn main() {
    let matches = clap_app!(maildirpack =>
        (version: env!("CARGO_PKG_VERSION"))
        (author: "Xidorn Quan <me@upsuper.org>")
        (about: "Pack mails from a maildir into archives")
        (@arg MAILDIR: +required "Path to the maildir")
    ).get_matches();

    let maildir = matches.value_of("MAILDIR").unwrap();
    let maildir = Path::new(maildir);
    let args = Args {
        maildir: maildir.to_path_buf(),
        packed_dir: maildir.join("packed"),
    };

    do_main(&args).unwrap();
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
