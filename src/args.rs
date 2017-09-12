use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Args {
    /// The maildir we are working on.
    pub maildir: PathBuf,
    /// The directory we put packed archives in, which is maildir/packed.
    pub packed_dir: PathBuf,
}

impl Args {
    pub fn parse_args() -> Self {
        let matches = clap_app!(maildirpack =>
            (version: env!("CARGO_PKG_VERSION"))
            (author: "Xidorn Quan <me@upsuper.org>")
            (about: "Pack mails from a maildir into archives")
            (@arg MAILDIR: +required "Path to the maildir")
        ).get_matches();

        let maildir = matches.value_of("MAILDIR").unwrap();
        let maildir = Path::new(maildir);
        Args {
            maildir: maildir.to_path_buf(),
            packed_dir: maildir.join("packed"),
        }
    }
}
