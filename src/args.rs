use std::path::PathBuf;

#[derive(Debug)]
pub struct Args {
    pub maildir: PathBuf,
    pub packed_dir: PathBuf,
}
